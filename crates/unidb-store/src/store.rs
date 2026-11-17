//! The app store: one SQLite file for connections, settings, history, and the
//! audit log. Migrations run at open, secrets are encrypted before they touch
//! disk, and the master key lifecycle is enforced here.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;
use zeroize::Zeroizing;

use unidb_core::{ConnectionProfile, Engine, ErrorCategory, SslMode, UniDbError};

use crate::secrets::{
    EncryptedSecret, MasterKey, MasterKeyProvider, SecretBox, connection_password_aad,
};

/// Stored audit entry, returned by `list_audit`.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEntry {
    pub id: i64,
    pub ts: String,
    pub action: String,
    pub connection_id: Option<String>,
    pub outcome: String,
    pub detail: serde_json::Value,
}

/// Saved query row, returned by `list_saved_queries`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQuery {
    pub id: Uuid,
    pub name: String,
    pub sql: String,
    pub connection_id: Option<Uuid>,
    pub folder: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Stored query history row, returned by `search_history`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub connection_id: Option<String>,
    pub sql: String,
    pub started_at: String,
    pub duration_ms: i64,
    pub row_count: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
    pub cancelled: bool,
}

pub struct Store {
    pool: SqlitePool,
    secrets: SecretBox,
    key_check: [u8; 32],
    database_path: PathBuf,
    backups_dir: PathBuf,
}

impl Store {
    /// Opens (or creates) the app database, runs migrations, and resolves the
    /// master key. A blocked keyring, a missing key, and a different key are
    /// three distinct, safe failures.
    pub async fn open(
        path: impl AsRef<Path>,
        provider: &dyn MasterKeyProvider,
    ) -> Result<Self, UniDbError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                UniDbError::new(
                    "store_directory_failed",
                    ErrorCategory::Internal,
                    "Could not create the app data directory",
                )
                .with_source(error)
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(|error| {
                UniDbError::new(
                    "store_open_failed",
                    ErrorCategory::Internal,
                    "Could not open the app database",
                )
                .with_source(error)
            })?;

