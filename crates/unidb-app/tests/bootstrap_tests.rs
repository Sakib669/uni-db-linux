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
