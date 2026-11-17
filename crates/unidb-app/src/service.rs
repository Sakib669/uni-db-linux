//! Application services: open connections through actors, run statements, and
//! keep history and the audit log current. One actor per open connection owns
//! the driver session and serializes commands for that connection.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use unidb_core::{ConnectionProfile, QueryResult, UniDbError};
use unidb_engine::{ConnectorRegistry, Session};
use unidb_store::Store;

use crate::bridge::Bridge;

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

/// The app service layer. Holds the runtime bridge, the store, the engine
/// registry, and the live connection actors.
pub struct AppService {
    bridge: Bridge,
    store: Arc<Store>,
    connectors: Arc<ConnectorRegistry>,
    connections: Mutex<HashMap<Uuid, ConnectionHandle>>,
}

impl AppService {
    pub fn new(bridge: Bridge, store: Arc<Store>, connectors: Arc<ConnectorRegistry>) -> Self {
        Self {
            bridge,
            store,
            connectors,
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// Opens a session, starts its actor, and records the audit entry.
    pub async fn open_connection(&self, profile: &ConnectionProfile) -> Result<(), UniDbError> {
        profile.validate()?;
        let connector = self.connectors.get(profile.engine)?;
        let password = self.store.get_password(profile.id).await?;
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
        self.store
            .add_audit(
                "connection_opened",
                Some(profile.id),
                "success",
                serde_json::json!({ "engine": profile.engine.display() }),
            )
            .await?;
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

        match &result {
            Ok(query_result) => {
                self.store
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
                self.store
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
        result
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
                self.store
                    .add_audit(
                        "connection_closed",
                        Some(connection_id),
                        "success",
                        serde_json::json!({}),
                    )
                    .await?;
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
