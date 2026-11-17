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
    ConnectionProfile, Engine, ErrorCategory, QueryResult, TableColumnInfo, UniDbError,
};
use unidb_engine::{
    CassandraConnector, ConnectorRegistry, MongoConnector, MssqlConnector, MySqlConnector,
    PostgresConnector, RedisConnector, Session, SqliteConnector,
};
use unidb_store::{AuditEntry, KeyringKeyProvider, MasterKeyProvider, Store};

use crate::bridge::{Bridge, QueryEvent};
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
    Shutdown,
}

struct ConnectionHandle {
    sender: mpsc::Sender<Command>,
    join: JoinHandle<()>,
}

/// The app service layer. Holds the runtime bridge, the optional store, the
/// engine registry, and the live connection actors.
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
