//! Application services: open connections through actors, run statements, and
//! keep history and the audit log current. One actor per open connection owns
//! the driver session and serializes commands for that connection.
//!
//! A service can also run without an app store (`headless`), which keeps the
//! scaffold probe possible before the connection manager exists and keeps the
//! future test harness free of keyring state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use unidb_core::{
    BundleQuery, BundleVerification, ConnectionProfile, Engine, ErrorCategory, ForeignKeyInfo,
    MultiTargetOutcome, MultiTargetSummary, QueryResult, ResultDiff, SqlDialect, TableColumnInfo,
    TeamBundle, TransferFormat, UniDbError, coerce_value, diff_results, generate_fake_rows,
    insert_statements, insertable_columns, inspect_bundle_json, mask_rows, merge_multi_results,
    parse_import, rows_to_csv, rows_to_json, valid_identifier,
};
use unidb_engine::{
    CassandraConnector, ConnectorRegistry, MongoConnector, MssqlConnector, MySqlConnector,
    PostgresConnector, RedisConnector, Session, SqliteConnector,
};
use unidb_store::{
    AuditEntry, HistoryEntry, KeyringKeyProvider, MasterKeyProvider, SavedQuery, Store,
    TableSnapshot,
};

use crate::bridge::{Bridge, QueryEvent};
use crate::migrations::{load_migrations, plan_migrations};
use crate::paths::default_data_dir;
use crate::security::{self, SecurityReport};

enum Command {
    Execute {
        sql: String,
        reply: oneshot::Sender<Result<QueryResult, UniDbError>>,
    },
    ListDatabases {
        reply: oneshot::Sender<Result<Vec<String>, UniDbError>>,
    },
    ListTables {
        database: String,
        reply: oneshot::Sender<Result<Vec<String>, UniDbError>>,
    },
    Cancel {
        reply: oneshot::Sender<Result<(), UniDbError>>,
    },
    DescribeTable {
        database: String,
        table: String,
        reply: oneshot::Sender<Result<Vec<TableColumnInfo>, UniDbError>>,
    },
    ApplyStatements {
        statements: Vec<String>,
        reply: oneshot::Sender<Result<(), UniDbError>>,
    },
    ForeignKeys {
        database: String,
        table: String,
        reply: oneshot::Sender<Result<Vec<ForeignKeyInfo>, UniDbError>>,
    },
    Shutdown,
}

struct ConnectionHandle {
    sender: mpsc::Sender<Command>,
    join: JoinHandle<()>,
}

/// The app service layer. Holds the runtime bridge, the optional store, the
/// engine registry, and the live connection actors.
/// Summary of one import run.
#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub rows_read: usize,
    pub rows_inserted: usize,
    pub rows_rejected: usize,
    pub batches: usize,
    pub warnings: Vec<String>,
}

/// Options for importing a file into a table.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub database: String,
    pub table: String,
    pub path: PathBuf,
    pub format: TransferFormat,
    pub has_headers: bool,
    pub delimiter: u8,
    pub null_marker: Option<String>,
}

/// Summary of one export run.
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub rows: usize,
    pub path: PathBuf,
}

/// Decoded and verified team bundle ready for review before import.
#[derive(Debug, Clone)]
pub struct BundlePreview {
    pub bundle: TeamBundle,
    pub verification: BundleVerification,
    pub sig_path: Option<PathBuf>,
}

/// Summary returned after importing a team bundle into the store.
#[derive(Debug, Clone)]
pub struct BundleImportReport {
    pub connections_imported: usize,
    pub connections_skipped: usize,
    pub queries_imported: usize,
    pub signature_valid: bool,
}

pub struct AppService {
    bridge: Bridge,
    store: Option<Arc<Store>>,
    connectors: Arc<ConnectorRegistry>,
    connections: Mutex<HashMap<Uuid, ConnectionHandle>>,
    previous_unclean_shutdown: Option<String>,
}

impl AppService {
    pub fn new(bridge: Bridge, store: Arc<Store>, connectors: Arc<ConnectorRegistry>) -> Self {
        Self {
            bridge,
            store: Some(store),
            connectors,
            connections: Mutex::new(HashMap::new()),
            previous_unclean_shutdown: None,
        }
    }

    /// A service without a store. Used by the scaffold probe and by tests that
    /// only exercise engine work.
    pub fn headless(bridge: Bridge, connectors: Arc<ConnectorRegistry>) -> Self {
        Self {
            bridge,
            store: None,
            connectors,
            connections: Mutex::new(HashMap::new()),
            previous_unclean_shutdown: None,
        }
    }

    /// The engine registry with every engine built into this binary.
    pub fn connector_registry() -> Arc<ConnectorRegistry> {
        let mut connectors = ConnectorRegistry::new();
        connectors.register(Arc::new(SqliteConnector::new()));
        connectors.register_shared(
            &[Engine::PostgreSQL, Engine::CockroachDB],
            Arc::new(PostgresConnector::new()),
        );
        connectors.register_shared(
            &[Engine::MySQL, Engine::MariaDB],
            Arc::new(MySqlConnector::new()),
        );
        connectors.register(Arc::new(RedisConnector::new()));
        connectors.register(Arc::new(MongoConnector::new()));
        connectors.register(Arc::new(CassandraConnector::new()));
        connectors.register(Arc::new(MssqlConnector::new()));
        Arc::new(connectors)
    }

    /// The scaffold service: runtime bridge plus the engines that are built.
    pub fn bootstrap_headless(runtime: Handle) -> Self {
        Self::headless(Bridge::new(runtime), Self::connector_registry())
    }

    /// The real service: opens the app store with the desktop keyring.
    pub async fn bootstrap(runtime: Handle) -> Result<Self, UniDbError> {
        Self::bootstrap_in(runtime, default_data_dir(), &KeyringKeyProvider::new()).await
    }

