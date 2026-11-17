//! Cassandra connector through the scylla driver. CQL statements return the
//! shared value model. Transactions and cancellation are not supported by CQL
//! the way SQL engines support them, and the capability flags say so.

use std::time::Instant;

use async_trait::async_trait;
use scylla::client::session::Session as CqlSession;
use scylla::client::session_builder::SessionBuilder;
use scylla::value::{CqlValue, Row};

use unidb_core::{
    Capabilities, Column, ConnectionProfile, Engine, ErrorCategory, QueryResult, UniDbError, Value,
};

use crate::sql::is_query_statement;
use crate::traits::{Connector, Session};

pub struct CassandraConnector;

impl CassandraConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CassandraConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for CassandraConnector {
    fn engine(&self) -> Engine {
        Engine::Cassandra
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let node = format!("{}:{}", profile.host, profile.port);
        let mut builder = SessionBuilder::new().known_node(node);
        if let Some(password) = password
            && !profile.username.is_empty()
        {
            builder = builder.user(&profile.username, password);
        }
        let session = builder.build().await.map_err(|error| {
            UniDbError::new(
                "cassandra_connect_failed",
                ErrorCategory::Internal,
                "Could not connect to Cassandra",
            )
            .with_source(error)
        })?;
        Ok(Box::new(CassandraSession {
            session,
            capabilities: Engine::Cassandra.capabilities(),
            read_only: profile.read_only,
        }))
    }
}

pub struct CassandraSession {
    session: CqlSession,
    capabilities: Capabilities,
    read_only: bool,
}

#[async_trait]
impl Session for CassandraSession {
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
        if self.read_only && !is_query_statement(statement) {
            return Err(UniDbError::new(
                "read_only_blocked",
                ErrorCategory::Permission,
                "This connection is read only, so only queries are allowed",
            ));
        }

        let result = self
            .session
            .query_unpaged(statement, &[])
            .await
            .map_err(|error| {
                UniDbError::new(
                    "cassandra_query_failed",
                    ErrorCategory::Internal,
                    "The CQL statement failed",
                )
                .with_source(error)
            })?;

        if !is_query_statement(statement) {
            return Ok(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
                affected_rows: None,
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
        }

        let rows_result = result.into_rows_result().map_err(|error| {
            UniDbError::new(
                "cassandra_result_invalid",
                ErrorCategory::Internal,
                "The CQL result could not be read",
            )
            .with_source(error)
        })?;

        let columns: Vec<Column> = rows_result
            .column_specs()
            .iter()
            .map(|spec| Column {
                name: spec.name().to_string(),
                type_name: format!("{:?}", spec.typ()),
                nullable: true,
            })
            .collect();

        let mut rows = Vec::new();
        let iterator = rows_result.rows::<Row>().map_err(|error| {
            UniDbError::new(
                "cassandra_result_invalid",
                ErrorCategory::Internal,
                "The CQL result columns could not be read",
            )
            .with_source(error)
        })?;
        for row in iterator {
            let row = row.map_err(|error| {
                UniDbError::new(
                    "cassandra_result_invalid",
                    ErrorCategory::Internal,
                    "A CQL row could not be read",
                )
                .with_source(error)
            })?;
            rows.push(
                row.columns
                    .into_iter()
                    .map(|value| value.map_or(Value::Null, cql_to_value))
                    .collect(),
            );
        }

        Ok(QueryResult {
            columns,
            rows,
            affected_rows: None,
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }

    async fn cancel(&self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "cancel_unsupported",
            ErrorCategory::Unsupported,
            "Cassandra cannot cancel a running statement",
        ))
    }

    async fn begin(&mut self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "transactions_unsupported",
            ErrorCategory::Unsupported,
            "CQL does not support SQL style transactions",
        ))
    }

    async fn commit(&mut self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "transactions_unsupported",
            ErrorCategory::Unsupported,
            "CQL does not support SQL style transactions",
        ))
    }

    async fn rollback(&mut self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "transactions_unsupported",
            ErrorCategory::Unsupported,
            "CQL does not support SQL style transactions",
        ))
    }
}

fn cql_to_value(value: CqlValue) -> Value {
    match value {
        CqlValue::Ascii(text) | CqlValue::Text(text) => Value::Text(text),
        CqlValue::Boolean(boolean) => Value::Bool(boolean),
        CqlValue::Blob(bytes) => Value::Bytes(bytes),
        CqlValue::Counter(counter) => Value::Int(counter.0),
        CqlValue::Decimal(decimal) => Value::Text(format!("{decimal:?}")),
        CqlValue::Date(date) => Value::Text(format!("{date:?}")),
        CqlValue::Double(number) => Value::Float(number),
        CqlValue::Duration(duration) => Value::Text(format!("{duration:?}")),
        CqlValue::Empty => Value::Null,
        CqlValue::Float(number) => Value::Float(f64::from(number)),
        CqlValue::Int(number) => Value::Int(i64::from(number)),
        CqlValue::BigInt(number) => Value::Int(number),
        CqlValue::SmallInt(number) => Value::Int(i64::from(number)),
        CqlValue::TinyInt(number) => Value::Int(i64::from(number)),
        CqlValue::Timestamp(timestamp) => chrono::DateTime::from_timestamp_millis(timestamp.0)
            .map_or_else(|| Value::Text(timestamp.0.to_string()), Value::Timestamp),
        CqlValue::Inet(address) => Value::Text(address.to_string()),
        CqlValue::Uuid(uuid) => Value::Uuid(uuid),
        CqlValue::Timeuuid(timeuuid) => Value::Text(format!("{timeuuid:?}")),
        CqlValue::Varint(varint) => Value::Text(format!("{varint:?}")),
        CqlValue::Time(time) => Value::Text(format!("{time:?}")),
        CqlValue::List(values) | CqlValue::Set(values) | CqlValue::Vector(values) => {
            Value::Array(values.into_iter().map(cql_to_value).collect())
        }
        CqlValue::Map(pairs) => Value::Array(
            pairs
                .into_iter()
                .map(|(key, value)| Value::Array(vec![cql_to_value(key), cql_to_value(value)]))
                .collect(),
        ),
        CqlValue::Tuple(values) => Value::Array(
            values
                .into_iter()
                .map(|value| value.map_or(Value::Null, cql_to_value))
                .collect(),
        ),
        CqlValue::UserDefinedType { fields, .. } => Value::Array(
            fields
                .into_iter()
                .map(|(_, value)| value.map_or(Value::Null, cql_to_value))
                .collect(),
        ),
        other => Value::Text(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_scalar_cql_values() {
        assert_eq!(cql_to_value(CqlValue::Int(7)), Value::Int(7));
        assert_eq!(
            cql_to_value(CqlValue::Text("hi".to_string())),
            Value::Text("hi".to_string())
        );
        assert_eq!(cql_to_value(CqlValue::Boolean(true)), Value::Bool(true));
    }
}
