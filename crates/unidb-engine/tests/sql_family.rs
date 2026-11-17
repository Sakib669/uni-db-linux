//! Integration tests for the SQL family drivers. Each test starts a real
//! server with testcontainers, so the protocol, TLS mode mapping, read only
//! guard, and value conversion are exercised for real.
//!
//! These need Docker. The CockroachDB test is marked ignored because its image
//! is large; run it with `cargo test -- --ignored` when needed.

use std::sync::OnceLock;
use std::time::Duration;

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use unidb_core::{ConnectionProfile, Engine, SslMode, Value};
use unidb_engine::Connector;

/// MySQL and MariaDB containers restart once after initialising their data
/// directory, so the readiness log can appear before the final server is up.
/// Connection retries make the tests robust against that restart window.
async fn connect_with_retry(
    connector: &dyn Connector,
    profile: &ConnectionProfile,
    password: Option<&str>,
) -> Box<dyn unidb_engine::Session> {
    let mut last_error = None;
    for _ in 0..30 {
        match connector.connect(profile, password).await {
            Ok(session) => return session,
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!(
        "connection never succeeded: {}",
        last_error.map_or_else(|| "unknown error".to_string(), |error| error.to_string())
    );
}

fn local_profile(engine: Engine, port: u16, database: &str, username: &str) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("Test", engine, "127.0.0.1", database);
    profile.port = port;
    profile.username = username.to_string();
    profile.ssl_mode = SslMode::Disable;
    profile
}

/// Container tests each start their own server, so run them one at a time to
/// stay inside Docker resource limits on CI runners.
fn container_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn postgres_container() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new("postgres", "16-alpine")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "unidb")
        .with_env_var("POSTGRES_PASSWORD", "unidb")
        .with_env_var("POSTGRES_DB", "unidb")
        .start()
        .await
        .expect("start postgres");
    let port = container
        .get_host_port_ipv4(5432.tcp())
        .await
        .expect("postgres port");
    (container, port)
}

async fn mysql_container(name: &str, tag: &str) -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new(name, tag)
        .with_exposed_port(3306.tcp())
        .with_wait_for(WaitFor::message_on_stderr("ready for connections"))
        .with_env_var("MYSQL_ROOT_PASSWORD", "unidb")
        .with_env_var("MYSQL_DATABASE", "unidb")
        .with_env_var("MARIADB_ROOT_PASSWORD", "unidb")
        .with_env_var("MARIADB_DATABASE", "unidb")
        .start()
        .await
        .expect("start mysql family container");
    let port = container
        .get_host_port_ipv4(3306.tcp())
        .await
        .expect("mysql port");
    (container, port)
}

#[tokio::test]
async fn postgres_connects_and_returns_typed_rows() {
    let _guard = container_test_lock().lock().await;
    let (_container, port) = postgres_container().await;
    let connector = unidb_engine::PostgresConnector::new();
    let profile = local_profile(Engine::PostgreSQL, port, "unidb", "unidb");
    let mut session = connector
        .connect(&profile, Some("unidb"))
        .await
        .expect("connect postgres");

    session
        .execute("CREATE TABLE IF NOT EXISTS t (id INT, name TEXT, score DOUBLE PRECISION, active BOOLEAN)")
        .await
        .expect("create");
    session
        .execute("INSERT INTO t VALUES (1, 'Ada', 9.5, true)")
        .await
        .expect("insert");

    let result = session
        .execute("SELECT id, name, score, active FROM t")
        .await
        .expect("select");
    assert_eq!(result.columns.len(), 4);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::Text("Ada".to_string()));
    assert_eq!(result.rows[0][2], Value::Float(9.5));
    assert_eq!(result.rows[0][3], Value::Bool(true));
}

#[tokio::test]
async fn postgres_read_only_sessions_block_writes() {
    let _guard = container_test_lock().lock().await;
    let (_container, port) = postgres_container().await;
    let connector = unidb_engine::PostgresConnector::new();
    let mut profile = local_profile(Engine::PostgreSQL, port, "unidb", "unidb");
    profile.read_only = true;
    let mut session = connector
        .connect(&profile, Some("unidb"))
        .await
        .expect("connect postgres");

    let error = session
        .execute("CREATE TABLE blocked (id INT)")
        .await
        .expect_err("writes are blocked");
    assert_eq!(error.code(), "read_only_blocked");

    let result = session
        .execute("SELECT 1 AS one")
        .await
        .expect("select works");
    assert_eq!(result.rows[0][0], Value::Int(1));
}

#[tokio::test]
async fn mysql_connects_and_returns_typed_rows() {
    let _guard = container_test_lock().lock().await;
    let (_container, port) = mysql_container("mysql", "8.4").await;
    let connector = unidb_engine::MySqlConnector::new();
    let profile = local_profile(Engine::MySQL, port, "unidb", "root");
    let mut session = connect_with_retry(&connector, &profile, Some("unidb")).await;

    session
        .execute(
            "CREATE TABLE IF NOT EXISTS t (id INT, name VARCHAR(64), score DOUBLE, active BOOLEAN)",
        )
        .await
        .expect("create");
    session
        .execute("INSERT INTO t VALUES (1, 'Ada', 9.5, true)")
        .await
        .expect("insert");

    let result = session
        .execute("SELECT id, name, score, active FROM t")
        .await
        .expect("select");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::Text("Ada".to_string()));
    assert_eq!(result.rows[0][2], Value::Float(9.5));
}

#[tokio::test]
async fn mariadb_connects_through_the_mysql_connector() {
    let _guard = container_test_lock().lock().await;
    let (_container, port) = mysql_container("mariadb", "11").await;
    let connector = unidb_engine::MySqlConnector::new();
    let profile = local_profile(Engine::MariaDB, port, "unidb", "root");
    let mut session = connect_with_retry(&connector, &profile, Some("unidb")).await;

    let result = session
        .execute("SELECT 1 AS one, 'mariadb' AS name")
        .await
        .expect("select");
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::Text("mariadb".to_string()));
}

#[tokio::test]
async fn mysql_read_only_sessions_block_writes() {
    let _guard = container_test_lock().lock().await;
    let (_container, port) = mysql_container("mysql", "8.4").await;
    let connector = unidb_engine::MySqlConnector::new();
    let mut profile = local_profile(Engine::MySQL, port, "unidb", "root");
    profile.read_only = true;
    let mut session = connect_with_retry(&connector, &profile, Some("unidb")).await;

    let error = session
        .execute("CREATE TABLE blocked (id INT)")
        .await
        .expect_err("writes are blocked");
    assert_eq!(error.code(), "read_only_blocked");
}

#[tokio::test]
#[ignore = "large CockroachDB image; run with --ignored"]
async fn cockroachdb_connects_through_the_postgres_connector() {
    let _guard = container_test_lock().lock().await;
    let container = GenericImage::new("cockroachdb/cockroach", "v24.3.0")
        .with_exposed_port(26257.tcp())
        .with_wait_for(WaitFor::message_on_stdout("node starting"))
        .with_cmd(vec!["start-single-node", "--insecure"])
        .start()
        .await
        .expect("start cockroachdb");
    let port = container
        .get_host_port_ipv4(26257.tcp())
        .await
        .expect("cockroach port");

    let connector = unidb_engine::PostgresConnector::new();
    let profile = local_profile(Engine::CockroachDB, port, "defaultdb", "root");
    let mut session = connector
        .connect(&profile, None)
        .await
        .expect("connect cockroachdb");
    let result = session
        .execute("SELECT 1::int AS one")
        .await
        .expect("select");
    assert_eq!(result.rows[0][0], Value::Int(1));
}
