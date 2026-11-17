use unidb_core::{ConnectionProfile, Engine, SslMode};
use unidb_store::Store;
use unidb_store::secrets::MemoryKeyProvider;

fn test_provider() -> MemoryKeyProvider {
    MemoryKeyProvider::empty()
}

#[tokio::test]
async fn opens_creates_schema_and_survives_a_reopen() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();

    {
        let store = Store::open(&path, &provider).await.expect("open store");
        let profile = ConnectionProfile::new("Local", Engine::SQLite, "", "app.db");
        store
            .save_connection(&profile, Some("secret"))
            .await
            .expect("save connection");
        assert_eq!(store.list_connections().await.expect("list").len(), 1);
    }

    let store = Store::open(&path, &provider).await.expect("reopen store");
    let connections = store.list_connections().await.expect("list");
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].name, "Local");

    let password = store
        .get_password(connections[0].id)
        .await
        .expect("read password")
        .expect("password exists");
    assert_eq!(password.as_str(), "secret");
}

#[tokio::test]
async fn a_different_keyring_key_is_rejected() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");

    {
        let provider = test_provider();
        let store = Store::open(&path, &provider).await.expect("open store");
        let profile = ConnectionProfile::new("Local", Engine::SQLite, "", "app.db");
        store
            .save_connection(&profile, Some("secret"))
            .await
            .expect("save connection");
    }

    let wrong_key = MemoryKeyProvider::with_bytes([7u8; 32]);
    let error = match Store::open(&path, &wrong_key).await {
        Ok(_) => panic!("mismatched key must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "master_key_mismatch");
}

#[tokio::test]
async fn a_missing_key_after_first_run_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");

    {
        let provider = test_provider();
        let store = Store::open(&path, &provider).await.expect("open store");
        drop(store);
    }

    let empty_keyring = test_provider();
    let error = match Store::open(&path, &empty_keyring).await {
        Ok(_) => panic!("missing key must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "master_key_missing");
}

#[tokio::test]
async fn settings_round_trip_as_json() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    store
        .set_setting("history_retention_days", serde_json::json!(90))
        .await
        .expect("set setting");
    let value = store
        .get_setting("history_retention_days")
        .await
        .expect("get setting")
        .expect("setting exists");
    assert_eq!(value, serde_json::json!(90));
    assert!(store.get_setting("missing").await.expect("get").is_none());
}

#[tokio::test]
async fn audit_log_keeps_the_newest_entries_first() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    store
        .add_audit("connection_opened", None, "success", serde_json::json!({}))
        .await
        .expect("audit");
    store
        .add_audit("query_run", None, "success", serde_json::json!({"rows": 1}))
        .await
        .expect("audit");

    let entries = store.list_audit(10).await.expect("list audit");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].action, "query_run");
}

#[tokio::test]
async fn history_is_searchable_with_full_text() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    store
        .record_history(
            None,
            "SELECT * FROM users",
            12,
            Some(3),
            "success",
            None,
            None,
        )
        .await
        .expect("record");
    store
        .record_history(
            None,
            "UPDATE orders SET paid = 1",
            8,
            None,
            "success",
            None,
            None,
        )
        .await
        .expect("record");

    let matches = store.search_history("users", 10).await.expect("search");
    assert_eq!(matches.len(), 1);
    assert!(matches[0].sql.contains("users"));

    let recent = store.search_history("", 10).await.expect("recent");
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].sql, "UPDATE orders SET paid = 1");
}

#[tokio::test]
async fn opening_fails_for_an_invalid_cloud_profile() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    let mut profile = ConnectionProfile::new("Cloud", Engine::PostgreSQL, "db.example.com", "app");
    profile.ssl_mode = SslMode::Disable;
    let error = store
        .save_connection(&profile, Some("secret"))
        .await
        .expect_err("cloud TLS is required");
    assert_eq!(error.code(), "cloud_requires_tls");
}