        let existing_database: bool = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'app_meta'",
        )
        .fetch_optional(&pool)
        .await
        .map_err(|error| {
            UniDbError::new(
                "store_open_failed",
                ErrorCategory::Internal,
                "Could not inspect the app database",
            )
            .with_source(error)
        })?
        .is_some();

        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .map_err(|error| {
                UniDbError::new(
                    "store_open_failed",
                    ErrorCategory::Internal,
                    "Could not check the app database",
                )
                .with_source(error)
            })?;
        if integrity != "ok" {
            return Err(UniDbError::new(
                "store_corrupt",
                ErrorCategory::Internal,
                "The app database is corrupt. Restore the newest file from backups/ and start again.",
            )
            .with_detail(integrity));
        }

        let backups_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups");
        std::fs::create_dir_all(&backups_dir).map_err(|error| {
            UniDbError::new(
                "store_backup_failed",
                ErrorCategory::Internal,
                "Could not create the backup directory",
            )
            .with_source(error)
        })?;
        if existing_database {
            let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&pool)
                .await;
            backup_database(path, &backups_dir)?;
            prune_backups(&backups_dir, 5)?;
        }

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|error| {
                UniDbError::new(
                    "store_migrate_failed",
                    ErrorCategory::Internal,
                    "Could not migrate the app database",
                )
                .with_source(error)
            })?;

        let stored_check: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT value FROM app_meta WHERE key = 'master_key_check'")
                .fetch_optional(&pool)
                .await
                .map_err(|error| {
                    UniDbError::new(
                        "store_open_failed",
                        ErrorCategory::Internal,
                        "Could not read the app database metadata",
                    )
                    .with_source(error)
                })?;

        let loaded = provider.load()?;
        let key = match (loaded, stored_check) {
            (Some(key), Some(stored)) => {
                if stored.as_slice() != key.fingerprint().as_slice() {
                    return Err(UniDbError::new(
                        "master_key_mismatch",
                        ErrorCategory::Internal,
                        "The system keyring holds a different key than this database. Saved secrets cannot be opened.",
                    ));
                }
                key
            }
            (Some(key), None) => {
                write_key_check(&pool, &key).await?;
                key
            }
            (None, Some(_)) => {
                return Err(UniDbError::new(
                    "master_key_missing",
                    ErrorCategory::Internal,
                    "The master key is missing from the system keyring. Saved secrets cannot be opened.",
                ));
            }
            (None, None) => {
                let key = MasterKey::generate();
                provider.store(&key)?;
                write_key_check(&pool, &key).await?;
                key
            }
        };

        let key_check = key.fingerprint();
        Ok(Self {
            pool,
            secrets: SecretBox::new(key),
            key_check,
            database_path: path.to_path_buf(),
            backups_dir,
        })
    }

    pub fn key_check(&self) -> &[u8; 32] {
        &self.key_check
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn backups_dir(&self) -> &Path {
        &self.backups_dir
    }

    /// Copies the database to the backups directory and rotates the old ones.
    pub async fn backup_now(&self) -> Result<PathBuf, UniDbError> {
        let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await;
        let backup = backup_database(&self.database_path, &self.backups_dir)?;
        prune_backups(&self.backups_dir, 5)?;
        Ok(backup)
    }

    /// Newest first.
    pub fn list_backups(&self) -> Result<Vec<PathBuf>, UniDbError> {
        let mut backups = std::fs::read_dir(&self.backups_dir)
            .map_err(|error| {
                UniDbError::new(
                    "store_backup_failed",
                    ErrorCategory::Internal,
                    "Could not read the backup directory",
                )
                .with_source(error)
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "db"))
            .collect::<Vec<_>>();
        backups.sort();
        backups.reverse();
        Ok(backups)
    }

    pub async fn connection_count(&self) -> Result<i64, UniDbError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM connections")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| UniDbError::internal("Could not count connections").with_source(error))
    }

    pub async fn encrypted_secret_count(&self) -> Result<i64, UniDbError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM connections WHERE password_ciphertext IS NOT NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                UniDbError::internal("Could not count saved secrets").with_source(error)
            })
    }

    pub async fn audit_count(&self) -> Result<i64, UniDbError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                UniDbError::internal("Could not count audit entries").with_source(error)
            })
    }

    /// Reads the previous run's shutdown state, then marks this run as in
    /// progress. Returns the timestamp of an unclean previous run.
    pub async fn begin_run(&self) -> Result<Option<String>, UniDbError> {
        let previous = self.previous_unclean_shutdown().await?;
        self.mark_running().await?;
        Ok(previous)
    }

    /// Records that a run started. Cleared by `mark_clean_shutdown`.
    pub async fn mark_running(&self) -> Result<(), UniDbError> {
        self.set_setting(
            "unclean_shutdown",
            serde_json::json!(Utc::now().to_rfc3339()),
        )
        .await
    }

    pub async fn mark_clean_shutdown(&self) -> Result<(), UniDbError> {
        self.set_setting("unclean_shutdown", serde_json::Value::Null)
            .await
    }

    /// Timestamp of the previous run when it did not shut down cleanly.
    pub async fn previous_unclean_shutdown(&self) -> Result<Option<String>, UniDbError> {
        match self.get_setting("unclean_shutdown").await? {
            Some(serde_json::Value::String(timestamp)) => Ok(Some(timestamp)),
            _ => Ok(None),
        }
    }

    /// Saves a profile. `password` is encrypted before it touches disk; passing
    /// `None` keeps the stored secret untouched.
    pub async fn save_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<(), UniDbError> {
        profile.validate()?;
        let now = Utc::now().to_rfc3339();
        let encrypted: Option<EncryptedSecret> = match password {
            Some(password) => Some(
                self.secrets
                    .encrypt(&connection_password_aad(profile.id), password.as_bytes())?,
            ),
            None => None,
        };

        let result = sqlx::query(
            "INSERT INTO connections (
                id, name, engine, host, port, database, username,
                password_ciphertext, password_nonce, ssl_mode, ssh_profile_id,
                read_only, color, group_name, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                engine = excluded.engine,
                host = excluded.host,
                port = excluded.port,
                database = excluded.database,
                username = excluded.username,
                password_ciphertext = COALESCE(excluded.password_ciphertext, connections.password_ciphertext),
                password_nonce = COALESCE(excluded.password_nonce, connections.password_nonce),
                ssl_mode = excluded.ssl_mode,
                ssh_profile_id = excluded.ssh_profile_id,
                read_only = excluded.read_only,
                color = excluded.color,
                group_name = excluded.group_name,
                updated_at = excluded.updated_at",
        )
        .bind(profile.id.to_string())
        .bind(&profile.name)
        .bind(profile.engine.display())
        .bind(&profile.host)
        .bind(i64::from(profile.port))
        .bind(&profile.database)
        .bind(&profile.username)
        .bind(encrypted.as_ref().map(|secret| secret.ciphertext.clone()))
        .bind(encrypted.as_ref().map(|secret| secret.nonce.to_vec()))
        .bind(ssl_to_db(profile.ssl_mode))
        .bind(profile.ssh_profile_id.map(|id| id.to_string()))
        .bind(i64::from(profile.read_only))
        .bind(profile.engine.color())
        .bind("")
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_connection_error)?;

        if result.rows_affected() == 0 {
            return Err(UniDbError::internal("Could not save the connection"));
        }
        Ok(())
    }

    pub async fn list_connections(&self) -> Result<Vec<ConnectionProfile>, UniDbError> {
        let rows = sqlx::query(
            "SELECT id, name, engine, host, port, database, username, ssl_mode,
                    ssh_profile_id, read_only
             FROM connections ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| UniDbError::internal("Could not list connections").with_source(error))?;

        rows.iter().map(row_to_profile).collect()
    }

    pub async fn get_connection(&self, id: Uuid) -> Result<Option<ConnectionProfile>, UniDbError> {
        let row = sqlx::query(
            "SELECT id, name, engine, host, port, database, username, ssl_mode,
                    ssh_profile_id, read_only
             FROM connections WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            UniDbError::internal("Could not read the connection").with_source(error)
        })?;
        row.as_ref().map(row_to_profile).transpose()
    }

    pub async fn delete_connection(&self, id: Uuid) -> Result<u64, UniDbError> {
        let result = sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|error| {
                UniDbError::internal("Could not delete the connection").with_source(error)
            })?;
        Ok(result.rows_affected())
    }

    /// Decrypts the saved password for a connection, if one exists.
    pub async fn get_password(&self, id: Uuid) -> Result<Option<Zeroizing<String>>, UniDbError> {
        let row =
            sqlx::query("SELECT password_ciphertext, password_nonce FROM connections WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| {
                    UniDbError::internal("Could not read the saved secret").with_source(error)
                })?;

        let Some(row) = row else {
            return Ok(None);
        };
        let ciphertext: Option<Vec<u8>> = row.try_get("password_ciphertext").map_err(|error| {
            UniDbError::internal("Could not read the saved secret").with_source(error)
        })?;
        let nonce: Option<Vec<u8>> = row.try_get("password_nonce").map_err(|error| {
            UniDbError::internal("Could not read the saved secret").with_source(error)
        })?;
        let (Some(ciphertext), Some(nonce)) = (ciphertext, nonce) else {
            return Ok(None);
        };
        let plaintext = self
            .secrets
            .decrypt(&connection_password_aad(id), &nonce, &ciphertext)?;
        let password = String::from_utf8(plaintext.to_vec()).map_err(|error| {
            UniDbError::new(
                "secret_not_utf8",
                ErrorCategory::Internal,
                "The saved secret is not valid text",
            )
            .with_source(error)
        })?;
        Ok(Some(Zeroizing::new(password)))
    }

    pub async fn set_setting(&self, key: &str, value: serde_json::Value) -> Result<(), UniDbError> {
        let encoded = serde_json::to_string(&value).map_err(|error| {
            UniDbError::internal("Could not encode the setting").with_source(error)
        })?;
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(encoded)
        .execute(&self.pool)
        .await
        .map_err(|error| UniDbError::internal("Could not save the setting").with_source(error))?;
        Ok(())
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<serde_json::Value>, UniDbError> {
        let encoded: Option<String> =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| {
                    UniDbError::internal("Could not read the setting").with_source(error)
                })?;
        encoded
            .map(|value| {
                serde_json::from_str(&value).map_err(|error| {
                    UniDbError::internal("The saved setting is not valid JSON").with_source(error)
                })
            })
            .transpose()
    }

    /// Saves a named statement and returns the stored row.
    pub async fn save_saved_query(
        &self,
        name: &str,
        sql: &str,
        connection_id: Option<Uuid>,
    ) -> Result<SavedQuery, UniDbError> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO saved_queries (id, name, sql, connection_id, folder, tags, created_at, updated_at)
             VALUES (?, ?, ?, ?, '', '[]', ?, ?)",
        )
        .bind(id.to_string())
        .bind(name)
        .bind(sql)
        .bind(connection_id.map(|value| value.to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            UniDbError::internal("Could not save the query").with_source(error)
        })?;
        Ok(SavedQuery {
            id,
            name: name.to_string(),
            sql: sql.to_string(),
            connection_id,
            folder: None,
            tags: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Lists saved queries newest first.
    pub async fn list_saved_queries(&self) -> Result<Vec<SavedQuery>, UniDbError> {
        let rows = sqlx::query(
            "SELECT id, name, sql, connection_id, folder, tags, created_at, updated_at
             FROM saved_queries ORDER BY created_at DESC, name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            UniDbError::internal("Could not list the saved queries").with_source(error)
        })?;
        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let connection: Option<String> = row.try_get("connection_id")?;
                let tags: String = row.try_get("tags")?;
                Ok(SavedQuery {
                    id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::nil()),
                    name: row.try_get("name")?,
                    sql: row.try_get("sql")?,
                    connection_id: connection.and_then(|value| Uuid::parse_str(&value).ok()),
                    folder: row
                        .try_get::<String, _>("folder")
                        .ok()
                        .filter(|value| !value.is_empty()),
                    tags: serde_json::from_str(&tags).unwrap_or_default(),
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(|error| {
                UniDbError::internal("Could not read the saved queries").with_source(error)
            })
    }

    /// Deletes one saved query and returns how many rows went away.
    pub async fn delete_saved_query(&self, id: Uuid) -> Result<u64, UniDbError> {
        let result = sqlx::query("DELETE FROM saved_queries WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|error| {
                UniDbError::internal("Could not delete the saved query").with_source(error)
            })?;
        Ok(result.rows_affected())
    }

    /// Deletes every history row and returns how many went away.
    pub async fn clear_history(&self) -> Result<u64, UniDbError> {
        let result = sqlx::query("DELETE FROM query_history")
            .execute(&self.pool)
            .await
            .map_err(|error| {
                UniDbError::internal("Could not clear the query history").with_source(error)
            })?;
        Ok(result.rows_affected())
    }

    pub async fn add_audit(
        &self,
        action: &str,
        connection_id: Option<Uuid>,
        outcome: &str,
        detail: serde_json::Value,
    ) -> Result<(), UniDbError> {
        let detail = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".to_string());
        sqlx::query(
            "INSERT INTO audit_log (ts, action, connection_id, outcome, detail_json)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(action)
        .bind(connection_id.map(|id| id.to_string()))
        .bind(outcome)
        .bind(detail)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            UniDbError::internal("Could not write the audit log").with_source(error)
        })?;
        Ok(())
    }

    pub async fn list_audit(&self, limit: i64) -> Result<Vec<AuditEntry>, UniDbError> {
        let rows = sqlx::query(
            "SELECT id, ts, action, connection_id, outcome, detail_json
             FROM audit_log ORDER BY id DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| UniDbError::internal("Could not read the audit log").with_source(error))?;

        rows.into_iter()
            .map(|row| {
                let detail_json: String = row.try_get("detail_json")?;
                Ok(AuditEntry {
                    id: row.try_get("id")?,
                    ts: row.try_get("ts")?,
                    action: row.try_get("action")?,
                    connection_id: row.try_get("connection_id")?,
                    outcome: row.try_get("outcome")?,
                    detail: serde_json::from_str(&detail_json)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(|error| {
                UniDbError::internal("Could not read the audit log").with_source(error)
            })
    }

    /// Records one executed statement. SQL text is kept so history works; it is
    /// never logged through tracing.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_history(
        &self,
        connection_id: Option<Uuid>,
        sql: &str,
        duration_ms: u64,
        row_count: Option<i64>,
        status: &str,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<i64, UniDbError> {
        let result = sqlx::query(
            "INSERT INTO query_history (
                connection_id, sql, started_at, duration_ms, row_count,
                status, error_code, error_message, cancelled
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(connection_id.map(|id| id.to_string()))
        .bind(sql)
        .bind(Utc::now().to_rfc3339())
        .bind(i64::try_from(duration_ms).unwrap_or(i64::MAX))
        .bind(row_count)
        .bind(status)
        .bind(error_code)
        .bind(error_message)
        .bind(0_i64)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            UniDbError::internal("Could not record the query history").with_source(error)
        })?;
        Ok(result.last_insert_rowid())
    }

    /// Full text search over history, newest first.
    pub async fn search_history(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<HistoryEntry>, UniDbError> {
        let rows = if query.trim().is_empty() {
            sqlx::query(
                "SELECT id, connection_id, sql, started_at, duration_ms, row_count,
                        status, error_code, cancelled
                 FROM query_history ORDER BY id DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                "SELECT h.id, h.connection_id, h.sql, h.started_at, h.duration_ms,
                        h.row_count, h.status, h.error_code, h.cancelled
                 FROM query_history_fts f
                 JOIN query_history h ON h.id = f.rowid
                 WHERE query_history_fts MATCH ?
                 ORDER BY h.id DESC LIMIT ?",
            )
            .bind(query)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|error| {
            UniDbError::internal("Could not search the query history").with_source(error)
        })?;

        rows.into_iter()
            .map(|row| {
                Ok(HistoryEntry {
                    id: row.try_get("id")?,
                    connection_id: row.try_get("connection_id")?,
                    sql: row.try_get("sql")?,
                    started_at: row.try_get("started_at")?,
                    duration_ms: row.try_get("duration_ms")?,
                    row_count: row.try_get("row_count")?,
                    status: row.try_get("status")?,
                    error_code: row.try_get("error_code")?,
                    cancelled: row.try_get::<i64, _>("cancelled")? != 0,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(|error| {
                UniDbError::internal("Could not read the query history").with_source(error)
            })
    }

    /// Deletes history older than `days`. Returns how many rows went away.
    pub async fn prune_history(&self, days: i64) -> Result<u64, UniDbError> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let result = sqlx::query("DELETE FROM query_history WHERE started_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                UniDbError::internal("Could not prune the query history").with_source(error)
            })?;
        Ok(result.rows_affected())
    }
}

fn backup_database(database: &Path, backups_dir: &Path) -> Result<PathBuf, UniDbError> {
    let stamp = Utc::now().format("%Y%m%d-%H%M%S-%3f");
    let destination = backups_dir.join(format!("unidb-{stamp}.db"));
    std::fs::copy(database, &destination).map_err(|error| {
        UniDbError::new(
            "store_backup_failed",
            ErrorCategory::Internal,
            "Could not back up the app database",
        )
        .with_source(error)
    })?;
    Ok(destination)
}

fn prune_backups(backups_dir: &Path, keep: usize) -> Result<(), UniDbError> {
    let mut backups = std::fs::read_dir(backups_dir)
        .map_err(|error| {
            UniDbError::new(
                "store_backup_failed",
                ErrorCategory::Internal,
                "Could not read the backup directory",
            )
            .with_source(error)
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "db"))
        .collect::<Vec<_>>();
    backups.sort();
    if backups.len() <= keep {
        return Ok(());
    }
    let remove_count = backups.len() - keep;
    for path in backups.into_iter().take(remove_count) {
        std::fs::remove_file(&path).map_err(|error| {
            UniDbError::new(
                "store_backup_failed",
                ErrorCategory::Internal,
                "Could not rotate an old backup",
            )
            .with_source(error)
        })?;
    }
    Ok(())
}

async fn write_key_check(pool: &SqlitePool, key: &MasterKey) -> Result<(), UniDbError> {
    sqlx::query(
        "INSERT INTO app_meta (key, value) VALUES ('master_key_check', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key.fingerprint().to_vec())
    .execute(pool)
    .await
    .map_err(|error| {
        UniDbError::internal("Could not save the master key check").with_source(error)
    })?;
    Ok(())
}

fn row_to_profile(row: &SqliteRow) -> Result<ConnectionProfile, UniDbError> {
    let id: String = row.try_get("id").map_err(row_error)?;
    let id = Uuid::parse_str(&id).map_err(|error| {
        UniDbError::internal("The saved connection id is invalid").with_source(error)
    })?;
    let engine_text: String = row.try_get("engine").map_err(row_error)?;
    let engine = engine_from_db(&engine_text)?;
    let ssl_text: String = row.try_get("ssl_mode").map_err(row_error)?;
    let ssh_profile_id: Option<String> = row.try_get("ssh_profile_id").map_err(row_error)?;
    let read_only: i64 = row.try_get("read_only").map_err(row_error)?;
    Ok(ConnectionProfile {
        id,
        name: row.try_get("name").map_err(row_error)?,
        engine,
        host: row.try_get("host").map_err(row_error)?,
        port: u16::try_from(row.try_get::<i64, _>("port").map_err(row_error)?).unwrap_or(0),
        database: row.try_get("database").map_err(row_error)?,
        username: row.try_get("username").map_err(row_error)?,
        ssl_mode: ssl_from_db(&ssl_text)?,
        ssh_profile_id: ssh_profile_id
            .map(|value| {
                Uuid::parse_str(&value).map_err(|error| {
                    UniDbError::internal("The saved SSH profile id is invalid").with_source(error)
                })
            })
            .transpose()?,
        read_only: read_only != 0,
    })
}

fn row_error(error: sqlx::Error) -> UniDbError {
    UniDbError::internal("Could not read the saved connection").with_source(error)
}

fn map_connection_error(error: sqlx::Error) -> UniDbError {
    if let Some(database_error) = error.as_database_error()
        && database_error.is_unique_violation()
    {
        return UniDbError::new(
            "connection_name_taken",
            ErrorCategory::Internal,
            "A connection with this name already exists",
        );
    }
    UniDbError::internal("Could not save the connection").with_source(error)
}

fn engine_from_db(name: &str) -> Result<Engine, UniDbError> {
    for engine in Engine::ALL {
        if engine.display() == name {
            return Ok(engine);
        }
    }
    Err(UniDbError::new(
        "engine_unknown",
        ErrorCategory::Internal,
        "The saved connection uses an unknown engine",
    ))
}

fn ssl_to_db(mode: SslMode) -> &'static str {
    match mode {
        SslMode::Disable => "disable",
        SslMode::Prefer => "prefer",
        SslMode::Require => "require",
        SslMode::VerifyCa => "verify_ca",
        SslMode::VerifyFull => "verify_full",
    }
}

fn ssl_from_db(value: &str) -> Result<SslMode, UniDbError> {
    match value {
        "disable" => Ok(SslMode::Disable),
        "prefer" => Ok(SslMode::Prefer),
        "require" => Ok(SslMode::Require),
        "verify_ca" => Ok(SslMode::VerifyCa),
        "verify_full" => Ok(SslMode::VerifyFull),
        other => Err(UniDbError::internal(format!(
            "The saved connection uses an unknown TLS mode: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_round_trips_through_its_display_name() {
        for engine in Engine::ALL {
            assert_eq!(
                engine_from_db(engine.display()).expect("known engine"),
                engine
            );
        }
    }

    #[test]
    fn every_ssl_mode_round_trips() {
        for mode in [
            SslMode::Disable,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ] {
            assert_eq!(ssl_from_db(ssl_to_db(mode)).expect("known mode"), mode);
        }
    }
}
