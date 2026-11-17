//! Redis connector. The session accepts a redis command line, runs it, and maps
//! the reply into the shared value model. Read only sessions allow only a known
//! set of read commands.

use std::time::Instant;

use async_trait::async_trait;
use redis::Value as RedisValue;

use unidb_core::{
    Capabilities, Column, ConnectionProfile, Engine, ErrorCategory, QueryResult, UniDbError, Value,
};

use crate::traits::{Connector, Session};
use crate::uri::encode_component;

pub struct RedisConnector;

impl RedisConnector {
    pub fn new() -> Self {
        Self
    }

    fn connection_url(profile: &ConnectionProfile, password: Option<&str>) -> String {
        let mut url = String::from("redis://");
        if let Some(password) = password {
            if !profile.username.is_empty() {
                url.push_str(&encode_component(&profile.username));
                url.push(':');
            }
            url.push_str(&encode_component(password));
            url.push('@');
        }
        url.push_str(&profile.host);
        url.push(':');
        url.push_str(&profile.port.to_string());
        url.push('/');
        if let Ok(database) = profile.database.parse::<u8>() {
            url.push_str(&database.to_string());
        }
        url
    }
}

impl Default for RedisConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for RedisConnector {
    fn engine(&self) -> Engine {
        Engine::Redis
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let client =
            redis::Client::open(Self::connection_url(profile, password)).map_err(|error| {
                UniDbError::new(
                    "redis_connect_failed",
                    ErrorCategory::Internal,
                    "Could not open the Redis connection",
                )
                .with_source(error)
            })?;
        let connection = client.get_connection_manager().await.map_err(|error| {
            UniDbError::new(
                "redis_connect_failed",
                ErrorCategory::Internal,
                "Could not connect to Redis",
            )
            .with_source(error)
        })?;
        Ok(Box::new(RedisSession {
            connection,
            capabilities: Engine::Redis.capabilities(),
            read_only: profile.read_only,
        }))
    }
}

pub struct RedisSession {
    connection: redis::aio::ConnectionManager,
    capabilities: Capabilities,
    read_only: bool,
}

#[async_trait]
impl Session for RedisSession {
    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    async fn execute(&mut self, sql: &str) -> Result<QueryResult, UniDbError> {
        let started = Instant::now();
        let parts = split_command(sql);
        let Some((command, arguments)) = parts.split_first() else {
            return Err(UniDbError::new(
                "sql_empty",
                ErrorCategory::Syntax,
                "Nothing to run",
            ));
        };
        if self.read_only && !is_read_command(command) {
            return Err(UniDbError::new(
                "read_only_blocked",
                ErrorCategory::Permission,
                "This connection is read only, so only read commands are allowed",
            ));
        }

        let mut redis_command = redis::cmd(command);
        for argument in arguments {
            redis_command.arg(argument);
        }
        let reply: RedisValue = redis_command
            .query_async(&mut self.connection)
            .await
            .map_err(|error| {
                UniDbError::new(
                    "redis_command_failed",
                    ErrorCategory::Internal,
                    "The Redis command failed",
                )
                .with_source(error)
            })?;
        Ok(map_reply(reply, started.elapsed().as_millis()))
    }

    async fn cancel(&self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "cancel_unsupported",
            ErrorCategory::Unsupported,
            "Redis cannot cancel a running command",
        ))
    }

    async fn begin(&mut self) -> Result<(), UniDbError> {
        let _: RedisValue = redis::cmd("MULTI")
            .query_async(&mut self.connection)
            .await
            .map_err(redis_error)?;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), UniDbError> {
        let _: RedisValue = redis::cmd("EXEC")
            .query_async(&mut self.connection)
            .await
            .map_err(redis_error)?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), UniDbError> {
        let _: RedisValue = redis::cmd("DISCARD")
            .query_async(&mut self.connection)
            .await
            .map_err(redis_error)?;
        Ok(())
    }

    async fn list_databases(&mut self) -> Result<Vec<String>, UniDbError> {
        Ok(vec!["0".to_string()])
    }

    async fn list_tables(&mut self, _database: &str) -> Result<Vec<String>, UniDbError> {
        let reply: RedisValue = redis::cmd("SCAN")
            .arg(0)
            .arg("COUNT")
            .arg(200)
            .query_async(&mut self.connection)
            .await
            .map_err(redis_error)?;
        let mut keys = Vec::new();
        if let RedisValue::Array(parts) = reply
            && let Some(RedisValue::Array(items)) = parts.get(1)
        {
            for item in items {
                keys.push(redis_value_text(item));
            }
        }
        keys.sort();
        Ok(keys)
    }
}

