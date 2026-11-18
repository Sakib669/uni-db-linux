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

#[tokio::test]
async fn query_runs_are_audited_and_cancel_reports_unsupported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");
    service
        .execute(profile.id, "SELECT 1 AS one")
        .await
        .expect("query");

    let audit = service.recent_audit(10).await.expect("audit");
    assert!(
        audit.iter().any(|entry| entry.action == "query_run"),
        "query runs are audited"
    );

    let error = service
        .cancel(profile.id)
        .await
        .expect_err("SQLite cannot cancel");
    assert_eq!(error.code(), "cancel_unsupported");

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn designer_creates_and_alters_a_table_transactionally() {
    use unidb_core::{ColumnDesign, SqlDialect, TableDesign, alter_table_sql, create_table_sql};

    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");

    let mut id = ColumnDesign::new("id", "INTEGER");
    id.primary_key = true;
    id.auto_increment = true;
    id.nullable = false;
    let mut title = ColumnDesign::new("title", "TEXT");
    title.nullable = false;
    let mut design = TableDesign::new("events");
    design.columns = vec![id, title];

    let create = create_table_sql(SqlDialect::Sqlite, &design).expect("ddl");
    service
        .apply_statements(profile.id, &create)
        .await
        .expect("create table");
    let tables = service
        .list_tables(profile.id, "main")
        .await
        .expect("tables");
    assert!(tables.contains(&"events".to_string()), "{tables:?}");

    let original = service
        .describe_table(profile.id, "main", "events")
        .await
        .expect("describe");
    assert_eq!(original.len(), 2);
    assert!(original[0].primary_key);

    let mut altered = TableDesign::new("events");
    altered.columns = original.iter().map(|column| column.to_design()).collect();
    altered.columns.push(ColumnDesign::new("location", "TEXT"));
    let alter = alter_table_sql(SqlDialect::Sqlite, "events", &original, &altered).expect("alter");
    service
        .apply_statements(profile.id, &alter)
        .await
        .expect("alter table");
    let described = service
        .describe_table(profile.id, "main", "events")
        .await
        .expect("describe again");
    assert_eq!(described.len(), 3);
    assert!(described.iter().any(|column| column.name == "location"));

    let missing = service
        .describe_table(profile.id, "main", "missing_table")
        .await
        .expect("empty describe");
    assert!(missing.is_empty());

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn import_and_export_round_trip_through_files() {
    use unidb_app::{ImportOptions, ImportReport};
    use unidb_core::{ColumnDesign, SqlDialect, TableDesign, TransferFormat, create_table_sql};

    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");

    let mut id = ColumnDesign::new("id", "INTEGER");
    id.primary_key = true;
    id.auto_increment = true;
    id.nullable = false;
    let mut name = ColumnDesign::new("name", "TEXT");
    name.nullable = false;
    let mut score = ColumnDesign::new("score", "REAL");
    score.nullable = true;
    let mut design = TableDesign::new("people");
    design.columns = vec![id, name, score];
    service
        .apply_statements(
            profile.id,
            &create_table_sql(SqlDialect::Sqlite, &design).expect("ddl"),
        )
        .await
        .expect("create");

    let csv_path = dir.path().join("people.csv");
    std::fs::write(
        &csv_path,
        "id,name,score\n1,Ada,9.5\n2,\"Grace \"\"Hopper\"\"\",8\n3,Bad,not-a-number\n",
    )
    .expect("csv");
    let report: ImportReport = service
        .import_file(
            profile.id,
            &ImportOptions {
                database: "main".to_string(),
                table: "people".to_string(),
                path: csv_path,
                format: TransferFormat::Csv,
                has_headers: true,
                delimiter: b',',
                null_marker: None,
            },
        )
        .await
        .expect("import");
    assert_eq!(report.rows_read, 3);
    assert_eq!(report.rows_inserted, 2);
    assert_eq!(report.rows_rejected, 1);

    let rows = service
        .execute(profile.id, "SELECT COUNT(*) FROM people")
        .await
        .expect("count");
    assert_eq!(rows.rows[0][0], unidb_core::Value::Int(2));

    let export_path = dir.path().join("people-out.csv");
    let exported = service
        .export_query(
            profile.id,
            "SELECT id, name FROM people ORDER BY id",
            TransferFormat::Csv,
            &export_path,
        )
        .await
        .expect("export");
    assert_eq!(exported.rows, 2);
    let contents = std::fs::read_to_string(&export_path).expect("read export");
    assert!(contents.starts_with("id,name"));
    assert!(contents.contains("1,Ada"));

    let json_path = dir.path().join("people-out.json");
    service
        .export_query(
            profile.id,
            "SELECT id, name FROM people ORDER BY id",
            TransferFormat::Json,
            &json_path,
        )
        .await
        .expect("json export");
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json_path).expect("read json"))
            .expect("parse json");
    assert_eq!(parsed[1]["name"], "Grace \"Hopper\"");

    let audit = service.recent_audit(20).await.expect("audit");
    assert!(audit.iter().any(|entry| entry.action == "import_finished"));
    assert!(audit.iter().any(|entry| entry.action == "export_finished"));

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn history_saved_queries_and_settings_round_trip() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    service.record_history_for_test("SELECT 1", "success").await;
    let history = service.search_history("", 20).await.expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].sql, "SELECT 1");
    assert_eq!(service.clear_history().await.expect("clear"), 1);
    assert!(
        service
            .search_history("", 20)
            .await
            .expect("history")
            .is_empty()
    );

    let saved = service
        .save_saved_query("One", "SELECT 1", None)
        .await
        .expect("save");
    let listed = service.list_saved_queries().await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        service.delete_saved_query(saved.id).await.expect("delete"),
        1
    );
    assert!(service.list_saved_queries().await.expect("list").is_empty());

    assert!(
        service
            .get_setting("color_scheme")
            .await
            .expect("get")
            .is_none()
    );
    service
        .set_setting("color_scheme", serde_json::json!("dark"))
        .await
        .expect("set");
    assert_eq!(
        service.get_setting("color_scheme").await.expect("get"),
        Some(serde_json::json!("dark"))
    );

    service.shutdown().await;
}

