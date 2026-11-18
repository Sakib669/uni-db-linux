//! MySQL and MariaDB connector. Both speak the MySQL protocol, so one module
//! serves the two engines. Cancellation needs a second connection because
//! `KILL QUERY` is issued by another session.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow, MySqlSslMode};
use sqlx::pool::PoolConnection;
use sqlx::{AssertSqlSafe, Column as SqlxColumn, Row, TypeInfo, ValueRef};

use unidb_core::{
    Capabilities, ConnectionProfile, Engine, ErrorCategory, QueryResult, SslMode, UniDbError, Value,
};

use crate::sql::{driver_error, is_query_statement};
use crate::traits::{Connector, Session};

pub struct MySqlConnector;

impl MySqlConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MySqlConnector {
    fn default() -> Self {
        Self::new()
    }
}

fn mysql_ssl_mode(mode: SslMode) -> MySqlSslMode {
    match mode {
        SslMode::Disable => MySqlSslMode::Disabled,
        SslMode::Prefer => MySqlSslMode::Preferred,
        SslMode::Require => MySqlSslMode::Required,
        SslMode::VerifyCa => MySqlSslMode::VerifyCa,
        SslMode::VerifyFull => MySqlSslMode::VerifyIdentity,
    }
}

#[async_trait]
impl Connector for MySqlConnector {
    fn engine(&self) -> Engine {
        Engine::MySQL
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let mut options = MySqlConnectOptions::new()
            .host(&profile.host)
            .port(profile.port)
            .database(&profile.database)
            .username(&profile.username)
            .ssl_mode(mysql_ssl_mode(profile.ssl_mode));
        if let Some(password) = password {
            options = options.password(password);
        }

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(300))
            .connect_with(options)
            .await
            .map_err(|error| {
                driver_error("mysql_connect_failed", "Could not connect to MySQL", error)
            })?;

        let mut connection = pool.acquire().await.map_err(|error| {
            driver_error(
                "mysql_connect_failed",
                "Could not use the MySQL connection",
                error,
            )
        })?;

        if profile.read_only {
            sqlx::query("SET SESSION TRANSACTION READ ONLY")
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

        let connection_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| {
                driver_error(
                    "mysql_connect_failed",
                    "Could not identify the MySQL session",
                    error,
                )
            })?;

        Ok(Box::new(MySqlSession {
            pool,
            connection,
            capabilities: profile.engine.capabilities(),
            read_only: profile.read_only,
            connection_id,
            database: profile.database.clone(),
        }))
    }
}

pub struct MySqlSession {
    pool: MySqlPool,
    connection: PoolConnection<sqlx::MySql>,
    capabilities: Capabilities,
    read_only: bool,
    connection_id: u64,
    database: String,
}