fn redis_value_text(value: &RedisValue) -> String {
    match value {
        RedisValue::BulkString(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        RedisValue::SimpleString(text) => text.clone(),
        RedisValue::Int(number) => number.to_string(),
        other => format!("{other:?}"),
    }
}

fn redis_error(error: redis::RedisError) -> UniDbError {
    UniDbError::new(
        "redis_command_failed",
        ErrorCategory::Internal,
        "The Redis command failed",
    )
    .with_source(error)
}

/// Splits a command line on whitespace, honoring single and double quotes.
fn split_command(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for character in input.trim().chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match quote {
            Some(active) => {
                if character == '\\' {
                    escaped = true;
                } else if character == active {
                    quote = None;
                } else {
                    current.push(character);
                }
            }
            None => {
                if character == '"' || character == '\'' {
                    quote = Some(character);
                } else if character.is_whitespace() {
                    if !current.is_empty() {
                        parts.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(character);
                }
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn is_read_command(command: &str) -> bool {
    matches!(
        command.to_ascii_lowercase().as_str(),
        "get"
            | "mget"
            | "getrange"
            | "strlen"
            | "exists"
            | "keys"
            | "scan"
            | "type"
            | "ttl"
            | "pttl"
            | "hget"
            | "hmget"
            | "hgetall"
            | "hkeys"
            | "hvals"
            | "hlen"
            | "hexists"
            | "lrange"
            | "llen"
            | "lindex"
            | "smembers"
            | "sismember"
            | "scard"
            | "srandmember"
            | "zrange"
            | "zrevrange"
            | "zscore"
            | "zcard"
            | "zrank"
            | "xrange"
            | "xrevrange"
            | "xlen"
            | "xinfo"
            | "dbsize"
            | "info"
            | "ping"
            | "echo"
            | "time"
            | "lastsave"
            | "memory"
            | "object"
    )
}

fn map_reply(reply: RedisValue, elapsed_ms: u128) -> QueryResult {
    let elapsed = u64::try_from(elapsed_ms).unwrap_or(u64::MAX);
    match reply {
        RedisValue::Nil => QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: None,
            elapsed_ms: elapsed,
        },
        RedisValue::Array(values) | RedisValue::Set(values) => {
            if values.iter().all(is_scalar) {
                QueryResult {
                    columns: vec![Column {
                        name: "value".to_string(),
                        type_name: "redis".to_string(),
                        nullable: false,
                    }],
                    rows: values
                        .into_iter()
                        .map(|value| vec![to_value(value)])
                        .collect(),
                    affected_rows: None,
                    elapsed_ms: elapsed,
                }
            } else {
                QueryResult {
                    columns: vec![Column {
                        name: "value".to_string(),
                        type_name: "redis".to_string(),
                        nullable: false,
                    }],
                    rows: vec![vec![to_value(RedisValue::Array(values))]],
                    affected_rows: None,
                    elapsed_ms: elapsed,
                }
            }
        }
        RedisValue::Map(pairs) => QueryResult {
            columns: vec![
                Column {
                    name: "key".to_string(),
                    type_name: "redis".to_string(),
                    nullable: false,
                },
                Column {
                    name: "value".to_string(),
                    type_name: "redis".to_string(),
                    nullable: false,
                },
            ],
            rows: pairs
                .into_iter()
                .map(|(key, value)| vec![to_value(key), to_value(value)])
                .collect(),
            affected_rows: None,
            elapsed_ms: elapsed,
        },
        RedisValue::Attribute { data, .. } => map_reply(*data, elapsed_ms),
        scalar => QueryResult {
            columns: vec![Column {
                name: "value".to_string(),
                type_name: "redis".to_string(),
                nullable: false,
            }],
            rows: vec![vec![to_value(scalar)]],
            affected_rows: None,
            elapsed_ms: elapsed,
        },
    }
}

fn is_scalar(value: &RedisValue) -> bool {
    !matches!(
        value,
        RedisValue::Array(_)
            | RedisValue::Set(_)
            | RedisValue::Map(_)
            | RedisValue::Attribute { .. }
    )
}

fn to_value(value: RedisValue) -> Value {
    match value {
        RedisValue::Nil => Value::Null,
        RedisValue::Int(number) => Value::Int(number),
        RedisValue::BulkString(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Value::Text(text),
            Err(error) => Value::Bytes(error.into_bytes()),
        },
        RedisValue::Okay => Value::Text("OK".to_string()),
        RedisValue::SimpleString(text) => Value::Text(text),
        RedisValue::Double(number) => Value::Float(number),
        RedisValue::Boolean(boolean) => Value::Bool(boolean),
        RedisValue::VerbatimString { text, .. } => Value::Text(text),
        RedisValue::Array(values) | RedisValue::Set(values) => {
            Value::Array(values.into_iter().map(to_value).collect())
        }
        RedisValue::Map(pairs) => {
            let mut object = serde_json::Map::new();
            for (key, item) in pairs {
                let key_text = match to_value(key) {
                    Value::Text(text) => text,
                    Value::Int(number) => number.to_string(),
                    other => other.display_text(),
                };
                object.insert(
                    key_text,
                    serde_json::to_value(to_value(item)).unwrap_or_default(),
                );
            }
            Value::Json(serde_json::Value::Object(object))
        }
        RedisValue::Attribute { data, .. } => to_value(*data),
        other => Value::Text(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_commands_with_quotes() {
        assert_eq!(split_command("SET key value"), vec!["SET", "key", "value"]);
        assert_eq!(
            split_command("SET greeting \"hello world\""),
            vec!["SET", "greeting", "hello world"]
        );
        assert_eq!(split_command("  GET   key  "), vec!["GET", "key"]);
    }

    #[test]
    fn read_only_command_list() {
        assert!(is_read_command("GET"));
        assert!(is_read_command("hgetall"));
        assert!(!is_read_command("SET"));
        assert!(!is_read_command("DEL"));
    }

    #[test]
    fn builds_the_url_with_credentials() {
        let mut profile = ConnectionProfile::new("R", Engine::Redis, "localhost", "2");
        profile.username = "default".to_string();
        let url = RedisConnector::connection_url(&profile, Some("p@ss"));
        assert_eq!(url, "redis://default:p%40ss@localhost:6379/2");
    }

    #[test]
    fn scalar_reply_becomes_one_cell() {
        let result = map_reply(RedisValue::Int(7), 1);
        assert_eq!(result.rows, vec![vec![Value::Int(7)]]);
    }
}