#[tokio::test]
async fn foreign_keys_are_reported_for_a_sqlite_schema() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");
    service
        .apply_statements(
            profile.id,
            &[
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_string(),
                "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id), title TEXT)".to_string(),
            ],
        )
        .await
        .expect("schema");
    let keys = service
        .foreign_keys(profile.id, "main", "posts")
        .await
        .expect("foreign keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].from_table, "posts");
    assert_eq!(keys[0].from_column, "user_id");
    assert_eq!(keys[0].to_table, "users");

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn fake_data_is_deterministic_and_inserted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");
    service
        .apply_statements(
            profile.id,
            &[
                "CREATE TABLE people (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)"
                    .to_string(),
            ],
        )
        .await
        .expect("schema");
    let generated = service
        .generate_fake_rows(profile.id, "main", "people", 4, 7)
        .await
        .expect("generate");
    assert_eq!(generated, 4);
    let result = service
        .execute(profile.id, "SELECT COUNT(*) FROM people")
        .await
        .expect("count");
    assert_eq!(result.rows[0][0], unidb_core::Value::Int(4));
    let audit = service.recent_audit(20).await.expect("audit");
    assert!(
        audit
            .iter()
            .any(|entry| entry.action == "fake_data_generated")
    );

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn masked_exports_hide_text_values() {
    use unidb_core::TransferFormat;

    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");
    service
        .apply_statements(
            profile.id,
            &[
                "CREATE TABLE people (id INTEGER PRIMARY KEY, name TEXT, email TEXT)".to_string(),
                "INSERT INTO people (id, name, email) VALUES (1, 'Ada', 'ada@example.com')"
                    .to_string(),
            ],
        )
        .await
        .expect("schema");
    let path = dir.path().join("masked.csv");
    let report = service
        .export_query_masked(
            profile.id,
            "SELECT id, name, email FROM people",
            TransferFormat::Csv,
            &path,
        )
        .await
        .expect("masked export");
    assert_eq!(report.rows, 1);
    let contents = std::fs::read_to_string(&path).expect("read");
    assert!(contents.contains("[masked]"), "{contents}");
    assert!(contents.contains("masked@example.test"), "{contents}");
    assert!(!contents.contains("Ada"), "{contents}");

    let audit = service.recent_audit(20).await.expect("audit");
    assert!(audit.iter().any(|entry| entry.action == "export_finished"));

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn migrations_apply_once_and_track_the_ledger() {
    let dir = tempfile::tempdir().expect("temp dir");
    let migrations = dir.path().join("migrations");
    std::fs::create_dir_all(&migrations).expect("migrations dir");
    std::fs::write(
        migrations.join("0001_create.sql"),
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL);",
    )
    .expect("write 1");
    std::fs::write(
        migrations.join("0002_seed.sql"),
        "INSERT INTO notes (id, body) VALUES (1, 'hello; world');",
    )
    .expect("write 2");

    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;
    let profile = sqlite_profile(dir.path());
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");

    let (plan_files, plan_applied, warnings) = service
        .plan_migrations(profile.id, &migrations)
        .await
        .expect("plan before apply");
    assert_eq!(plan_files.len(), 2);
    assert!(plan_applied.is_empty());
    assert!(warnings.is_empty());

    let first = service
        .apply_migrations(profile.id, &migrations)
        .await
        .expect("first run");
    assert_eq!(first.applied.len(), 2);
    assert_eq!(first.skipped, 0);

    let (plan_files2, plan_applied2, warnings2) = service
        .plan_migrations(profile.id, &migrations)
        .await
        .expect("plan after apply");
    assert_eq!(plan_files2.len(), 2);
    assert_eq!(plan_applied2.len(), 2);
    assert!(warnings2.is_empty());
    let rows = service
        .execute(profile.id, "SELECT body FROM notes WHERE id = 1")
        .await
        .expect("query");
    assert_eq!(
        rows.rows[0][0],
        unidb_core::Value::Text("hello; world".to_string())
    );

    let second = service
        .apply_migrations(profile.id, &migrations)
        .await
        .expect("second run");
    assert!(second.applied.is_empty());
    assert_eq!(second.skipped, 2);

    let audit = service.recent_audit(20).await.expect("audit");
    assert!(
        audit
            .iter()
            .any(|entry| entry.action == "migration_applied")
    );

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn table_snapshots_capture_diff_and_restore() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let db_path = dir.path().join("target.db");
    let profile = ConnectionProfile::new(
        "Target",
        Engine::SQLite,
        "",
        db_path.to_str().expect("path"),
    );
    service
        .save_connection(&profile, None)
        .await
        .expect("save connection");
    service
        .open_connection(&profile)
        .await
        .expect("open connection");

    service
        .execute(
            profile.id,
            "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, qty INTEGER);",
        )
        .await
        .expect("create table");
    service
        .execute(
            profile.id,
            "INSERT INTO items (id, name, qty) VALUES (1, 'hammer', 5), (2, 'wrench', 10);",
        )
        .await
        .expect("insert rows");

    let snapshot = service
        .create_snapshot(
            profile.id,
            "main",
            "items",
            Some("pre-update baseline".to_string()),
        )
        .await
        .expect("create snapshot");
    assert_eq!(snapshot.table_name, "items");
    assert_eq!(snapshot.row_count, 2);
    assert_eq!(snapshot.note.as_deref(), Some("pre-update baseline"));

    let list = service
        .list_snapshots(profile.id)
        .await
        .expect("list snapshots");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, snapshot.id);

    let fetched = service
        .get_snapshot(snapshot.id)
        .await
        .expect("get snapshot")
        .expect("found");
    assert_eq!(fetched.id, snapshot.id);

    let diff_baseline = service
        .diff_snapshot(profile.id, snapshot.id)
        .await
        .expect("diff against unchanged");
    assert!(!diff_baseline.has_changes());
    assert_eq!(diff_baseline.unchanged, 2);

    service
        .execute(profile.id, "UPDATE items SET qty = 99 WHERE id = 1;")
        .await
        .expect("update row");
    service
        .execute(
            profile.id,
            "INSERT INTO items (id, name, qty) VALUES (3, 'saw', 2);",
        )
        .await
        .expect("insert new row");

    let diff_modified = service
        .diff_snapshot(profile.id, snapshot.id)
        .await
        .expect("diff against modified");
    assert!(diff_modified.has_changes());
    assert_eq!(diff_modified.changed, 1);
    assert_eq!(diff_modified.added, 1);

    let restored_count = service
        .restore_snapshot(profile.id, snapshot.id)
        .await
        .expect("restore snapshot");
    assert_eq!(restored_count, 2);

    let check = service
        .execute(profile.id, "SELECT id, name, qty FROM items ORDER BY id;")
        .await
        .expect("query restored rows");
    assert_eq!(check.rows.len(), 2);
    assert_eq!(check.rows[0][0], unidb_core::Value::Int(1));
    assert_eq!(
        check.rows[0][1],
        unidb_core::Value::Text("hammer".to_string())
    );
    assert_eq!(check.rows[0][2], unidb_core::Value::Int(5));
    assert_eq!(check.rows[1][0], unidb_core::Value::Int(2));
    assert_eq!(
        check.rows[1][1],
        unidb_core::Value::Text("wrench".to_string())
    );
    assert_eq!(check.rows[1][2], unidb_core::Value::Int(10));

    let audit = service.recent_audit(20).await.expect("audit");
    assert!(audit.iter().any(|entry| entry.action == "snapshot_created"));
    assert!(
        audit
            .iter()
            .any(|entry| entry.action == "snapshot_restored")
    );

    assert!(
        service
            .delete_snapshot(snapshot.id)
            .await
            .expect("delete snapshot")
    );
    assert!(
        service
            .list_snapshots(profile.id)
            .await
            .expect("list after delete")
            .is_empty()
    );

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn multi_database_query_runs_across_open_connections_and_merges() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let db1 = dir.path().join("shard_east.db");
    let db2 = dir.path().join("shard_west.db");
    let p1 = ConnectionProfile::new("East", Engine::SQLite, "", db1.to_str().expect("p1"));
    let p2 = ConnectionProfile::new("West", Engine::SQLite, "", db2.to_str().expect("p2"));

    service.save_connection(&p1, None).await.expect("save p1");
    service.save_connection(&p2, None).await.expect("save p2");
    service.open_connection(&p1).await.expect("open p1");
    service.open_connection(&p2).await.expect("open p2");

    service
        .execute(
            p1.id,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users VALUES (1, 'Ada');",
        )
        .await
        .expect("seed p1");
    service
        .execute(
            p2.id,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users VALUES (2, 'Grace');",
        )
        .await
        .expect("seed p2");

    let (summaries, merged) = service
        .execute_multi(&[p1.id, p2.id], "SELECT id, name FROM users ORDER BY id;")
        .await
        .expect("execute_multi");
    assert_eq!(summaries.len(), 2);
    assert!(
        summaries
            .iter()
            .all(|s| s.error.is_none() && s.row_count == 1)
    );
    assert_eq!(merged.columns.len(), 3);
    assert_eq!(merged.columns[0].name, "_connection");
    assert_eq!(merged.rows.len(), 2);
    assert_eq!(
        merged.rows[0][0],
        unidb_core::Value::Text("East".to_string())
    );
    assert_eq!(
        merged.rows[1][0],
        unidb_core::Value::Text("West".to_string())
    );

    let audit = service.recent_audit(20).await.expect("audit");
    assert!(audit.iter().any(|entry| entry.action == "multi_query_run"));

    service.close_connection(p1.id).await.expect("close p1");
    service.close_connection(p2.id).await.expect("close p2");
    service.shutdown().await;
}