    /// Bootstrap with an injected data directory and key provider, used by
    /// tests and by the diagnostic retry path.
    pub async fn bootstrap_in(
        runtime: Handle,
        data_dir: impl AsRef<Path>,
        provider: &dyn MasterKeyProvider,
    ) -> Result<Self, UniDbError> {
        let data_dir = data_dir.as_ref();
        std::fs::create_dir_all(data_dir).map_err(|error| {
            UniDbError::new(
                "store_directory_failed",
                ErrorCategory::Internal,
                "Could not create the app data directory",
            )
            .with_source(error)
        })?;
        let store = Store::open(data_dir.join("unidb.db"), provider).await?;
        let previous_unclean_shutdown = store.begin_run().await?;
        Ok(Self {
            bridge: Bridge::new(runtime),
            store: Some(Arc::new(store)),
            connectors: Self::connector_registry(),
            connections: Mutex::new(HashMap::new()),
            previous_unclean_shutdown,
        })
    }

    pub fn bridge(&self) -> &Bridge {
        &self.bridge
    }

    pub fn store(&self) -> Option<&Arc<Store>> {
        self.store.as_ref()
    }

    /// Sends an event to the interface sink. Used by callers that own a stream.
    pub fn emit(&self, sink: &dyn crate::bridge::EventSink<QueryEvent>, event: QueryEvent) {
        sink.send(event);
    }

    fn require_store(&self) -> Result<&Arc<Store>, UniDbError> {
        self.store.as_ref().ok_or_else(|| {
            UniDbError::new(
                "store_unavailable",
                ErrorCategory::Internal,
                "The app database is not open",
            )
        })
    }

    pub fn data_dir(&self) -> Option<PathBuf> {
        self.store
            .as_ref()
            .and_then(|store| store.database_path().parent().map(Path::to_path_buf))
    }

    /// Saves or updates a connection and records the audit entry.
    pub async fn save_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<(), UniDbError> {
        let store = self.require_store()?;
        store.save_connection(profile, password).await?;
        store
            .add_audit(
                "connection_saved",
                Some(profile.id),
                "success",
                serde_json::json!({
                    "engine": profile.engine.display(),
                    "name": profile.name,
                }),
            )
            .await
    }

    pub async fn list_connections(&self) -> Result<Vec<ConnectionProfile>, UniDbError> {
        self.require_store()?.list_connections().await
    }

    pub async fn delete_connection(&self, id: Uuid) -> Result<u64, UniDbError> {
        let store = self.require_store()?;
        let removed = store.delete_connection(id).await?;
        store
            .add_audit(
                "connection_deleted",
                Some(id),
                if removed > 0 { "success" } else { "missing" },
                serde_json::json!({}),
            )
            .await?;
        Ok(removed)
    }

    pub async fn recent_audit(&self, limit: i64) -> Result<Vec<AuditEntry>, UniDbError> {
        self.require_store()?.list_audit(limit).await
    }

    /// Copies the app database to a new backup and rotates the old ones.
    pub async fn backup_now(&self) -> Result<PathBuf, UniDbError> {
        self.require_store()?.backup_now().await
    }

    /// The live security report for the Security page.
    pub async fn security_report(&self) -> Result<SecurityReport, UniDbError> {
        security::build(
            self.require_store()?,
            self.previous_unclean_shutdown.clone(),
        )
        .await
    }

    /// Closes every connection and marks a clean shutdown.
    pub async fn shutdown(&self) {
        self.close_all().await;
        if let Some(store) = &self.store
            && let Err(error) = store.mark_clean_shutdown().await
        {
            tracing::warn!(message = %error, "could not mark a clean shutdown");
        }
    }

    /// Opens a session, starts its actor, and records the audit entry.
    pub async fn open_connection(&self, profile: &ConnectionProfile) -> Result<(), UniDbError> {
        profile.validate()?;
        let connector = self.connectors.get(profile.engine)?;
        let password = match &self.store {
            Some(store) => store.get_password(profile.id).await?,
            None => None,
        };
        let session = connector
            .connect(profile, password.as_ref().map(|secret| secret.as_str()))
            .await?;

        let (sender, receiver) = mpsc::channel(8);
        let actor = ConnectionActor {
            session,
            connection_id: profile.id,
        };
        let join = self.bridge.handle().spawn(actor.run(receiver));

        let mut connections = self.connections.lock().await;
        connections.insert(profile.id, ConnectionHandle { sender, join });
        if let Some(store) = &self.store {
            store
                .add_audit(
                    "connection_opened",
                    Some(profile.id),
                    "success",
                    serde_json::json!({ "engine": profile.engine.display() }),
                )
                .await?;
        }
        Ok(())
    }

    /// Runs a statement on an open connection and records history.
    pub async fn execute(&self, connection_id: Uuid, sql: &str) -> Result<QueryResult, UniDbError> {
        let sender = {
            let connections = self.connections.lock().await;
            connections
                .get(&connection_id)
                .map(|handle| handle.sender.clone())
                .ok_or_else(|| {
                    UniDbError::new(
                        "connection_not_open",
                        unidb_core::ErrorCategory::ConnectionLost,
                        "This connection is not open",
                    )
                })?
        };

        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::Execute {
                sql: sql.to_string(),
                reply: reply_sender,
            })
            .await
            .map_err(|_| {
                UniDbError::new(
                    "connection_actor_stopped",
                    unidb_core::ErrorCategory::ConnectionLost,
                    "The connection stopped unexpectedly",
                )
            })?;
        let result = reply_receiver.await.map_err(|_| {
            UniDbError::new(
                "connection_actor_stopped",
                unidb_core::ErrorCategory::ConnectionLost,
                "The connection stopped unexpectedly",
            )
        })?;

