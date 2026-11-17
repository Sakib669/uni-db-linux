//! Application services: open connections through actors, run statements, and
//! keep history and the audit log current. One actor per open connection owns
//! the driver session and serializes commands for that connection.
//!
//! A service can also run without an app store (`headless`), which keeps the
//! scaffold probe possible before the connection manager exists and keeps the
//! future test harness free of keyring state.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use unidb_core::{ConnectionProfile, Engine, QueryResult, UniDbError};
use unidb_engine::{ConnectorRegistry, Session, SqliteConnector};
use unidb_store::Store;

use crate::bridge::{Bridge, QueryEvent};

enum Command {
    Execute {
        sql: String,
        reply: oneshot::Sender<Result<QueryResult, UniDbError>>,
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
}

impl AppService {
    pub fn new(bridge: Bridge, store: Arc<Store>, connectors: Arc<ConnectorRegistry>) -> Self {
        Self {
            bridge,
            store: Some(store),
            connectors,
            connections: Mutex::new(HashMap::new()),
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
        }
    }

    /// The scaffold service: runtime bridge plus the engines that are built.
    pub fn bootstrap_headless(runtime: Handle) -> Self {
        let mut connectors = ConnectorRegistry::new();
        connectors.register(Arc::new(SqliteConnector::new()));
        Self::headless(Bridge::new(runtime), Arc::new(connectors))
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
            match &result {
                Ok(query_result) => {
                    store
                        .record_history(
                            Some(connection_id),
                            sql,
                            query_result.elapsed_ms,
                            Some(i64::try_from(query_result.rows.len()).unwrap_or(i64::MAX)),
                            "success",
                            None,
                            None,
                        )
                        .await?;
                }
                Err(error) => {
                    store
                        .record_history(
                            Some(connection_id),
                            sql,
                            0,
                            None,
                            "error",
                            Some(error.code()),
                            Some(error.user_message()),
                        )
                        .await?;
                }
            }
        }
        result
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
                Command::Shutdown => break,
            }
        }
        tracing::debug!(connection_id = %self.connection_id, "connection actor stopped");
    }
}