#[tokio::test]
async fn team_bundles_export_verify_and_import_without_secrets() {
    let dir_a = tempfile::tempdir().expect("dir a");
    let provider_a = MemoryKeyProvider::empty();
    let service_a = bootstrap(dir_a.path(), &provider_a).await;

    let mut conn = ConnectionProfile::new("Staging PG", Engine::PostgreSQL, "127.0.0.1", "app");
    conn.username = "reader".to_string();
    service_a
        .save_connection(&conn, Some("super-secret-password"))
        .await
        .expect("save conn");
    service_a
        .save_saved_query("Health check", "SELECT 1;", None)
        .await
        .expect("save query");

    let bundle_path = dir_a.path().join("team.json");
    let (bundle, sig_path) = service_a
        .export_team_bundle("Platform", "Ada", &bundle_path)
        .await
        .expect("export bundle");
    assert_eq!(bundle.connections.len(), 1);
    assert_eq!(bundle.saved_queries.len(), 1);
    assert!(sig_path.exists());
    let exported_text = std::fs::read_to_string(&bundle_path).expect("read bundle");
    assert!(!exported_text.contains("super-secret-password"));
    service_a.shutdown().await;

    let dir_b = tempfile::tempdir().expect("dir b");
    let provider_b = MemoryKeyProvider::empty();
    let service_b = bootstrap(dir_b.path(), &provider_b).await;

    let preview = service_b
        .inspect_team_bundle(&bundle_path)
        .await
        .expect("inspect bundle");
    assert!(preview.verification.signature_valid);
    assert!(preview.verification.detached_sig_checked);

    let report = service_b
        .import_team_bundle(&bundle_path, true)
        .await
        .expect("import bundle");
    assert_eq!(report.connections_imported, 1);
    assert_eq!(report.queries_imported, 1);
    assert!(report.signature_valid);

    let imported_conns = service_b.list_connections().await.expect("list conns");
    assert_eq!(imported_conns.len(), 1);
    assert_eq!(imported_conns[0].name, "Staging PG");

    // Re-importing skips duplicates.
    let second = service_b
        .import_team_bundle(&bundle_path, true)
        .await
        .expect("re-import");
    assert_eq!(second.connections_imported, 0);
    assert_eq!(second.connections_skipped, 1);

    // Tampered bundle is rejected when signature verification is required.
    std::fs::write(&bundle_path, exported_text.replace("Staging PG", "Evil PG")).expect("tamper");
    let err = service_b
        .import_team_bundle(&bundle_path, true)
        .await
        .expect_err("reject tampered");
    assert_eq!(err.code(), "bundle_signature_invalid");

    let audit = service_b.recent_audit(20).await.expect("audit");
    assert!(
        audit
            .iter()
            .any(|entry| entry.action == "team_bundle_imported")
    );
    service_b.shutdown().await;
}

