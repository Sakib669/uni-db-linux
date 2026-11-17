//! MongoDB connector. The session accepts a JSON database command, runs it, and
//! returns the reply as one JSON value. This is the honest v1 shape before the
//! collection browser exists: MongoDB has no SQL dialect, so a command document
//! is the natural unit.

use async_trait::async_trait;
use mongodb::bson::{Document, doc};
use mongodb::{Client, Database};

use unidb_core::{
    Capabilities, Column, ConnectionProfile, Engine, ErrorCategory, QueryResult, UniDbError, Value,
};

use crate::traits::{Connector, Session};
use crate::uri::encode_component;

pub struct MongoConnector;

impl MongoConnector {
    pub fn new() -> Self {
        Self
    }

    fn connection_uri(profile: &ConnectionProfile, password: Option<&str>) -> String {
        let mut uri = String::from("mongodb://");
        if let Some(password) = password {
            if !profile.username.is_empty() {
                uri.push_str(&encode_component(&profile.username));
                uri.push(':');
            }
            uri.push_str(&encode_component(password));
            uri.push('@');
        }
        uri.push_str(&profile.host);
        uri.push(':');
        uri.push_str(&profile.port.to_string());
        uri.push_str("/?directConnection=true&serverSelectionTimeoutMS=10000&appName=UniDB");
        uri
    }

    fn database(profile: &ConnectionProfile) -> String {
        if profile.database.trim().is_empty() {
            "admin".to_string()
        } else {
            profile.database.clone()
        }
    }
}

impl Default for MongoConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for MongoConnector {
    fn engine(&self) -> Engine {
        Engine::MongoDB
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError> {
        profile.validate()?;
        let client = Client::with_uri_str(Self::connection_uri(profile, password))
            .await
            .map_err(|error| {
                UniDbError::new(
                    "mongodb_connect_failed",
                    ErrorCategory::Internal,
                    "Could not connect to MongoDB",
                )
                .with_source(error)
            })?;
        let database_name = Self::database(profile);
        let database = client.database(&database_name);
        database
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|error| {
                UniDbError::new(
                    "mongodb_connect_failed",
                    ErrorCategory::Internal,
                    "MongoDB did not answer the ping",
                )
                .with_source(error)
            })?;
        Ok(Box::new(MongoSession {
            client,
            database,
            capabilities: Engine::MongoDB.capabilities(),
            read_only: profile.read_only,
            database_name,
        }))
    }
}

pub struct MongoSession {
    client: Client,
    database: Database,
    capabilities: Capabilities,
    read_only: bool,
    database_name: String,
}

#[async_trait]
impl Session for MongoSession {
    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    async fn execute(&mut self, sql: &str) -> Result<QueryResult, UniDbError> {
        let started = std::time::Instant::now();
        let command = parse_command(sql)?;
        let command_name = command.keys().next().cloned().unwrap_or_default();
        if self.read_only && !is_read_command(&command_name) {
            return Err(UniDbError::new(
                "read_only_blocked",
                ErrorCategory::Permission,
                "This connection is read only, so only read commands are allowed",
            ));
        }

        let reply = self.database.run_command(command).await.map_err(|error| {
            UniDbError::new(
                "mongodb_command_failed",
                ErrorCategory::Internal,
                "The MongoDB command failed",
            )
            .with_source(error)
        })?;
        let json = serde_json::to_value(&reply).unwrap_or(serde_json::Value::Null);
        Ok(QueryResult {
            columns: vec![Column {
                name: command_name,
                type_name: "document".to_string(),
                nullable: false,
            }],
            rows: vec![vec![Value::Json(json)]],
            affected_rows: None,
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }

    async fn cancel(&self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "cancel_unsupported",
            ErrorCategory::Unsupported,
            "MongoDB commands cannot be cancelled from this build",
        ))
    }

    async fn begin(&mut self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "transactions_unsupported",
            ErrorCategory::Unsupported,
            "MongoDB transactions are not available in this build",
        ))
    }

    async fn commit(&mut self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "transactions_unsupported",
            ErrorCategory::Unsupported,
            "MongoDB transactions are not available in this build",
        ))
    }

    async fn rollback(&mut self) -> Result<(), UniDbError> {
        Err(UniDbError::new(
            "transactions_unsupported",
            ErrorCategory::Unsupported,
            "MongoDB transactions are not available in this build",
        ))
    }

    async fn list_databases(&mut self) -> Result<Vec<String>, UniDbError> {
        self.client.list_database_names().await.map_err(|error| {
            UniDbError::internal("Could not list the MongoDB databases").with_source(error)
        })
    }

    async fn list_tables(&mut self, database: &str) -> Result<Vec<String>, UniDbError> {
        let _ = &self.database_name;
        self.client
            .database(database)
            .list_collection_names()
            .await
            .map_err(|error| {
                UniDbError::internal("Could not list the MongoDB collections").with_source(error)
            })
    }
}

fn parse_command(sql: &str) -> Result<Document, UniDbError> {
    let json: serde_json::Value = serde_json::from_str(sql.trim()).map_err(|error| {
        UniDbError::new(
            "mongodb_command_invalid",
            ErrorCategory::Syntax,
            "MongoDB sessions accept a JSON command document",
        )
        .with_detail(format!("json parse error at line {}", error.line()))
    })?;
    let bson = mongodb::bson::to_bson(&json).map_err(|error| {
        UniDbError::new(
            "mongodb_command_invalid",
            ErrorCategory::Syntax,
            "The command document could not be converted to BSON",
        )
        .with_source(error)
    })?;
    bson.as_document().cloned().ok_or_else(|| {
        UniDbError::new(
            "mongodb_command_invalid",
            ErrorCategory::Syntax,
            "The command must be a JSON object",
        )
    })
}

fn is_read_command(command: &str) -> bool {
    matches!(
        command,
        "find"
            | "aggregate"
            | "count"
            | "distinct"
            | "listCollections"
            | "listDatabases"
            | "listIndexes"
            | "dbStats"
            | "collStats"
            | "explain"
            | "ping"
            | "hello"
            | "isMaster"
            | "buildInfo"
            | "serverStatus"
            | "connectionStatus"
    )
}

/// Kept for the client field so the connection lives as long as the session.
impl MongoSession {
    #[allow(dead_code)]
    fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_json_command() {
        let document = parse_command(r#"{"find": "users", "filter": {"active": true}}"#)
            .expect("valid command");
        assert_eq!(document.keys().next().map(String::as_str), Some("find"));
    }

    #[test]
    fn rejects_non_json_input() {
        let error = parse_command("SELECT * FROM users").expect_err("sql is not a command");
        assert_eq!(error.code(), "mongodb_command_invalid");
    }

    #[test]
    fn builds_the_uri_with_credentials() {
        let mut profile = ConnectionProfile::new("M", Engine::MongoDB, "localhost", "admin");
        profile.username = "ada".to_string();
        let uri = MongoConnector::connection_uri(&profile, Some("p@ss"));
        assert!(uri.contains("ada:p%40ss@"));
        assert!(uri.contains("directConnection=true"));
    }

    #[test]
    fn read_command_classification() {
        assert!(is_read_command("find"));
        assert!(!is_read_command("insert"));
        assert!(!is_read_command("create"));
    }
}
