use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use unidb_app::bridge::{Bridge, EventSink};
use unidb_core::{ConnectionProfile, Engine, Value};
use unidb_engine::{ConnectorRegistry, SqliteConnector};
use unidb_store::Store;
use unidb_store::secrets::MemoryKeyProvider;

#[derive(Clone)]
struct CountingSink {
    received: Arc<AtomicUsize>,
}

impl EventSink<u32> for CountingSink {
    fn send(&self, event: u32) {
        self.received.fetch_add(event as usize, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn bridge_runs_work_off_the_calling_thread_and_delivers_events() {
    let bridge = Bridge::new(tokio::runtime::Handle::current());
    let received = Arc::new(AtomicUsize::new(0));
    let sink = CountingSink {
        received: Arc::clone(&received),
    };

    bridge.spawn(async move {
        sink.send(21);
        sink.send(21);
    });

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while received.load(Ordering::SeqCst) < 42 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("events arrive");
    assert_eq!(received.load(Ordering::SeqCst), 42);
}

async fn test_service() -> (AppServiceFixture, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let store = Store::open(dir.path().join("unidb.db"), &provider)
        .await
        .expect("open store");
    let mut connectors = ConnectorRegistry::new();
    connectors.register(Arc::new(SqliteConnector::new()));
    let bridge = Bridge::new(tokio::runtime::Handle::current());
    let service = unidb_app::AppService::new(bridge, Arc::new(store), Arc::new(connectors));
    (AppServiceFixture { service }, dir)
}

struct AppServiceFixture {
    service: unidb_app::AppService,
}

async fn save_sqlite_profile(
    service: &unidb_app::AppService,
    dir: &std::path::Path,
) -> ConnectionProfile {
    let database = dir.join("target.db");
    let profile =
        ConnectionProfile::new("Local file", Engine::SQLite, "", database.to_string_lossy());
    service
        .store()
        .expect("store")
        .save_connection(&profile, None)
        .await
        .expect("save profile");
    profile
}

#[tokio::test]
async fn opens_a_connection_runs_statements_and_records_history() {
    let (fixture, dir) = test_service().await;
    let profile = save_sqlite_profile(&fixture.service, dir.path()).await;

    fixture
        .service
        .open_connection(&profile)
        .await
        .expect("open connection");

    fixture
        .service
        .execute(profile.id, "CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .expect("create");
    fixture
        .service
        .execute(profile.id, "INSERT INTO users VALUES (1, 'Ada')")
        .await
        .expect("insert");
    let result = fixture
        .service
        .execute(profile.id, "SELECT id, name FROM users")
        .await
        .expect("select");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::Text("Ada".to_string()));

    let history = fixture
        .service
        .store()
        .expect("store")
        .search_history("users", 10)
        .await
        .expect("history");
    assert!(
        history
            .iter()
            .any(|entry| entry.sql.contains("SELECT id, name"))
    );

    fixture
        .service
        .close_connection(profile.id)
        .await
        .expect("close");
}

#[tokio::test]
async fn executing_on_a_closed_connection_is_a_safe_error() {
    let (fixture, dir) = test_service().await;
    let profile = save_sqlite_profile(&fixture.service, dir.path()).await;

    let error = fixture
        .service
        .execute(profile.id, "SELECT 1")
        .await
        .expect_err("no open connection");
    assert_eq!(error.code(), "connection_not_open");
}

#[tokio::test]
async fn close_is_idempotent() {
    let (fixture, dir) = test_service().await;
    let profile = save_sqlite_profile(&fixture.service, dir.path()).await;
    fixture
        .service
        .open_connection(&profile)
        .await
        .expect("open");

    fixture
        .service
        .close_connection(profile.id)
        .await
        .expect("first close");
    fixture
        .service
        .close_connection(profile.id)
        .await
        .expect("second close is a no op");
}

#[tokio::test]
async fn probe_runs_a_real_query_through_the_service() {
    let (fixture, _dir) = test_service().await;
    let result = fixture
        .service
        .probe_sqlite()
        .await
        .expect("probe should run");
    assert_eq!(result.rows.len(), 1);
    assert!(matches!(result.rows[0][0], Value::Text(_)));
}