#[tokio::test]
async fn schema_doctor_diagnoses_live_sqlite_tables() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let db_path = dir.path().join("doctor.db");
    let profile =
        ConnectionProfile::new("Doctor", Engine::SQLite, "", db_path.to_str().expect("p"));
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");

    service
        .execute(
            profile.id,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE events (user_id INTEGER REFERENCES users(id), kind TEXT);
             CREATE TABLE logs (msg TEXT);",
        )
        .await
        .expect("create tables");

    let report = service
        .diagnose_connection_schema(profile.id, "main")
        .await
        .expect("diagnose");
    assert_eq!(report.tables_scanned, 3);
    assert!(report.score < 100);
    assert!(report.warnings >= 2);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.table == "events" && f.column.as_deref() == Some("user_id"))
    );

    let audit = service.recent_audit(10).await.expect("audit");
    assert!(audit.iter().any(|e| e.action == "schema_doctor_run"));

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn scheduled_jobs_create_run_and_sweep() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let db_path = dir.path().join("jobs.db");
    let profile = ConnectionProfile::new("Jobs", Engine::SQLite, "", db_path.to_str().expect("p"));
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");

    service
        .execute(profile.id, "CREATE TABLE ticks (n INTEGER);")
        .await
        .expect("create table");

    let job = service
        .create_job("Heartbeat", profile.id, "INSERT INTO ticks VALUES (1);", 60)
        .await
        .expect("create job");
    assert!(job.enabled);

    let updated = service.run_job_now(job.id).await.expect("run_job_now");
    assert_eq!(updated.last_status.as_deref(), Some("success"));
    assert_eq!(updated.last_rows, Some(1));

    let count = service
        .execute(profile.id, "SELECT count(*) FROM ticks;")
        .await
        .expect("count");
    assert_eq!(count.rows[0][0], unidb_core::Value::Int(1));

    service
        .set_job_enabled(job.id, false)
        .await
        .expect("disable");
    assert!(service.run_due_jobs().await.expect("sweep").is_empty());

    assert!(service.delete_job(job.id).await.expect("delete"));
    assert!(service.list_jobs().await.expect("list").is_empty());

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}