        if let Some(store) = &self.store {
            let (outcome, rows, elapsed, code) = match &result {
                Ok(query_result) => (
                    "success",
                    i64::try_from(query_result.rows.len()).unwrap_or(i64::MAX),
                    query_result.elapsed_ms,
                    None,
                ),
                Err(error) => ("error", 0, 0, Some(error.code())),
            };
            store
                .record_history(
                    Some(connection_id),
                    sql,
                    elapsed,
                    Some(rows),
                    outcome,
                    code,
                    None,
                )
                .await?;
            store
                .add_audit(
                    "query_run",
                    Some(connection_id),
                    outcome,
                    serde_json::json!({
                        "rows": rows,
                        "elapsed_ms": elapsed,
                        "error_code": code,
                    }),
                )
                .await?;
        }
        result
    }

    /// Asks the engine to cancel the running statement on a connection.
    pub async fn cancel(&self, connection_id: Uuid) -> Result<(), UniDbError> {
        let sender = self.connection_sender(connection_id).await?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::Cancel {
                reply: reply_sender,
            })
            .await
            .map_err(|_| connection_stopped_error())?;
        reply_receiver
            .await
            .map_err(|_| connection_stopped_error())?
    }

    /// Plans migrations for a connection against a migrations directory.
    /// Returns discovered files, names of already applied migrations, and any checksum warnings.
    pub async fn plan_migrations(
        &self,
        connection_id: Uuid,
        dir: &Path,
    ) -> Result<
        (
            Vec<crate::migrations::MigrationFile>,
            Vec<String>,
            Vec<String>,
        ),
        UniDbError,
    > {
        let files = load_migrations(dir)?;
        let applied_rows = self
            .require_store()?
            .list_applied_migrations(connection_id)
            .await?;
        let applied: Vec<(String, String)> = applied_rows
            .iter()
            .map(|row| (row.name.clone(), row.checksum.clone()))
            .collect();
        let applied_names: Vec<String> = applied.iter().map(|(n, _)| n.clone()).collect();
        let (_pending, warnings) = plan_migrations(&files, &applied);
        Ok((files, applied_names, warnings))
    }

    /// Applies pending numbered SQL files from a directory in one transaction
    /// per file, recording each in the migration ledger.
    pub async fn apply_migrations(
        &self,
        connection_id: Uuid,
        dir: &Path,
    ) -> Result<crate::migrations::MigrationReport, UniDbError> {
        let files = load_migrations(dir)?;
        let applied_rows = self
            .require_store()?
            .list_applied_migrations(connection_id)
            .await?;
        let applied: Vec<(String, String)> = applied_rows
            .iter()
            .map(|row| (row.name.clone(), row.checksum.clone()))
            .collect();
        let (pending, warnings) = plan_migrations(&files, &applied);
        let mut report = crate::migrations::MigrationReport {
            skipped: files.len() - pending.len(),
            warnings,
            ..crate::migrations::MigrationReport::default()
        };
        for file in pending {
            let statements = unidb_core::split_statements(&file.sql);
            if statements.is_empty() {
                continue;
            }
            self.apply_statements(connection_id, &statements)
                .await
                .map_err(|error| {
                    UniDbError::new(
                        error.code(),
                        error.category(),
                        format!("Migration {} failed: {}", file.name, error.user_message()),
                    )
                    .with_detail(error.to_string())
                })?;
            self.require_store()?
                .record_applied_migration(connection_id, &file.name, &file.checksum)
                .await?;
            self.audit_transfer(
                "migration_applied",
                Some(connection_id),
                "success",
                serde_json::json!({ "name": file.name }),
            )
            .await?;
            report.applied.push(file.name);
        }
        Ok(report)
    }

    /// Captures a point in time snapshot of a table, capped at 500 rows.
    pub async fn create_snapshot(
        &self,
        connection_id: Uuid,
        database: &str,
        table: &str,
        note: Option<String>,
    ) -> Result<TableSnapshot, UniDbError> {
        let profile = self
            .require_store()?
            .get_connection(connection_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "connection_not_found",
                    ErrorCategory::ConnectionLost,
                    "The connection profile could not be loaded",
                )
            })?;
        let dialect = SqlDialect::for_engine(profile.engine).ok_or_else(|| {
            UniDbError::new(
                "snapshot_unsupported",
                ErrorCategory::Unsupported,
                "Snapshots are not available for this engine",
            )
        })?;
        if !valid_identifier(table) {
            return Err(UniDbError::new(
                "snapshot_invalid_table",
                ErrorCategory::Syntax,
                format!("Invalid table name {table}"),
            ));
        }
        let sql = format!("SELECT * FROM {} LIMIT 500", dialect.quote(table));
        let result = self.execute(connection_id, &sql).await?;
        let snapshot =
            TableSnapshot::from_query_result(connection_id, database, table, note, &result)
                .map_err(|error| {
                    UniDbError::internal("Could not encode snapshot data").with_source(error)
                })?;
        self.require_store()?.save_snapshot(&snapshot).await?;
        self.audit_transfer(
            "snapshot_created",
            Some(connection_id),
            "success",
            serde_json::json!({
                "table": table,
                "database": database,
                "rows": snapshot.row_count,
            }),
        )
        .await?;
        Ok(snapshot)
    }

    /// Lists snapshots for one connection, newest first.
    pub async fn list_snapshots(
        &self,
        connection_id: Uuid,
    ) -> Result<Vec<TableSnapshot>, UniDbError> {
        self.require_store()?.list_snapshots(connection_id).await
    }

    /// Fetches one snapshot by id.
    pub async fn get_snapshot(&self, id: Uuid) -> Result<Option<TableSnapshot>, UniDbError> {
        self.require_store()?.get_snapshot(id).await
    }

    /// Deletes one snapshot by id.
    pub async fn delete_snapshot(&self, id: Uuid) -> Result<bool, UniDbError> {
        self.require_store()?.delete_snapshot(id).await
    }

    /// Compares a stored table snapshot against the live table state.
    pub async fn diff_snapshot(
        &self,
        connection_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<ResultDiff, UniDbError> {
        let snapshot = self
            .require_store()?
            .get_snapshot(snapshot_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "snapshot_not_found",
                    ErrorCategory::Internal,
                    "The snapshot could not be found",
                )
            })?;
        let baseline = snapshot.to_query_result().map_err(|error| {
            UniDbError::internal("Could not decode snapshot data").with_source(error)
        })?;
        let profile = self
            .require_store()?
            .get_connection(connection_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "connection_not_found",
                    ErrorCategory::ConnectionLost,
                    "The connection profile could not be loaded",
                )
            })?;
        let dialect = SqlDialect::for_engine(profile.engine).ok_or_else(|| {
            UniDbError::new(
                "snapshot_unsupported",
                ErrorCategory::Unsupported,
                "Snapshots are not available for this engine",
            )
        })?;
        let sql = format!(
            "SELECT * FROM {} LIMIT 500",
            dialect.quote(&snapshot.table_name)
        );
        let current = self.execute(connection_id, &sql).await?;
        let key_index = baseline
            .columns
            .iter()
            .position(|col| col.name.eq_ignore_ascii_case("id"))
            .unwrap_or(0);
        let diff = diff_results(&baseline, &current, key_index, 100);
        Ok(diff)
    }

    /// Restores a table from a snapshot transactionally.
    pub async fn restore_snapshot(
        &self,
        connection_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<usize, UniDbError> {
        let snapshot = self
            .require_store()?
            .get_snapshot(snapshot_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "snapshot_not_found",
                    ErrorCategory::Internal,
                    "The snapshot could not be found",
                )
            })?;
        let result = snapshot.to_query_result().map_err(|error| {
            UniDbError::internal("Could not decode snapshot data").with_source(error)
        })?;
        let profile = self
            .require_store()?
            .get_connection(connection_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "connection_not_found",
                    ErrorCategory::ConnectionLost,
                    "The connection profile could not be loaded",
                )
            })?;
        let dialect = SqlDialect::for_engine(profile.engine).ok_or_else(|| {
            UniDbError::new(
                "snapshot_unsupported",
                ErrorCategory::Unsupported,
                "Snapshots are not available for this engine",
            )
        })?;
        if !valid_identifier(&snapshot.table_name) {
            return Err(UniDbError::new(
                "snapshot_invalid_table",
                ErrorCategory::Syntax,
                format!("Invalid table name {}", snapshot.table_name),
            ));
        }

        let mut statements = vec![format!(
            "DELETE FROM {}",
            dialect.quote(&snapshot.table_name)
        )];
        if !result.rows.is_empty() && !result.columns.is_empty() {
            let column_names: Vec<String> = result.columns.iter().map(|c| c.name.clone()).collect();
            let inserts = insert_statements(
                dialect,
                &snapshot.table_name,
                &column_names,
                &result.rows,
                200,
            )?;
            statements.extend(inserts);
        }

        self.apply_statements(connection_id, &statements).await?;

        self.audit_transfer(
            "snapshot_restored",
            Some(connection_id),
            "success",
            serde_json::json!({
                "table": snapshot.table_name,
                "database": snapshot.database_name,
                "rows": snapshot.row_count,
            }),
        )
        .await?;

        Ok(snapshot.row_count)
    }

    /// Executes a SQL statement across multiple open connections concurrently and
    /// merges the rows with a leading `_connection` column.
    pub async fn execute_multi(
        &self,
        connection_ids: &[Uuid],
        sql: &str,
    ) -> Result<(Vec<MultiTargetSummary>, QueryResult), UniDbError> {
        if connection_ids.is_empty() {
            return Err(UniDbError::new(
                "multi_query_no_connections",
                ErrorCategory::ConnectionLost,
                "Open at least one connection before running across databases",
            ));
        }
        let saved = match &self.store {
            Some(store) => store.list_connections().await.unwrap_or_default(),
            None => Vec::new(),
        };
        let senders: HashMap<Uuid, mpsc::Sender<Command>> = {
            let guard = self.connections.lock().await;
            connection_ids
                .iter()
                .filter_map(|id| guard.get(id).map(|handle| (*id, handle.sender.clone())))
                .collect()
        };

        let mut tasks = Vec::with_capacity(connection_ids.len());
        for &id in connection_ids {
            let (name, engine) = saved
                .iter()
                .find(|profile| profile.id == id)
                .map(|profile| (profile.name.clone(), profile.engine.display().to_string()))
                .unwrap_or_else(|| (id.to_string(), "Unknown".to_string()));
            let sender = senders.get(&id).cloned();
            let statement = sql.to_string();
            tasks.push(tokio::spawn(async move {
                let started = std::time::Instant::now();
                let result = match sender {
                    None => Err("This connection is not open".to_string()),
                    Some(tx) => {
                        let (reply_sender, reply_receiver) = oneshot::channel();
                        if tx
                            .send(Command::Execute {
                                sql: statement,
                                reply: reply_sender,
                            })
                            .await
                            .is_err()
                        {
                            Err("The connection stopped unexpectedly".to_string())
                        } else {
                            match reply_receiver.await {
                                Ok(Ok(qr)) => Ok(qr),
                                Ok(Err(error)) => Err(error.user_message().to_string()),
                                Err(_) => Err("The connection stopped unexpectedly".to_string()),
                            }
                        }
                    }
                };
                MultiTargetOutcome {
                    connection_id: id,
                    connection_name: name,
                    engine,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    result,
                }
            }));
        }

        let mut outcomes = Vec::with_capacity(tasks.len());
        for task in tasks {
            let outcome = task.await.map_err(|error| {
                UniDbError::internal("Multi database query task failed").with_source(error)
            })?;
            outcomes.push(outcome);
        }

        let (summaries, merged) = merge_multi_results(&outcomes, 500);
        self.audit_transfer(
            "multi_query_run",
            None,
            "success",
            serde_json::json!({
                "targets": summaries.len(),
                "rows": merged.rows.len(),
            }),
        )
        .await?;
        Ok((summaries, merged))
    }

    /// Exports sanitized connection profiles and saved queries to a signed JSON bundle
    /// plus a detached `.sig` integrity file. Passwords and keys are never exported.
    pub async fn export_team_bundle(
        &self,
        bundle_name: &str,
        created_by: &str,
        path: &Path,
    ) -> Result<(TeamBundle, PathBuf), UniDbError> {
        let store = self.require_store()?;
        let profiles = store.list_connections().await?;
        let saved = store.list_saved_queries().await?;
        let queries: Vec<BundleQuery> = saved
            .into_iter()
            .map(|q| BundleQuery {
                name: q.name,
                sql: q.sql,
            })
            .collect();
        let bundle = TeamBundle::build(bundle_name, created_by, &profiles, queries)?;
        let json = bundle.to_json_pretty()?;
        tokio::fs::write(path, &json).await.map_err(|error| {
            UniDbError::new(
                "bundle_write_failed",
                ErrorCategory::Network,
                "Could not write the team bundle file",
            )
            .with_detail(error.to_string())
        })?;
        let sig_path = PathBuf::from(format!("{}.sig", path.display()));
        tokio::fs::write(&sig_path, bundle.detached_signature_content())
            .await
            .map_err(|error| {
                UniDbError::new(
                    "bundle_sig_write_failed",
                    ErrorCategory::Network,
                    "Could not write the detached signature file",
                )
                .with_detail(error.to_string())
            })?;
        self.audit_transfer(
            "team_bundle_exported",
            None,
            "success",
            serde_json::json!({
                "bundle_name": bundle.bundle_name,
                "connections": bundle.connections.len(),
                "queries": bundle.saved_queries.len(),
                "fingerprint": bundle.signature_sha256.chars().take(12).collect::<String>(),
            }),
        )
        .await?;
        Ok((bundle, sig_path))
    }

    /// Reads and verifies a team bundle and optional sibling `.sig` file without importing.
    pub async fn inspect_team_bundle(&self, path: &Path) -> Result<BundlePreview, UniDbError> {
        let json_text = tokio::fs::read_to_string(path).await.map_err(|error| {
            UniDbError::new(
                "bundle_read_failed",
                ErrorCategory::Network,
                "Could not read the team bundle file",
            )
            .with_detail(error.to_string())
        })?;
        let sig_candidate = PathBuf::from(format!("{}.sig", path.display()));
        let (sig_text, sig_path) = match tokio::fs::read_to_string(&sig_candidate).await {
            Ok(text) => (Some(text), Some(sig_candidate)),
            Err(_) => (None, None),
        };
        let (bundle, verification) = inspect_bundle_json(&json_text, sig_text.as_deref())?;
        Ok(BundlePreview {
            bundle,
            verification,
            sig_path,
        })
    }

    /// Imports a verified team bundle into the local app store without credentials.
    pub async fn import_team_bundle(
        &self,
        path: &Path,
        require_valid_signature: bool,
    ) -> Result<BundleImportReport, UniDbError> {
        let preview = self.inspect_team_bundle(path).await?;
        if require_valid_signature && !preview.verification.signature_valid {
            return Err(UniDbError::new(
                "bundle_signature_invalid",
                ErrorCategory::Permission,
                preview.verification.message,
            ));
        }
        let store = self.require_store()?;
        let existing_conns = store.list_connections().await?;
        let mut connections_imported = 0usize;
        let mut connections_skipped = 0usize;
        for item in &preview.bundle.connections {
            let duplicate = existing_conns.iter().any(|c| {
                c.name.eq_ignore_ascii_case(&item.name)
                    && c.database.eq_ignore_ascii_case(&item.database)
            });
            if duplicate {
                connections_skipped += 1;
                continue;
            }
            let profile = item.to_profile();
            store.save_connection(&profile, None).await?;
            connections_imported += 1;
        }
        let existing_queries = store.list_saved_queries().await?;
        let mut queries_imported = 0usize;
        for query in &preview.bundle.saved_queries {
            let duplicate = existing_queries
                .iter()
                .any(|q| q.name.eq_ignore_ascii_case(&query.name));
            if duplicate {
                continue;
            }
            store
                .save_saved_query(&query.name, &query.sql, None)
                .await?;
            queries_imported += 1;
        }
        self.audit_transfer(
            "team_bundle_imported",
            None,
            "success",
            serde_json::json!({
                "bundle_name": preview.bundle.bundle_name,
                "connections_imported": connections_imported,
                "connections_skipped": connections_skipped,
                "queries_imported": queries_imported,
                "signature_valid": preview.verification.signature_valid,
            }),
        )
        .await?;
        Ok(BundleImportReport {
            connections_imported,
            connections_skipped,
            queries_imported,
            signature_valid: preview.verification.signature_valid,
        })
    }

    /// Generates deterministic fake rows and inserts them into a table.
    pub async fn generate_fake_rows(
        &self,
        connection_id: Uuid,
        database: &str,
        table: &str,
        rows: usize,
        seed: u64,
    ) -> Result<usize, UniDbError> {
        let columns = self.describe_table(connection_id, database, table).await?;
        let insertable = insertable_columns(&columns);
        if insertable.is_empty() {
            return Err(UniDbError::new(
                "fake_data_no_columns",
                ErrorCategory::Unsupported,
                "This table has no columns that can receive generated values",
            ));
        }
        let profile = self
            .require_store()?
            .get_connection(connection_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "connection_not_found",
                    ErrorCategory::ConnectionLost,
                    "The connection profile could not be loaded",
                )
            })?;
        let dialect = SqlDialect::for_engine(profile.engine).ok_or_else(|| {
            UniDbError::new(
                "fake_data_unsupported",
                ErrorCategory::Unsupported,
                "Fake data is supported for SQL engines only",
            )
        })?;
        let data = generate_fake_rows(&columns, rows, seed);
        let names: Vec<String> = insertable
            .iter()
            .map(|column| column.name.clone())
            .collect();
        let statements = insert_statements(dialect, table, &names, &data, 200)?;
        for statement in &statements {
            self.apply_statements(connection_id, std::slice::from_ref(statement))
                .await?;
        }
        self.audit_transfer(
            "fake_data_generated",
            Some(connection_id),
            "success",
            serde_json::json!({
                "table": table,
                "rows": data.len(),
                "seed": seed,
            }),
        )
        .await?;
        Ok(data.len())
    }

    /// Describes the columns of a table for the designer.
    pub async fn describe_table(
        &self,
        connection_id: Uuid,
        database: &str,
        table: &str,
    ) -> Result<Vec<TableColumnInfo>, UniDbError> {
        let sender = self.connection_sender(connection_id).await?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::DescribeTable {
                database: database.to_string(),
                table: table.to_string(),
                reply: reply_sender,
            })
            .await
            .map_err(|_| connection_stopped_error())?;
        reply_receiver
            .await
            .map_err(|_| connection_stopped_error())?
    }

    /// Lists the foreign keys declared on a table.
    pub async fn foreign_keys(
        &self,
        connection_id: Uuid,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, UniDbError> {
        let sender = self.connection_sender(connection_id).await?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::ForeignKeys {
                database: database.to_string(),
                table: table.to_string(),
                reply: reply_sender,
            })
            .await
            .map_err(|_| connection_stopped_error())?;
        reply_receiver
            .await
            .map_err(|_| connection_stopped_error())?
    }

    /// Applies DDL statements inside one transaction on an open connection.
    pub async fn apply_statements(
        &self,
        connection_id: Uuid,
        statements: &[String],
    ) -> Result<(), UniDbError> {
        let sender = self.connection_sender(connection_id).await?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::ApplyStatements {
                statements: statements.to_vec(),
                reply: reply_sender,
            })
            .await
            .map_err(|_| connection_stopped_error())?;
        reply_receiver
            .await
            .map_err(|_| connection_stopped_error())?
    }

    /// Lists databases or keyspaces for an open connection.
    pub async fn list_databases(&self, connection_id: Uuid) -> Result<Vec<String>, UniDbError> {
        let sender = self.connection_sender(connection_id).await?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::ListDatabases {
                reply: reply_sender,
            })
            .await
            .map_err(|_| connection_stopped_error())?;
        reply_receiver
            .await
            .map_err(|_| connection_stopped_error())?
    }

    /// Lists tables, collections, or keys inside a database.
    pub async fn list_tables(
        &self,
        connection_id: Uuid,
        database: &str,
    ) -> Result<Vec<String>, UniDbError> {
        let sender = self.connection_sender(connection_id).await?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        sender
            .send(Command::ListTables {
                database: database.to_string(),
                reply: reply_sender,
            })
            .await
            .map_err(|_| connection_stopped_error())?;
        reply_receiver
            .await
            .map_err(|_| connection_stopped_error())?
    }

    async fn connection_sender(
        &self,
        connection_id: Uuid,
    ) -> Result<mpsc::Sender<Command>, UniDbError> {
        self.connections
            .lock()
            .await
            .get(&connection_id)
            .map(|handle| handle.sender.clone())
            .ok_or_else(|| {
                UniDbError::new(
                    "connection_not_open",
                    ErrorCategory::ConnectionLost,
                    "This connection is not open",
                )
            })
    }

    /// Tests a profile without saving it or keeping the session open.
    pub async fn test_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<(), UniDbError> {
        profile.validate()?;
        let connector = self.connectors.get(profile.engine)?;
        connector.test(profile, password).await
    }

    /// Reads a CSV or JSON file and inserts its rows into a table in batches.
    pub async fn import_file(
        &self,
        connection_id: Uuid,
        options: &ImportOptions,
    ) -> Result<ImportReport, UniDbError> {
        let bytes = tokio::fs::read(&options.path).await.map_err(|error| {
            UniDbError::new(
                "transfer_read_failed",
                ErrorCategory::Network,
                "The import file could not be read",
            )
            .with_detail(error.to_string())
        })?;
        let data = parse_import(
            &bytes,
            options.format,
            options.has_headers,
            options.delimiter,
        )?;
        let profile = self
            .require_store()?
            .get_connection(connection_id)
            .await?
            .ok_or_else(|| {
                UniDbError::new(
                    "connection_not_found",
                    ErrorCategory::ConnectionLost,
                    "The connection profile could not be loaded",
                )
            })?;
        let dialect = SqlDialect::for_engine(profile.engine).ok_or_else(|| {
            UniDbError::new(
                "transfer_unsupported",
                ErrorCategory::Unsupported,
                "Import is not available for this engine",
            )
        })?;
        let table_columns = self
            .describe_table(connection_id, &options.database, &options.table)
            .await?;
        let mut warnings = Vec::new();

        let mut mapped: Vec<(usize, String)> = Vec::new();
        for column in &table_columns {
            match data
                .headers
                .iter()
                .position(|header| header.eq_ignore_ascii_case(&column.name))
            {
                Some(index) => mapped.push((index, column.name.clone())),
                None => {
                    if column.nullable || column.default.is_some() {
                        warnings.push(format!(
                            "Column {} is missing from the file and will be NULL",
                            column.name
                        ));
                    } else if !column.primary_key {
                        return Err(UniDbError::new(
                            "transfer_mapping_failed",
                            ErrorCategory::Syntax,
                            format!(
                                "Column {} is required but missing from the file",
                                column.name
                            ),
                        ));
                    }
                }
            }
        }
        for (index, header) in data.headers.iter().enumerate() {
            if !mapped
                .iter()
                .any(|(mapped_index, _)| *mapped_index == index)
            {
                warnings.push(format!("Field {header} does not match a table column"));
            }
        }
        if mapped.is_empty() {
            return Err(UniDbError::new(
                "transfer_mapping_failed",
                ErrorCategory::Syntax,
                "No file fields match the table columns",
            ));
        }

        let mut report = ImportReport {
            rows_read: data.rows.len(),
            ..ImportReport::default()
        };
        let mut coerced_rows: Vec<Vec<unidb_core::Value>> = Vec::new();
        for (row_index, row) in data.rows.iter().enumerate() {
            let mut values = Vec::new();
            let mut failed: Option<String> = None;
            for (header_index, column_name) in &mapped {
                let target = table_columns
                    .iter()
                    .find(|column| &column.name == column_name)
                    .map(|column| column.type_name.as_str())
                    .unwrap_or("TEXT");
                let field = row.get(*header_index).and_then(Option::as_deref);
                match coerce_value(field, target, options.null_marker.as_deref()) {
                    Ok(value) => values.push(value),
                    Err(reason) => {
                        failed = Some(format!(
                            "row {}: column {}: {reason}",
                            row_index + 1,
                            column_name
                        ));
                        break;
                    }
                }
            }
            match failed {
                Some(reason) => {
                    report.rows_rejected += 1;
                    if report.warnings.len() < 5 {
                        report.warnings.push(reason);
                    }
                }
                None => coerced_rows.push(values),
            }
        }
        if coerced_rows.is_empty() {
            report.batches = 0;
            report.warnings.push("No rows were imported".to_string());
            self.audit_transfer(
                "import_finished",
                Some(connection_id),
                "success",
                serde_json::json!({
                    "rows_read": report.rows_read,
                    "rows_inserted": 0,
                    "rows_rejected": report.rows_rejected,
                    "format": options.format.label(),
                    "path": options.path.display().to_string(),
                }),
            )
            .await?;
            return Ok(report);
        }
        let column_names: Vec<String> = mapped.iter().map(|(_, name)| name.clone()).collect();
        let statements =
            insert_statements(dialect, &options.table, &column_names, &coerced_rows, 200)?;
        for statement in &statements {
            let rows_in_batch = (coerced_rows.len() - report.rows_inserted).min(200);
            if let Err(error) = self
                .apply_statements(connection_id, std::slice::from_ref(statement))
                .await
            {
                self.audit_transfer(
                    "import_finished",
                    Some(connection_id),
                    "error",
                    serde_json::json!({
                        "rows_read": report.rows_read,
                        "rows_inserted": report.rows_inserted,
                        "error_code": error.code(),
                        "format": options.format.label(),
                    }),
                )
                .await?;
                return Err(error);
            }
            report.batches += 1;
            report.rows_inserted += rows_in_batch;
        }
        self.audit_transfer(
            "import_finished",
            Some(connection_id),
            "success",
            serde_json::json!({
                "rows_read": report.rows_read,
                "rows_inserted": report.rows_inserted,
                "rows_rejected": report.rows_rejected,
                "batches": report.batches,
                "format": options.format.label(),
                "path": options.path.display().to_string(),
            }),
        )
        .await?;
        Ok(report)
    }

    /// Runs a statement and writes its result as CSV or JSON.
    pub async fn export_query(
        &self,
        connection_id: Uuid,
        sql: &str,
        format: TransferFormat,
        path: &Path,
    ) -> Result<ExportReport, UniDbError> {
        let result = self.execute(connection_id, sql).await?;
        let contents = match format {
            TransferFormat::Csv => rows_to_csv(&result.columns, &result.rows)?,
            TransferFormat::Json => rows_to_json(&result.columns, &result.rows)?,
        };
        tokio::fs::write(path, contents).await.map_err(|error| {
            UniDbError::new(
                "transfer_write_failed",
                ErrorCategory::Network,
                "The export file could not be written",
            )
            .with_detail(error.to_string())
        })?;
        self.audit_transfer(
            "export_finished",
            Some(connection_id),
            "success",
            serde_json::json!({
                "rows": result.rows.len(),
                "format": format.label(),
                "path": path.display().to_string(),
            }),
        )
        .await?;
        Ok(ExportReport {
            rows: result.rows.len(),
            path: path.to_path_buf(),
        })
    }

    /// Runs a statement and writes a masked CSV or JSON copy of its result.
    pub async fn export_query_masked(
        &self,
        connection_id: Uuid,
        sql: &str,
        format: TransferFormat,
        path: &Path,
    ) -> Result<ExportReport, UniDbError> {
        let result = self.execute(connection_id, sql).await?;
        let rows = mask_rows(&result.rows);
        let contents = match format {
            TransferFormat::Csv => rows_to_csv(&result.columns, &rows)?,
            TransferFormat::Json => rows_to_json(&result.columns, &rows)?,
        };
        tokio::fs::write(path, contents).await.map_err(|error| {
            UniDbError::new(
                "transfer_write_failed",
                ErrorCategory::Network,
                "The masked export could not be written",
            )
            .with_detail(error.to_string())
        })?;
        self.audit_transfer(
            "export_finished",
            Some(connection_id),
            "success",
            serde_json::json!({
                "rows": rows.len(),
                "format": format.label(),
                "masked": true,
                "path": path.display().to_string(),
            }),
        )
        .await?;
        Ok(ExportReport {
            rows: rows.len(),
            path: path.to_path_buf(),
        })
    }

    async fn audit_transfer(
        &self,
        action: &str,
        connection_id: Option<Uuid>,
        outcome: &str,
        detail: serde_json::Value,
    ) -> Result<(), UniDbError> {
        if let Some(store) = &self.store {
            store
                .add_audit(action, connection_id, outcome, detail)
                .await?;
        }
        Ok(())
    }

    /// Records a history row without a live connection. Used by tests and by
    /// future offline features.
    pub async fn record_history_for_test(&self, sql: &str, status: &str) {
        if let Some(store) = &self.store {
            let _ = store
                .record_history(None, sql, 0, None, status, None, None)
                .await;
        }
    }

    /// Checks an update feed and returns a release only when it is newer than
    /// the running build. Nothing is downloaded or installed.
    pub async fn check_updates(
        &self,
        feed_url: &str,
    ) -> Result<Option<crate::update::ReleaseInfo>, UniDbError> {
        let release = crate::update::fetch_latest(feed_url).await?;
        if crate::update::is_newer(env!("CARGO_PKG_VERSION"), &release.version) {
            Ok(Some(release))
        } else {
            Ok(None)
        }
    }

    /// Searches the query history through FTS5, newest first.
    pub async fn search_history(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<HistoryEntry>, UniDbError> {
        self.require_store()?.search_history(query, limit).await
    }

    /// Deletes every history row.
    pub async fn clear_history(&self) -> Result<u64, UniDbError> {
        self.require_store()?.clear_history().await
    }

    /// Lists saved queries newest first.
    pub async fn list_saved_queries(&self) -> Result<Vec<SavedQuery>, UniDbError> {
        self.require_store()?.list_saved_queries().await
    }

    /// Saves a named statement.
    pub async fn save_saved_query(
        &self,
        name: &str,
        sql: &str,
        connection_id: Option<Uuid>,
    ) -> Result<SavedQuery, UniDbError> {
        self.require_store()?
            .save_saved_query(name, sql, connection_id)
            .await
    }

    /// Deletes one saved query.
    pub async fn delete_saved_query(&self, id: Uuid) -> Result<u64, UniDbError> {
        self.require_store()?.delete_saved_query(id).await
    }

    /// Reads one setting.
    pub async fn get_setting(&self, key: &str) -> Result<Option<serde_json::Value>, UniDbError> {
        self.require_store()?.get_setting(key).await
    }

    /// Writes one setting.
    pub async fn set_setting(&self, key: &str, value: serde_json::Value) -> Result<(), UniDbError> {
        self.require_store()?.set_setting(key, value).await
    }

    /// Opens one in memory SQLite session and runs a version query. This is the
    /// scaffold proof that the interface can reach a worker and get a result.
    pub async fn probe_sqlite(&self) -> Result<QueryResult, UniDbError> {
        let profile = ConnectionProfile::new("Probe", Engine::SQLite, "", ":memory:");
        let connector = self.connectors.get(Engine::SQLite)?;
        let mut session = connector.connect(&profile, None).await?;
        session.execute("SELECT sqlite_version() AS version").await
    }

    /// Sends shutdown to the actor and waits briefly for it to finish.
    pub async fn close_connection(&self, connection_id: Uuid) -> Result<(), UniDbError> {
        let handle = {
            let mut connections = self.connections.lock().await;
            connections.remove(&connection_id)
        };
        let Some(handle) = handle else {
            return Ok(());
        };
        let _ = handle.sender.send(Command::Shutdown).await;
        match tokio::time::timeout(Duration::from_secs(5), handle.join).await {
            Ok(Ok(())) => {
                if let Some(store) = &self.store {
                    store
                        .add_audit(
                            "connection_closed",
                            Some(connection_id),
                            "success",
                            serde_json::json!({}),
                        )
                        .await?;
                }
                Ok(())
            }
            Ok(Err(error)) => Err(
                UniDbError::internal("The connection task ended unexpectedly").with_source(error),
            ),
            Err(_) => Err(UniDbError::new(
                "connection_close_timeout",
                unidb_core::ErrorCategory::Timeout,
                "The connection did not close in time",
            )),
        }
    }

    /// Closes every open connection. Called on app shutdown.
    pub async fn close_all(&self) {
        let ids: Vec<Uuid> = {
            let connections = self.connections.lock().await;
            connections.keys().copied().collect()
        };
        for id in ids {
            let _ = self.close_connection(id).await;
        }
    }
}

