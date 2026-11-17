//! Live container tests for the remaining engines. These images are large, so
//! the tests are ignored by default; run them with `-- --ignored` before a
//! release or when the engine code changes.

use std::sync::OnceLock;
use std::time::Duration;

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use unidb_core::{ConnectionProfile, Engine, SslMode, Value};
use unidb_engine::{CassandraConnector, Connector, MssqlConnector};

fn container_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn local_profile(engine: Engine, port: u16, database: &str) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("Test", engine, "127.0.0.1", database);
    profile.port = port;
    profile.ssl_mode = SslMode::Disable;
    profile
}

#[tokio::test]
#[ignore = "large Cassandra image and slow start; run with --ignored"]
async fn cassandra_runs_cql_and_returns_rows() {
    let _guard = container_test_lock().lock().await;
    // The driver reconnects to the address the node advertises. Tell Cassandra
    // to advertise 127.0.0.1 and publish the fixed native port so the follow up
    // connections reach the container through the host mapping.
    let container: ContainerAsync<GenericImage> = GenericImage::new("cassandra", "4.1")
        .with_wait_for(WaitFor::message_on_stdout(
            "Starting listening for CQL clients",
        ))
        .with_startup_timeout(Duration::from_secs(300))
        .with_mapped_port(9042, 9042.tcp())
        .with_env_var("CASSANDRA_BROADCAST_ADDRESS", "127.0.0.1")
        .start()
        .await
        .expect("start cassandra");
    let port = container
        .get_host_port_ipv4(9042.tcp())
        .await
        .expect("cassandra port");

    let connector = CassandraConnector::new();
    let mut session = connector
        .connect(&local_profile(Engine::Cassandra, port, ""), None)
        .await
        .expect("connect cassandra");

    session
        .execute(
            "CREATE KEYSPACE IF NOT EXISTS unidb WITH replication = {'class':'SimpleStrategy','replication_factor':1}",
        )
        .await
        .expect("create keyspace");
    session
        .execute("CREATE TABLE IF NOT EXISTS unidb.users (id int PRIMARY KEY, name text)")
        .await
        .expect("create table");
    session
        .execute("INSERT INTO unidb.users (id, name) VALUES (1, 'Ada')")
        .await
        .expect("insert");

    let result = session
        .execute("SELECT id, name FROM unidb.users")
        .await
        .expect("select");
    assert_eq!(result.rows.len(), 1);
    assert!(result.rows[0].contains(&Value::Int(1)));
    assert!(result.rows[0].contains(&Value::Text("Ada".to_string())));
}

#[tokio::test]
#[ignore = "large SQL Server image; run with --ignored"]
async fn mssql_runs_statements_and_returns_rows() {
    let _guard = container_test_lock().lock().await;
    let container: ContainerAsync<GenericImage> =
        GenericImage::new("mcr.microsoft.com/mssql/server", "2022-latest")
            .with_exposed_port(1433.tcp())
            .with_wait_for(WaitFor::message_on_stdout(
                "SQL Server is now ready for client connections",
            ))
            .with_startup_timeout(Duration::from_secs(300))
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_SA_PASSWORD", "Unidb!Passw0rd")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .expect("start sql server");
    let port = container
        .get_host_port_ipv4(1433.tcp())
        .await
        .expect("sql server port");

    let connector = MssqlConnector::new();
    let mut profile = local_profile(Engine::MSSQL, port, "master");
    profile.username = "sa".to_string();
    let mut last_error = None;
    let mut session = None;
    for _ in 0..10 {
        match connector.connect(&profile, Some("Unidb!Passw0rd")).await {
            Ok(connected) => {
                session = Some(connected);
                break;
            }
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
    let mut session = session.unwrap_or_else(|| {
        panic!(
            "connect sql server: {}",
            last_error.map_or_else(|| "unknown error".to_string(), |error| error.to_string())
        )
    });

    session
        .execute("CREATE TABLE #unidb_users (id INT, name NVARCHAR(64))")
        .await
        .expect("create table");
    session
        .execute("INSERT INTO #unidb_users VALUES (1, 'Ada')")
        .await
        .expect("insert");
    let result = session
        .execute("SELECT id, name FROM #unidb_users")
        .await
        .expect("select");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::Text("Ada".to_string()));
}