#[tokio::test]
async fn wasm_plugins_install_execute_and_audit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let record = service
        .install_sample_wasm_plugin()
        .await
        .expect("install sample");
    assert_eq!(record.name, "sample-metrics");
    assert!(record.exports.contains(&"add".to_string()));

    let sum = service
        .run_wasm_plugin(record.id, "add", &[12, 30])
        .await
        .expect("run add");
    assert_eq!(sum, 42);

    let bonus = service
        .run_wasm_plugin(record.id, "score_bonus", &[5])
        .await
        .expect("run score_bonus");
    assert_eq!(bonus, 20);

    let audit = service.recent_audit(10).await.expect("audit");
    assert!(audit.iter().any(|e| e.action == "plugin_installed"));
    assert!(audit.iter().any(|e| e.action == "plugin_executed"));

    assert!(service.delete_wasm_plugin(record.id).await.expect("delete"));
    service.shutdown().await;
}

#[tokio::test]
async fn offline_sync_queues_detects_conflicts_and_flushes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = MemoryKeyProvider::empty();
    let service = bootstrap(dir.path(), &provider).await;

    let db_path = dir.path().join("sync.db");
    let profile = ConnectionProfile::new("Sync", Engine::SQLite, "", db_path.to_str().expect("p"));
    service.save_connection(&profile, None).await.expect("save");
    service.open_connection(&profile).await.expect("open");
    service
        .execute(
            profile.id,
            "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT); INSERT INTO items VALUES (1, 'one');",
        )
        .await
        .expect("seed");
    service
        .close_connection(profile.id)
        .await
        .expect("disconnect");

    // Draft offline operations while disconnected.
    let item_ok = service
        .queue_offline_operation(
            profile.id,
            "Add item 2",
            "INSERT INTO items VALUES (2, 'two');",
            Some("SELECT count(*) FROM items;".to_string()),
            Some("1".to_string()),
        )
        .await
        .expect("queue ok");

    let item_conflict = service
        .queue_offline_operation(
            profile.id,
            "Stale count write",
            "INSERT INTO items VALUES (99, 'conflict');",
            Some("SELECT count(*) FROM items;".to_string()),
            Some("999".to_string()),
        )
        .await
        .expect("queue conflict");

    assert_eq!(
        service
            .list_sync_queue(Some(profile.id))
            .await
            .expect("list")
            .len(),
        2
    );

    // Reconnect and flush.
    service.open_connection(&profile).await.expect("reconnect");
    let report = service.flush_sync_queue(profile.id).await.expect("flush");
    assert_eq!(report.synced, 1);
    assert_eq!(report.conflicts, 1);
    assert_eq!(report.failed, 0);

    let rows = service
        .execute(profile.id, "SELECT count(*) FROM items;")
        .await
        .expect("verify");
    assert_eq!(rows.rows[0][0], unidb_core::Value::Int(2));

    let items_after = service
        .list_sync_queue(Some(profile.id))
        .await
        .expect("list");
    assert_eq!(
        items_after
            .iter()
            .find(|i| i.id == item_ok.id)
            .expect("ok")
            .status,
        "synced"
    );
    assert_eq!(
        items_after
            .iter()
            .find(|i| i.id == item_conflict.id)
            .expect("cf")
            .status,
        "conflict"
    );

    let audit = service.recent_audit(10).await.expect("audit");
    assert!(audit.iter().any(|e| e.action == "offline_sync_flushed"));

    service.close_connection(profile.id).await.expect("close");
    service.shutdown().await;
}
