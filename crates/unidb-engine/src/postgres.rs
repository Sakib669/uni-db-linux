//! PostgreSQL connector. It also serves CockroachDB, which speaks the
//! PostgreSQL wire protocol, so there is no second implementation to maintain.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow, PgSslMode};
use sqlx::{AssertSqlSafe, Column as SqlxColumn, Row, TypeInfo, ValueRef};

use unidb_core::{
    Capabilities, ConnectionProfile, Engine, ErrorCategory, QueryResult, SslMode, UniDbError, Value,
};

use crate::sql::{driver_error, is_query_statement};
use crate::traits::{Connector, Session};

pub struct PostgresConnector;

impl PostgresConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PostgresConnector {
    fn default() -> Self {
        Self::new()
    }
}

fn pg_ssl_mode(mode: SslMode) -> PgSslMode {
    match mode {
        SslMode::Disable => PgSslMode::Disable,
        SslMode::Prefer => PgSslMode::Prefer,
        SslMode::Require => PgSslMode::Require,
        SslMode::VerifyCa => PgSslMode::VerifyCa,
        SslMode::VerifyFull => PgSslMode::VerifyFull,
    }
}

#[async_trait]
impl Connector for PostgresConnector {
    fn engine(&self) -> Engine {
        Engine::PostgreSQL
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let mut options = PgConnectOptions::new()
            .host(&profile.host)
            .port(profile.port)
            .database(&profile.database)
            .username(&profile.username)
            .application_name("UniDB")
            .ssl_mode(pg_ssl_mode(profile.ssl_mode));
        if let Some(password) = password {
            options = options.password(password);
        }

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(300))
            .connect_with(options)
            .await
            .map_err(|error| {
                driver_error(
                    "postgres_connect_failed",
                    "Could not connect to PostgreSQL",
                    error,
                )
            })?;

        let mut connection = pool.acquire().await.map_err(|error| {
            driver_error(
                "postgres_connect_failed",
                "Could not use the PostgreSQL connection",
                error,
            )
        })?;

        if profile.read_only {
            sqlx::query("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
                .execute(&mut *connection)
                .await
                .map_err(|error| {
                    driver_error(
                        "read_only_setup_failed",
                        "Could not set the session read only",
                        error,
                    )
                })?;
        }

        // CockroachDB reports this as INT8, PostgreSQL as INT4.
        let backend_pid: i64 = sqlx::query_scalar("SELECT pg_backend_pid()::bigint")
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| {
                driver_error(
                    "postgres_connect_failed",
                    "Could not identify the PostgreSQL session",
                    error,
                )
            })?;
        let backend_pid = i32::try_from(backend_pid).unwrap_or_default();

        Ok(Box::new(PostgresSession {
            pool,
            connection,
            capabilities: profile.engine.capabilities(),
            read_only: profile.read_only,
            backend_pid,
        }))
    }
}

pub struct PostgresSession {
    pool: PgPool,
    connection: PoolConnection<sqlx::Postgres>,
    capabilities: Capabilities,
    read_only: bool,
    backend_pid: i32,
}

#[async_trait]
impl Session for PostgresSession {
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
        let is_query = is_query_statement(statement);
        if self.read_only && !is_query {
            return Err(UniDbError::new(
                "read_only_blocked",
                ErrorCategory::Permission,
                "This connection is read only, so only queries are allowed",
            ));
        }

