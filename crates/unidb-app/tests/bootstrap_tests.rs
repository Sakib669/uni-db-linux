//! Headless tests for the real store bootstrap, the security report, and the
//! startup and shutdown markers. No desktop keyring is needed.

use tokio::runtime::Handle;
use unidb_app::AppService;
use unidb_core::{ConnectionProfile, Engine, Value};
use unidb_store::MemoryKeyProvider;

async fn bootstrap(dir: &std::path::Path, provider: &MemoryKeyProvider) -> AppService {
    AppService::bootstrap_in(Handle::current(), dir, provider)
        .await
        .expect("bootstrap app service")
}

fn sqlite_profile(dir: &std::path::Path) -> ConnectionProfile {
    let database = dir.join("target.db");
    ConnectionProfile::new("Local file", Engine::SQLite, "", database.to_string_lossy())
}

#[tokio::test]
async fn bootstrap_opens_the_store_and_saves_connections() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let report = service.security_report().await.expect("report");
    assert!(report.checks.len() >= 8);
    assert_eq!(report.connection_count, 0);

    let profile = sqlite_profile(dir.path());
    service
        .save_connection(&profile, Some("secret"))
        .await
        .expect("save connection");
    let connections = service.list_connections().await.expect("list");
    assert_eq!(connections.len(), 1);

    let stored_password = service
        .store()
        .expect("store")
        .get_password(profile.id)
        .await
        .expect("password")
        .expect("password exists");
    assert_eq!(stored_password.as_str(), "secret");

    let report = service.security_report().await.expect("report");
    assert_eq!(report.connection_count, 1);
    assert_eq!(report.secret_count, 1);
    assert!(report.audit_count >= 1);

    let audit = service.recent_audit(10).await.expect("audit");
    assert!(audit.iter().any(|entry| entry.action == "connection_saved"));

    service.shutdown().await;
}

#[tokio::test]
async fn deleting_a_connection_removes_it_and_audits_the_change() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service
        .save_connection(&profile, Some("secret"))
        .await
        .expect("save");

    let removed = service.delete_connection(profile.id).await.expect("delete");
    assert_eq!(removed, 1);
    assert!(service.list_connections().await.expect("list").is_empty());
    let audit = service.recent_audit(10).await.expect("audit");
    assert!(
        audit
            .iter()
            .any(|entry| entry.action == "connection_deleted")
    );

    service.shutdown().await;
}

#[tokio::test]
async fn clean_and_unclean_shutdowns_are_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();

    {
        let service = bootstrap(dir.path(), &provider).await;
        service.shutdown().await;
    }
    let service = bootstrap(dir.path(), &provider).await;
    let report = service.security_report().await.expect("report");
    assert!(
        report.previous_unclean_shutdown.is_none(),
        "a clean shutdown must not be reported as unclean"
    );
    // Dropping without shutdown simulates a crash.
    drop(service);

    let service = bootstrap(dir.path(), &provider).await;
    let report = service.security_report().await.expect("report");
    assert!(
        report.previous_unclean_shutdown.is_some(),
        "an unclean shutdown must be reported"
    );
    let shutdown_check = report
        .checks
        .iter()
        .find(|check| check.id == "shutdown")
        .expect("shutdown check");
    assert_eq!(shutdown_check.status, unidb_app::CheckStatus::Warn);
    service.shutdown().await;
}

#[tokio::test]
async fn bootstrap_opens_the_sqlite_engine_and_runs_a_query() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service
        .open_connection(&profile)
        .await
        .expect("open connection");
    let result = service
        .execute(profile.id, "SELECT 1 + 1 AS two")
        .await
        .expect("query");
    assert_eq!(result.rows, vec![vec![Value::Int(2)]]);
    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn backup_now_creates_a_copy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let backup = service.backup_now().await.expect("backup");
    assert!(backup.exists());
    assert_eq!(
        backup.extension().and_then(|value| value.to_str()),
        Some("db")
    );
    service.shutdown().await;
}

#[tokio::test]
async fn test_connection_reports_success_and_safe_failures() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let good = sqlite_profile(dir.path());
    service
        .test_connection(&good, None)
        .await
        .expect("sqlite file is reachable");

    let mut bad = ConnectionProfile::new("Bad", Engine::SQLite, "", "");
    bad.database = String::new();
    let error = service
        .test_connection(&bad, None)
        .await
        .expect_err("an empty SQLite path must fail");
    assert_eq!(error.code(), "sqlite_path_required");

    service.shutdown().await;
}

#[tokio::test]
async fn navigator_lists_databases_and_tables_for_an_open_connection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");
    service
        .execute(profile.id, "CREATE TABLE users (id INTEGER)")
        .await
        .expect("create");

    let databases = service.list_databases(profile.id).await.expect("databases");
    assert_eq!(databases, vec!["main".to_string()]);
    let tables = service
        .list_tables(profile.id, "main")
        .await
        .expect("tables");
    assert!(tables.contains(&"users".to_string()), "{tables:?}");

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}
