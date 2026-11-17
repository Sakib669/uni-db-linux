//! SQLite driver. One dedicated connection per session, so `BEGIN` and `COMMIT`
//! apply to the statements that follow. Row values are converted into the shared
//! `Value` model here and nowhere else.

use std::str::FromStr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::AssertSqlSafe;
use sqlx::Column as SqlxColumn;
use sqlx::Row;
use sqlx::TypeInfo;
use sqlx::ValueRef;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteRow,
};

use unidb_core::{
    Capabilities, ConnectionProfile, Engine, ErrorCategory, QueryResult, UniDbError, Value,
};

use crate::traits::{Connector, Session};

pub struct SqliteConnector;

impl SqliteConnector {
    pub fn new() -> Self {
        Self
    }

    fn connect_options(profile: &ConnectionProfile) -> Result<SqliteConnectOptions, UniDbError> {
        if profile.database.trim().is_empty() {
            return Err(UniDbError::new(
                "sqlite_path_required",
                ErrorCategory::Unsupported,
                "A database file path is required for SQLite",
            ));
        }
        let options = if profile.database == ":memory:" {
            SqliteConnectOptions::from_str("sqlite::memory:").map_err(sqlite_config_error)?
        } else {
            SqliteConnectOptions::new()
                .filename(&profile.database)
                .create_if_missing(true)
        };
        Ok(options
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .read_only(profile.read_only))
    }
}

impl Default for SqliteConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for SqliteConnector {
    fn engine(&self) -> Engine {
        Engine::SQLite
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        _password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let options = Self::connect_options(profile)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|error| sqlite_error("Could not open the SQLite database", error))?;
        let connection = pool
            .acquire()
            .await
            .map_err(|error| sqlite_error("Could not use the SQLite database", error))?;
        Ok(Box::new(SqliteSession {
            pool,
            connection,
            capabilities: Engine::SQLite.capabilities(),
            read_only: profile.read_only,
        }))
    }
}

pub struct SqliteSession {
    pool: SqlitePool,
    connection: sqlx::pool::PoolConnection<sqlx::Sqlite>,
    capabilities: Capabilities,
    read_only: bool,
}

#[async_trait]
impl Session for SqliteSession {
    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    async fn execute(&mut self, sql: &str) -> Result<QueryResult, UniDbError> {
        let started = Instant::now();
        let statement = sql.trim().trim_end_matches(';').trim();
        if statement.is_empty() {
            return Err(UniDbError::new(
                "sql_empty",
                ErrorCategory::Syntax,
                "Nothing to run",
            ));
        }
        let is_query = starts_with_keyword(statement, "select")
            || starts_with_keyword(statement, "with")
            || starts_with_keyword(statement, "pragma")
            || starts_with_keyword(statement, "explain")
            || starts_with_keyword(statement, "values");
        if self.read_only && !is_query {
            return Err(UniDbError::new(
                "read_only_blocked",
                ErrorCategory::Permission,
                "This connection is read only, so only queries are allowed",
            ));
        }

        if is_query {
            // The user typed this SQL; UniDB is a database client, so the
            // safety assertion is the product's core operation, not a bypass.
            let rows = sqlx::query(AssertSqlSafe(statement))
                .fetch_all(&mut *self.connection)
                .await
                .map_err(|error| sqlite_error("The query failed", error))?;
            let result = map_rows(&rows, started.elapsed().as_millis());
            Ok(result)
        } else {
            let result = sqlx::query(AssertSqlSafe(statement))
                .execute(&mut *self.connection)
                .await
                .map_err(|error| sqlite_error("The statement failed", error))?;
            Ok(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
                affected_rows: Some(result.rows_affected()),
                elapsed_ms: started.elapsed().as_millis() as u64,
            })
        }
    }

    async fn cancel(&self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "cancel_unsupported",
            ErrorCategory::Unsupported,
            "SQLite cannot cancel a running statement",
        ))
    }

    async fn begin(&mut self) -> Result<(), UniDbError> {
        sqlx::query("BEGIN")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| sqlite_error("Could not begin the transaction", error))?;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), UniDbError> {
        sqlx::query("COMMIT")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| sqlite_error("Could not commit the transaction", error))?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), UniDbError> {
        sqlx::query("ROLLBACK")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| sqlite_error("Could not roll back the transaction", error))?;
        Ok(())
    }

    async fn list_databases(&mut self) -> Result<Vec<String>, UniDbError> {
        Ok(vec!["main".to_string()])
    }

    async fn list_tables(&mut self, _database: &str) -> Result<Vec<String>, UniDbError> {
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|error| sqlite_error("Could not list the SQLite tables", error))
    }
}

impl SqliteSession {
    /// Unused pool handle kept so the connection's pool lives as long as the
    /// session; dropping the session closes both.
    #[allow(dead_code)]
    fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn map_rows(rows: &[SqliteRow], elapsed_ms: u128) -> QueryResult {
    let columns = rows
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .map(|column| unidb_core::Column {
                    name: column.name().to_string(),
                    type_name: column.type_info().name().to_string(),
                    nullable: true,
                })
                .collect()
        })
        .unwrap_or_default();

    let values = rows
        .iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|index| value_at(row, index))
                .collect()
        })
        .collect();

    QueryResult {
        columns,
        rows: values,
        affected_rows: None,
        elapsed_ms: u64::try_from(elapsed_ms).unwrap_or(u64::MAX),
    }
}

fn value_at(row: &SqliteRow, index: usize) -> Value {
    let Ok(raw) = row.try_get_raw(index) else {
        return Value::Text("<unreadable>".to_string());
    };
    if raw.is_null() {
        return Value::Null;
    }
    match raw.type_info().name() {
        "INTEGER" | "INT" => row.try_get::<i64, _>(index).map_or(Value::Null, Value::Int),
        "REAL" => row
            .try_get::<f64, _>(index)
            .map_or(Value::Null, Value::Float),
        "TEXT" => row
            .try_get::<String, _>(index)
            .map_or(Value::Null, Value::Text),
        "BLOB" => row
            .try_get::<Vec<u8>, _>(index)
            .map_or(Value::Null, Value::Bytes),
        "NUMERIC" => row.try_get::<i64, _>(index).map_or_else(
            |_| {
                row.try_get::<f64, _>(index)
                    .map_or(Value::Null, Value::Float)
            },
            Value::Int,
        ),
        _ => row
            .try_get::<String, _>(index)
            .map_or(Value::Null, Value::Text),
    }
}

fn starts_with_keyword(sql: &str, keyword: &str) -> bool {
    sql.get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
}

fn sqlite_config_error(error: sqlx::Error) -> UniDbError {
    UniDbError::internal("Could not configure the SQLite connection").with_source(error)
}

fn sqlite_error(message: &'static str, error: sqlx::Error) -> UniDbError {
    let detail = error
        .as_database_error()
        .map(|database_error| {
            format!(
                "database error {}",
                database_error.code().unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "driver error".to_string());
    UniDbError::new("sqlite_error", ErrorCategory::Internal, message)
        .with_detail(detail)
        .with_source(error)
}
