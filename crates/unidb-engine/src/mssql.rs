//! Microsoft SQL Server connector through the pure Rust TDS client. TLS
//! verification is limited to trusting the server certificate in this build;
//! full certificate verification is a follow up item.

use std::time::Instant;

use async_trait::async_trait;
use tiberius::error::Error as TiberiusError;
use tiberius::{AuthMethod, Client, ColumnData, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use unidb_core::{
    Capabilities, Column, ConnectionProfile, Engine, ErrorCategory, QueryResult, SslMode,
    UniDbError, Value,
};

use crate::sql::is_query_statement;
use crate::traits::{Connector, Session};

type MssqlClient = Client<Compat<TcpStream>>;

pub struct MssqlConnector;

impl MssqlConnector {
    pub fn new() -> Self {
        Self
    }

    fn config(profile: &ConnectionProfile, password: Option<&str>) -> Config {
        let mut config = Config::new();
        config.host(&profile.host);
        config.port(profile.port);
        config.database(&profile.database);
        config.authentication(AuthMethod::sql_server(
            &profile.username,
            password.unwrap_or_default(),
        ));
        config.encryption(match profile.ssl_mode {
            SslMode::Disable => EncryptionLevel::NotSupported,
            SslMode::Prefer => EncryptionLevel::Off,
            SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull => EncryptionLevel::Required,
        });
        if matches!(profile.ssl_mode, SslMode::VerifyCa | SslMode::VerifyFull) {
            // The TDS client does not expose full chain verification here yet.
            config.trust_cert();
        }
        config
    }
}

impl Default for MssqlConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for MssqlConnector {
    fn engine(&self) -> Engine {
        Engine::MSSQL
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let address = format!("{}:{}", profile.host, profile.port);
        let stream = TcpStream::connect(&address).await.map_err(|error| {
            UniDbError::new(
                "mssql_connect_failed",
                ErrorCategory::Network,
                "Could not reach the SQL Server host",
            )
            .with_source(error)
        })?;
        let client = Client::connect(Self::config(profile, password), stream.compat_write())
            .await
            .map_err(|error| {
                UniDbError::new(
                    "mssql_connect_failed",
                    ErrorCategory::Internal,
                    "Could not connect to SQL Server",
                )
                .with_source(error)
            })?;
        Ok(Box::new(MssqlSession {
            client,
            capabilities: Engine::MSSQL.capabilities(),
            read_only: profile.read_only,
        }))
    }
}

pub struct MssqlSession {
    client: MssqlClient,
    capabilities: Capabilities,
    read_only: bool,
}

#[async_trait]
impl Session for MssqlSession {
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

        // simple_query runs a plain batch instead of sp_executesql, so temp
        // tables keep their session scope and multi statement batches behave
        // the way a database client user expects.
        let stream = self
            .client
            .simple_query(statement)
            .await
            .map_err(mssql_error)?;
        let rows = stream.into_first_result().await.map_err(mssql_error)?;

        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|column| Column {
                        name: column.name().to_string(),
                        type_name: "tds".to_string(),
                        nullable: true,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let values = rows
            .iter()
            .map(|row| {
                row.cells()
                    .map(|(_, data)| column_data_to_value(data))
                    .collect()
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows: values,
            affected_rows: None,
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }

    async fn cancel(&self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "cancel_unsupported",
            ErrorCategory::Unsupported,
            "SQL Server statements cannot be cancelled from this build",
        ))
    }

    async fn begin(&mut self) -> Result<(), UniDbError> {
        self.client
            .simple_query("BEGIN TRANSACTION")
            .await
            .map_err(mssql_error)?;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), UniDbError> {
        self.client
            .simple_query("COMMIT TRANSACTION")
            .await
            .map_err(mssql_error)?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), UniDbError> {
        self.client
            .simple_query("ROLLBACK TRANSACTION")
            .await
            .map_err(mssql_error)?;
        Ok(())
    }

    async fn list_databases(&mut self) -> Result<Vec<String>, UniDbError> {
        let rows = self
            .client
            .simple_query("SELECT name FROM sys.databases ORDER BY name")
            .await
            .map_err(mssql_error)?
            .into_first_result()
            .await
            .map_err(mssql_error)?;
        Ok(rows
            .iter()
            .filter_map(|row| row.get::<&str, _>(0).map(str::to_string))
            .collect())
    }

    async fn list_tables(&mut self, database: &str) -> Result<Vec<String>, UniDbError> {
        if !database
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(UniDbError::new(
                "schema_database_invalid",
                ErrorCategory::Unsupported,
                "The database name contains unsupported characters",
            ));
        }
        let statement = format!(
            "SELECT TABLE_SCHEMA + '.' + TABLE_NAME FROM [{database}].INFORMATION_SCHEMA.TABLES ORDER BY 1"
        );
        let rows = self
            .client
            .simple_query(statement)
            .await
            .map_err(mssql_error)?
            .into_first_result()
            .await
            .map_err(mssql_error)?;
        Ok(rows
            .iter()
            .filter_map(|row| row.get::<&str, _>(0).map(str::to_string))
            .collect())
    }
}

fn mssql_error(error: TiberiusError) -> UniDbError {
    UniDbError::new(
        "mssql_statement_failed",
        ErrorCategory::Internal,
        "The SQL Server statement failed",
    )
    .with_source(error)
}

fn column_data_to_value(data: &ColumnData<'_>) -> Value {
    match data {
        ColumnData::U8(value) => value.map_or(Value::Null, |item| Value::Int(i64::from(item))),
        ColumnData::I16(value) => value.map_or(Value::Null, |item| Value::Int(i64::from(item))),
        ColumnData::I32(value) => value.map_or(Value::Null, |item| Value::Int(i64::from(item))),
        ColumnData::I64(value) => value.map_or(Value::Null, Value::Int),
        ColumnData::F32(value) => value.map_or(Value::Null, |item| Value::Float(f64::from(item))),
        ColumnData::F64(value) => value.map_or(Value::Null, Value::Float),
        ColumnData::Bit(value) => value.map_or(Value::Null, Value::Bool),
        ColumnData::String(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(item.clone().into_owned())),
        ColumnData::Guid(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(item.to_string())),
        ColumnData::Binary(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Bytes(item.clone().into_owned())),
        ColumnData::Numeric(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::Xml(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::DateTime(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::SmallDateTime(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::Time(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::Date(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::DateTime2(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
        ColumnData::DateTimeOffset(value) => value
            .as_ref()
            .map_or(Value::Null, |item| Value::Text(format!("{item:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn maps_core_column_types() {
        assert_eq!(
            column_data_to_value(&ColumnData::I32(Some(4))),
            Value::Int(4)
        );
        assert_eq!(
            column_data_to_value(&ColumnData::String(Some(Cow::Borrowed("hi")))),
            Value::Text("hi".to_string())
        );
        assert_eq!(
            column_data_to_value(&ColumnData::Bit(Some(true))),
            Value::Bool(true)
        );
        assert_eq!(column_data_to_value(&ColumnData::I64(None)), Value::Null);
    }
}