struct ConnectionActor {
    session: Box<dyn Session>,
    connection_id: Uuid,
}

impl ConnectionActor {
    async fn apply_statements(&mut self, statements: &[String]) -> Result<(), UniDbError> {
        self.session.begin().await?;
        for statement in statements {
            if let Err(error) = self.session.execute(statement).await {
                let _ = self.session.rollback().await;
                return Err(error);
            }
        }
        self.session.commit().await
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<Command>) {
        while let Some(command) = receiver.recv().await {
            match command {
                Command::Execute { sql, reply } => {
                    let result = self.session.execute(&sql).await;
                    if reply.send(result).is_err() {
                        tracing::debug!(
                            connection_id = %self.connection_id,
                            "query reply dropped by the interface"
                        );
                    }
                }
                Command::ListDatabases { reply } => {
                    let result = self.session.list_databases().await;
                    let _ = reply.send(result);
                }
                Command::ListTables { database, reply } => {
                    let result = self.session.list_tables(&database).await;
                    let _ = reply.send(result);
                }
                Command::Cancel { reply } => {
                    let result = self.session.cancel().await;
                    let _ = reply.send(result);
                }
                Command::DescribeTable {
                    database,
                    table,
                    reply,
                } => {
                    let result = self.session.describe_table(&database, &table).await;
                    let _ = reply.send(result);
                }
                Command::ApplyStatements { statements, reply } => {
                    let result = self.apply_statements(&statements).await;
                    let _ = reply.send(result);
                }
                Command::ForeignKeys {
                    database,
                    table,
                    reply,
                } => {
                    let result = self.session.list_foreign_keys(&database, &table).await;
                    let _ = reply.send(result);
                }
                Command::Shutdown => break,
            }
        }
        tracing::debug!(connection_id = %self.connection_id, "connection actor stopped");
    }
}

fn connection_stopped_error() -> UniDbError {
    UniDbError::new(
        "connection_actor_stopped",
        ErrorCategory::ConnectionLost,
        "The connection stopped unexpectedly",
    )
}
