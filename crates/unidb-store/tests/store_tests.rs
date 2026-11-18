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

#[tokio::test]
async fn backups_rotate_and_counts_are_live() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();

    let store = Store::open(&path, &provider).await.expect("open store");
    let profile = ConnectionProfile::new("Local", Engine::SQLite, "", "app.db");
    store
        .save_connection(&profile, Some("secret"))
        .await
        .expect("save connection");
    store
        .add_audit(
            "connection_saved",
            Some(profile.id),
            "success",
            serde_json::json!({}),
        )
        .await
        .expect("audit");

    assert_eq!(store.connection_count().await.expect("count"), 1);
    assert_eq!(store.encrypted_secret_count().await.expect("secrets"), 1);
    assert_eq!(store.audit_count().await.expect("audit"), 1);

    for _ in 0..7 {
        store.backup_now().await.expect("backup");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let backups = store.list_backups().expect("list backups");
    assert_eq!(backups.len(), 5, "rotation keeps five backups");
    assert!(backups.iter().all(|path| path.exists()));
}

#[tokio::test]
async fn unclean_shutdown_marker_survives_and_clears() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    assert!(store.begin_run().await.expect("begin run").is_none());
    assert!(
        store
            .previous_unclean_shutdown()
            .await
            .expect("read marker")
            .is_some(),
        "the running marker is stored"
    );
    store.mark_clean_shutdown().await.expect("clean shutdown");
    assert!(
        store
            .previous_unclean_shutdown()
            .await
            .expect("read")
            .is_none()
    );
}

#[tokio::test]
async fn a_corrupt_database_reports_a_safe_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    std::fs::write(&path, b"this is not a sqlite database").expect("write garbage");
    let provider = test_provider();

    let error = match Store::open(&path, &provider).await {
        Ok(_) => panic!("a corrupt database must not open"),
        Err(error) => error,
    };
    assert!(
        matches!(error.code(), "store_corrupt" | "store_open_failed"),
        "unexpected error code {}",
        error.code()
    );
    assert!(error.user_message().to_lowercase().contains("database"));
}

#[tokio::test]
async fn saved_queries_round_trip_and_history_can_be_cleared() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    let saved = store
        .save_saved_query("Recent users", "SELECT * FROM users", None)
        .await
        .expect("save");
    assert_eq!(saved.name, "Recent users");
    let listed = store.list_saved_queries().await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].sql, "SELECT * FROM users");

    assert_eq!(store.delete_saved_query(saved.id).await.expect("delete"), 1);
    assert!(store.list_saved_queries().await.expect("list").is_empty());

    store
        .record_history(None, "SELECT 1", 3, Some(1), "success", None, None)
        .await
        .expect("history");
    assert_eq!(store.search_history("", 10).await.expect("search").len(), 1);
    assert_eq!(store.clear_history().await.expect("clear"), 1);
    assert!(
        store
            .search_history("", 10)
            .await
            .expect("search")
            .is_empty()
    );
}

#[tokio::test]
async fn table_snapshots_round_trip_and_can_be_deleted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    let conn_id = uuid::Uuid::new_v4();
    let query_result = unidb_core::QueryResult {
        columns: vec![
            unidb_core::Column {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                nullable: false,
            },
            unidb_core::Column {
                name: "name".to_string(),
                type_name: "TEXT".to_string(),
                nullable: true,
            },
        ],
        rows: vec![
            vec![
                unidb_core::Value::Int(1),
                unidb_core::Value::Text("Ada".to_string()),
            ],
            vec![
                unidb_core::Value::Int(2),
                unidb_core::Value::Text("Alan Turing".to_string()),
            ],
        ],
        affected_rows: None,
        elapsed_ms: 5,
    };

    let snapshot = unidb_store::TableSnapshot::from_query_result(
        conn_id,
        "main",
        "users",
        Some("pre-migration snapshot".to_string()),
        &query_result,
    )
    .expect("build snapshot");

    store.save_snapshot(&snapshot).await.expect("save snapshot");

    let listed = store.list_snapshots(conn_id).await.expect("list snapshots");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].table_name, "users");
    assert_eq!(listed[0].row_count, 2);
    assert_eq!(listed[0].note.as_deref(), Some("pre-migration snapshot"));

    let fetched = store
        .get_snapshot(snapshot.id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.id, snapshot.id);
    let decoded = fetched.to_query_result().expect("decode query result");
    assert_eq!(decoded.rows.len(), 2);
    assert_eq!(decoded.columns[0].name, "id");
    assert_eq!(decoded.columns[1].name, "name");

    assert!(store.delete_snapshot(snapshot.id).await.expect("delete"));
    assert!(
        store
            .list_snapshots(conn_id)
            .await
            .expect("list")
            .is_empty()
    );
}

#[tokio::test]
async fn platform_tables_round_trip_jobs_sync_and_plugins() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unidb.db");
    let provider = test_provider();
    let store = Store::open(&path, &provider).await.expect("open store");

    let conn_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();

    let job = unidb_store::ScheduledJob {
        id: uuid::Uuid::new_v4(),
        name: "Daily cleanup".to_string(),
        connection_id: conn_id,
        sql: "DELETE FROM temp_rows;".to_string(),
        interval_seconds: 60,
        enabled: true,
        last_run_at: None,
        next_run_at: now.clone(),
        last_status: None,
        last_rows: None,
        created_at: now.clone(),
    };
    store.save_job(&job).await.expect("save job");
    assert_eq!(store.list_jobs().await.expect("list").len(), 1);
    store.set_job_enabled(job.id, false).await.expect("disable");
    assert!(
        !store
            .get_job(job.id)
            .await
            .expect("get")
            .expect("some")
            .enabled
    );
    store
        .update_job_run(job.id, &now, &now, "success", Some(3))
        .await
        .expect("update run");
    assert!(store.delete_job(job.id).await.expect("delete"));

    let sync_item = unidb_store::SyncQueueItem {
        id: uuid::Uuid::new_v4(),
        connection_id: conn_id,
        label: "Seed row".to_string(),
        sql: "INSERT INTO users (name) VALUES ('Offline');".to_string(),
        guard_query: Some("SELECT count(*) FROM users;".to_string()),
        expected_guard_value: Some("1".to_string()),
        status: "pending".to_string(),
        error_message: None,
        created_at: now.clone(),
        synced_at: None,
    };
    store.enqueue_sync_item(&sync_item).await.expect("enqueue");
    assert_eq!(
        store
            .list_sync_items(Some(conn_id))
            .await
            .expect("list")
            .len(),
        1
    );
    store
        .update_sync_status(sync_item.id, "synced", None, Some(&now))
        .await
        .expect("update status");
    assert!(store.delete_sync_item(sync_item.id).await.expect("delete"));

    let plugin = unidb_store::WasmPluginRecord {
        id: uuid::Uuid::new_v4(),
        name: "metrics".to_string(),
        exports: vec!["add".to_string()],
        wasm_base64: "AGFzbQEAAAA=".to_string(),
        installed_at: now,
    };
    store.save_wasm_plugin(&plugin).await.expect("save plugin");
    assert_eq!(store.list_wasm_plugins().await.expect("list").len(), 1);
    assert!(store.delete_wasm_plugin(plugin.id).await.expect("delete"));
}