#[async_trait]
impl Session for MySqlSession {
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
                .map_err(|error| driver_error("mysql_query_failed", "The query failed", error))?;
            Ok(map_rows(&rows, started.elapsed().as_millis()))
        } else {
            let result = sqlx::query(AssertSqlSafe(statement))
                .execute(&mut *self.connection)
                .await
                .map_err(|error| {
                    driver_error("mysql_statement_failed", "The statement failed", error)
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
        let command = format!("KILL QUERY {}", self.connection_id);
        let mut connection = self.pool.acquire().await.map_err(|error| {
            driver_error(
                "cancel_failed",
                "Could not open a connection to cancel the statement",
                error,
            )
        })?;
        sqlx::query(AssertSqlSafe(command))
            .execute(&mut *connection)
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

    async fn list_databases(&mut self) -> Result<Vec<String>, UniDbError> {
        Ok(vec![self.database.clone()])
    }

    async fn list_tables(&mut self, database: &str) -> Result<Vec<String>, UniDbError> {
        let target = if database.is_empty() {
            self.database.as_str()
        } else {
            database
        };
        if target != self.database {
            return Err(UniDbError::new(
                "database_switch_unsupported",
                ErrorCategory::Unsupported,
                "Switching databases is not available in this build; reconnect to the other database",
            ));
        }
        sqlx::query_scalar::<_, String>(
            "SELECT CONCAT(table_schema, '.', table_name) FROM information_schema.tables
             WHERE table_schema = ? ORDER BY 1",
        )
        .bind(target)
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|error| driver_error("schema_query_failed", "Could not list the tables", error))
    }

    async fn describe_table(
        &mut self,
        database: &str,
        table: &str,
    ) -> Result<Vec<unidb_core::TableColumnInfo>, UniDbError> {
        let (schema, table) = match table.split_once('.') {
            Some((schema, table)) => (schema.to_string(), table.to_string()),
            None => (database.to_string(), table.to_string()),
        };
        let rows = sqlx::query(
            "SELECT column_name, column_type, is_nullable, column_default, column_key
             FROM information_schema.columns
             WHERE table_schema = ? AND table_name = ?
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|error| {
            driver_error("schema_query_failed", "Could not describe the table", error)
        })?;
        let mut columns = Vec::new();
        for row in rows {
            use sqlx::Row;
            let name: String = row.try_get(0).map_err(|error| {
                driver_error("schema_query_failed", "Could not read a column name", error)
            })?;
            let type_name: String = row.try_get(1).unwrap_or_default();
            let nullable: String = row.try_get(2).unwrap_or_default();
            let default: Option<String> = row.try_get(3).unwrap_or(None);
            let key: String = row.try_get(4).unwrap_or_default();
            columns.push(unidb_core::TableColumnInfo {
                name,
                type_name,
                nullable: nullable == "YES",
                primary_key: key == "PRI",
                default,
            });
        }
        Ok(columns)
    }

    async fn list_foreign_keys(
        &mut self,
        database: &str,
        table: &str,
    ) -> Result<Vec<unidb_core::ForeignKeyInfo>, UniDbError> {
        let (schema, table) = match table.split_once('.') {
            Some((schema, table)) => (schema.to_string(), table.to_string()),
            None => (database.to_string(), table.to_string()),
        };
        let rows = sqlx::query(
            "SELECT column_name, referenced_table_name, referenced_column_name
             FROM information_schema.key_column_usage
             WHERE table_schema = ? AND table_name = ?
               AND referenced_table_name IS NOT NULL",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|error| {
            driver_error(
                "schema_query_failed",
                "Could not read the foreign keys",
                error,
            )
        })?;
        let mut keys = Vec::new();
        for row in rows {
            use sqlx::Row;
            let from_column: String = row.try_get(0).unwrap_or_default();
            let to_table: String = row.try_get(1).unwrap_or_default();
            let to_column: String = row.try_get(2).unwrap_or_default();
            keys.push(unidb_core::ForeignKeyInfo {
                from_table: table.clone(),
                from_column,
                to_table,
                to_column,
            });
        }
        Ok(keys)
    }
}

fn map_rows(rows: &[MySqlRow], elapsed_ms: u128) -> QueryResult {
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

fn value_at(row: &MySqlRow, index: usize) -> Value {
    let Ok(raw) = row.try_get_raw(index) else {
        return Value::Text("<unreadable>".to_string());
    };
    if raw.is_null() {
        return Value::Null;
    }
    let type_name = raw.type_info().name().to_string();
    let unsigned = type_name.contains("UNSIGNED");
    let base = type_name.split(' ').next().unwrap_or(&type_name);

    match base {
        "BOOLEAN" | "BOOL" => row
            .try_get::<bool, _>(index)
            .map_or(Value::Null, Value::Bool),
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" => {
            if unsigned {
                row.try_get::<u64, _>(index).map_or(Value::Null, |value| {
                    Value::Int(i64::try_from(value).unwrap_or(i64::MAX))
                })
            } else {
                row.try_get::<i64, _>(index).map_or(Value::Null, Value::Int)
            }
        }
        "FLOAT" | "DOUBLE" => row
            .try_get::<f64, _>(index)
            .map_or(Value::Null, Value::Float),
        "DECIMAL" => row
            .try_get::<rust_decimal::Decimal, _>(index)
            .map_or(Value::Null, Value::Decimal),
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => {
            row.try_get::<String, _>(index)
                .map_or(Value::Null, Value::Text)
        }
        "BINARY" | "VARBINARY" | "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" => row
            .try_get::<Vec<u8>, _>(index)
            .map_or(Value::Null, Value::Bytes),
        "JSON" => row.try_get::<String, _>(index).map_or(Value::Null, |text| {
            serde_json::from_str(&text).map_or(Value::Text(text), Value::Json)
        }),
        "DATETIME" | "TIMESTAMP" => {
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
        "TIME" => row
            .try_get::<chrono::NaiveTime, _>(index)
            .map_or(Value::Null, |value| Value::Text(value.to_string())),
        "BIT" => row
            .try_get::<Vec<u8>, _>(index)
            .map_or(Value::Null, Value::Bytes),
        other => fallback_value(row, index, other),
    }
}

fn fallback_value(row: &MySqlRow, index: usize, type_name: &str) -> Value {
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
        assert!(matches!(
            mysql_ssl_mode(SslMode::Disable),
            MySqlSslMode::Disabled
        ));
        assert!(matches!(
            mysql_ssl_mode(SslMode::VerifyFull),
            MySqlSslMode::VerifyIdentity
        ));
    }
}