        if is_query {
            let rows = sqlx::query(AssertSqlSafe(statement))
                .fetch_all(&mut *self.connection)
                .await
                .map_err(|error| {
                    driver_error("postgres_query_failed", "The query failed", error)
                })?;
            Ok(map_rows(&rows, started.elapsed().as_millis()))
        } else {
            let result = sqlx::query(AssertSqlSafe(statement))
                .execute(&mut *self.connection)
                .await
                .map_err(|error| {
                    driver_error("postgres_statement_failed", "The statement failed", error)
                })?;
            Ok(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
                affected_rows: Some(result.rows_affected()),
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            })
        }
    }

    async fn cancel(&self) -> Result<(), UniDbError> {
        sqlx::query("SELECT pg_cancel_backend($1)")
            .bind(self.backend_pid)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                driver_error(
                    "cancel_failed",
                    "Could not cancel the running statement",
                    error,
                )
            })?;
        Ok(())
    }

    async fn begin(&mut self) -> Result<(), UniDbError> {
        sqlx::query("BEGIN")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| {
                driver_error(
                    "transaction_failed",
                    "Could not begin the transaction",
                    error,
                )
            })?;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), UniDbError> {
        sqlx::query("COMMIT")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| {
                driver_error(
                    "transaction_failed",
                    "Could not commit the transaction",
                    error,
                )
            })?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), UniDbError> {
        sqlx::query("ROLLBACK")
            .execute(&mut *self.connection)
            .await
            .map_err(|error| {
                driver_error(
                    "transaction_failed",
                    "Could not roll back the transaction",
                    error,
                )
            })?;
        Ok(())
    }
}

fn map_rows(rows: &[PgRow], elapsed_ms: u128) -> QueryResult {
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

fn value_at(row: &PgRow, index: usize) -> Value {
    let Ok(raw) = row.try_get_raw(index) else {
        return Value::Text("<unreadable>".to_string());
    };
    if raw.is_null() {
        return Value::Null;
    }
    let type_name = raw.type_info().name().to_string();
    match type_name.as_str() {
        "BOOL" => row
            .try_get::<bool, _>(index)
            .map_or(Value::Null, Value::Bool),
        "INT2" => row
            .try_get::<i16, _>(index)
            .map_or(Value::Null, |value| Value::Int(i64::from(value))),
        "INT4" => row
            .try_get::<i32, _>(index)
            .map_or(Value::Null, |value| Value::Int(i64::from(value))),
        "INT8" => row.try_get::<i64, _>(index).map_or(Value::Null, Value::Int),
        "FLOAT4" => row
            .try_get::<f32, _>(index)
            .map_or(Value::Null, |value| Value::Float(f64::from(value))),
        "FLOAT8" => row
            .try_get::<f64, _>(index)
            .map_or(Value::Null, Value::Float),
        "NUMERIC" => row
            .try_get::<rust_decimal::Decimal, _>(index)
            .map_or(Value::Null, Value::Decimal),
        "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "UNKNOWN" => row
            .try_get::<String, _>(index)
            .map_or(Value::Null, Value::Text),
        "BYTEA" => row
            .try_get::<Vec<u8>, _>(index)
            .map_or(Value::Null, Value::Bytes),
        "JSON" | "JSONB" => row
            .try_get::<serde_json::Value, _>(index)
            .map_or(Value::Null, Value::Json),
        "UUID" => row
            .try_get::<uuid::Uuid, _>(index)
            .map_or(Value::Null, Value::Uuid),
        "TIMESTAMPTZ" => row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(index)
            .map_or(Value::Null, Value::Timestamp),
        "TIMESTAMP" => {
            row.try_get::<chrono::NaiveDateTime, _>(index)
                .map_or(Value::Null, |value| {
                    Value::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
                        value,
                        chrono::Utc,
                    ))
                })
        }
        "DATE" => row
            .try_get::<chrono::NaiveDate, _>(index)
            .map_or(Value::Null, |value| {
                let timestamp = value.and_hms_opt(0, 0, 0).unwrap_or_default();
                Value::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
                    timestamp,
                    chrono::Utc,
                ))
            }),
        other => fallback_value(row, index, other),
    }
}

fn fallback_value(row: &PgRow, index: usize, type_name: &str) -> Value {
    if let Ok(value) = row.try_get::<i64, _>(index) {
        return Value::Int(value);
    }
    if let Ok(value) = row.try_get::<f64, _>(index) {
        return Value::Float(value);
    }
    if let Ok(value) = row.try_get::<String, _>(index) {
        return Value::Text(value);
    }
    if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
        return Value::Bytes(value);
    }
    Value::Text(format!("<{type_name}>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssl_modes_map_one_to_one() {
        assert!(matches!(pg_ssl_mode(SslMode::Disable), PgSslMode::Disable));
        assert!(matches!(
            pg_ssl_mode(SslMode::VerifyFull),
            PgSslMode::VerifyFull
        ));
    }
}
