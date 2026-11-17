//! Live container tests for the Redis and MongoDB connectors.

use std::sync::OnceLock;
use std::time::Duration;

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};

use unidb_core::{ConnectionProfile, Engine, SslMode, Value};
use unidb_engine::{Connector, MongoConnector, RedisConnector};

fn container_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn connect_with_retry(
    connector: &dyn Connector,
    profile: &ConnectionProfile,
) -> Box<dyn unidb_engine::Session> {
    let mut last_error = None;
    for _ in 0..30 {
        match connector.connect(profile, None).await {
            Ok(session) => return session,
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    panic!(
        "connection never succeeded: {}",
        last_error.map_or_else(|| "unknown error".to_string(), |error| error.to_string())
    );
}

fn local_profile(engine: Engine, port: u16, database: &str) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("Test", engine, "127.0.0.1", database);
    profile.port = port;
    profile.ssl_mode = SslMode::Disable;
    profile
}

#[tokio::test]
async fn redis_runs_commands_and_maps_replies() {
    let _guard = container_test_lock().lock().await;
    let container: ContainerAsync<GenericImage> = GenericImage::new("redis", "7-alpine")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await
        .expect("start redis");
    let port = container
        .get_host_port_ipv4(6379.tcp())
        .await
        .expect("redis port");

    let connector = RedisConnector::new();
    let mut session =
        connect_with_retry(&connector, &local_profile(Engine::Redis, port, "0")).await;

    session.execute("SET greeting hello").await.expect("set");
    let result = session.execute("GET greeting").await.expect("get");
    assert_eq!(result.rows, vec![vec![Value::Text("hello".to_string())]]);

    session.execute("RPUSH numbers 1 2 3").await.expect("push");
    let list = session.execute("LRANGE numbers 0 -1").await.expect("range");
    assert_eq!(
        list.rows,
        vec![
            vec![Value::Text("1".to_string())],
            vec![Value::Text("2".to_string())],
            vec![Value::Text("3".to_string())],
        ]
    );
}

#[tokio::test]
async fn redis_read_only_sessions_block_writes() {
    let _guard = container_test_lock().lock().await;
    let container: ContainerAsync<GenericImage> = GenericImage::new("redis", "7-alpine")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await
        .expect("start redis");
    let port = container
        .get_host_port_ipv4(6379.tcp())
        .await
        .expect("redis port");

    let connector = RedisConnector::new();
    let mut profile = local_profile(Engine::Redis, port, "0");
    profile.read_only = true;
    let mut session = connect_with_retry(&connector, &profile).await;

    let error = session
        .execute("SET blocked 1")
        .await
        .expect_err("writes are blocked");
    assert_eq!(error.code(), "read_only_blocked");
    assert!(session.execute("PING").await.is_ok());
}

#[tokio::test]
async fn mongodb_runs_json_commands() {
    let _guard = container_test_lock().lock().await;
    let container: ContainerAsync<GenericImage> = GenericImage::new("mongo", "7")
        .with_exposed_port(27017.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Waiting for connections"))
        .start()
        .await
        .expect("start mongo");
    let port = container
        .get_host_port_ipv4(27017.tcp())
        .await
        .expect("mongo port");

    let connector = MongoConnector::new();
    let mut session =
        connect_with_retry(&connector, &local_profile(Engine::MongoDB, port, "admin")).await;

    let ping = session.execute(r#"{"ping": 1}"#).await.expect("ping");
    let Value::Json(reply) = &ping.rows[0][0] else {
        panic!("expected a JSON reply");
    };
    assert_eq!(
        reply.get("ok").and_then(serde_json::Value::as_f64),
        Some(1.0)
    );

    session
        .execute(r#"{"insert": "unidb_test", "documents": [{"name": "Ada", "score": 9.5}]}"#)
        .await
        .expect("insert");
    let find = session
        .execute(r#"{"find": "unidb_test", "filter": {}}"#)
        .await
        .expect("find");
    let Value::Json(reply) = &find.rows[0][0] else {
        panic!("expected a JSON reply");
    };
    let batch = reply
        .pointer("/cursor/firstBatch")
        .and_then(serde_json::Value::as_array)
        .expect("cursor batch");
    assert_eq!(batch.len(), 1);
    assert_eq!(
        batch[0].get("name").and_then(serde_json::Value::as_str),
        Some("Ada")
    );
}

#[tokio::test]
async fn mongodb_read_only_sessions_block_writes() {
    let _guard = container_test_lock().lock().await;
    let container: ContainerAsync<GenericImage> = GenericImage::new("mongo", "7")
        .with_exposed_port(27017.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Waiting for connections"))
        .start()
        .await
        .expect("start mongo");
    let port = container
        .get_host_port_ipv4(27017.tcp())
        .await
        .expect("mongo port");

    let connector = MongoConnector::new();
    let mut profile = local_profile(Engine::MongoDB, port, "admin");
    profile.read_only = true;
    let mut session = connect_with_retry(&connector, &profile).await;

    let error = session
        .execute(r#"{"insert": "blocked", "documents": [{"a": 1}]}"#)
        .await
        .expect_err("writes are blocked");
    assert_eq!(error.code(), "read_only_blocked");
    assert!(session.execute(r#"{"ping": 1}"#).await.is_ok());
}
