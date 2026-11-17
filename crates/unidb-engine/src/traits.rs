//! Driver traits. They are object safe through `async_trait` so the registry can
//! hold `Arc<dyn Connector>` and the app can hold `Box<dyn Session>` per
//! connection, keeping each engine crate independent.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use unidb_core::{Capabilities, ConnectionProfile, Engine, QueryResult, UniDbError};

#[async_trait]
pub trait Connector: Send + Sync {
    fn engine(&self) -> Engine;

    /// Connects, runs a trivial probe, and disconnects. Used by the test button.
    async fn test(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<(), UniDbError> {
        self.connect(profile, password).await.map(|_| ())
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<Box<dyn Session>, UniDbError>;
}

#[async_trait]
pub trait Session: Send {
    fn capabilities(&self) -> Capabilities;

    /// Runs one statement. Query statements return columns and rows; statements
    /// that change data return `affected_rows` instead.
    async fn execute(&mut self, sql: &str) -> Result<QueryResult, UniDbError>;

    /// Stops a running statement where the engine supports it.
    async fn cancel(&self) -> Result<(), UniDbError>;

    async fn begin(&mut self) -> Result<(), UniDbError>;

    async fn commit(&mut self) -> Result<(), UniDbError>;

    async fn rollback(&mut self) -> Result<(), UniDbError>;
}

/// Maps an engine to its connector. Engines are registered as they are built,
/// so a missing engine fails with a clear message instead of a panic.
#[derive(Default)]
pub struct ConnectorRegistry {
    connectors: HashMap<Engine, Arc<dyn Connector>>,
}

impl ConnectorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, connector: Arc<dyn Connector>) -> &mut Self {
        self.connectors.insert(connector.engine(), connector);
        self
    }

    pub fn get(&self, engine: Engine) -> Result<Arc<dyn Connector>, UniDbError> {
        self.connectors.get(&engine).cloned().ok_or_else(|| {
            unidb_core::UniDbError::new(
                "engine_not_installed",
                unidb_core::ErrorCategory::Unsupported,
                format!("{} is not available in this build yet", engine.display()),
            )
        })
    }

    pub fn engines(&self) -> Vec<Engine> {
        let mut engines: Vec<Engine> = self.connectors.keys().copied().collect();
        engines.sort_by_key(|engine| engine.display());
        engines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubConnector(Engine);

    #[async_trait]
    impl Connector for StubConnector {
        fn engine(&self) -> Engine {
            self.0
        }

        async fn connect(
            &self,
            _profile: &ConnectionProfile,
            _password: Option<&str>,
        ) -> Result<Box<dyn Session>, UniDbError> {
            Err(UniDbError::internal("stub"))
        }
    }

    #[test]
    fn registry_reports_missing_engines_and_lists_registered_ones() {
        let mut registry = ConnectorRegistry::new();
        registry.register(Arc::new(StubConnector(Engine::SQLite)));

        assert_eq!(registry.engines(), vec![Engine::SQLite]);
        assert!(registry.get(Engine::SQLite).is_ok());
        let missing = match registry.get(Engine::PostgreSQL) {
            Ok(_) => panic!("postgres should not be registered"),
            Err(error) => error,
        };
        assert_eq!(missing.code(), "engine_not_installed");
    }
}
