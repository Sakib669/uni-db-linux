use unidb_core::{ConnectionProfile, Engine, Value};
use unidb_engine::{Connector, SqliteConnector};

fn memory_profile(read_only: bool) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("Memory", Engine::SQLite, "", ":memory:");
    profile.read_only = read_only;
    profile
}

#[tokio::test]
async fn connects_runs_queries_and_maps_types() {
    let connector = SqliteConnector::new();
    let mut session = connector
        .connect(&memory_profile(false), None)
        .await
        .expect("connect");

    session
        .execute("CREATE TABLE users (id INTEGER, name TEXT, score REAL, data BLOB, note TEXT)")
        .await
        .expect("create table");
    session
        .execute("INSERT INTO users VALUES (1, 'Ada', 9.5, x'00ff', NULL)")
        .await
        .expect("insert");

    let result = session
        .execute("SELECT id, name, score, data, note FROM users")
        .await
        .expect("select");

    assert_eq!(result.columns.len(), 5);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::Text("Ada".to_string()));
    assert_eq!(result.rows[0][2], Value::Float(9.5));
    assert_eq!(result.rows[0][3], Value::Bytes(vec![0x00, 0xff]));
    assert_eq!(result.rows[0][4], Value::Null);
}

#[tokio::test]
async fn statements_report_affected_rows() {
    let connector = SqliteConnector::new();
    let mut session = connector
        .connect(&memory_profile(false), None)
        .await
        .expect("connect");
    session
        .execute("CREATE TABLE notes (id INTEGER)")
        .await
        .expect("create");
    let result = session
        .execute("INSERT INTO notes VALUES (1), (2)")
        .await
        .expect("insert");
    assert_eq!(result.affected_rows, Some(2));
}

#[tokio::test]
async fn read_only_sessions_block_writes() {
    let connector = SqliteConnector::new();
    let mut session = connector
        .connect(&memory_profile(true), None)
        .await
        .expect("connect");

    let error = session
        .execute("CREATE TABLE nope (id INTEGER)")
        .await
        .expect_err("writes are blocked");
    assert_eq!(error.code(), "read_only_blocked");
}

#[tokio::test]
async fn transactions_commit_and_roll_back() {
    let connector = SqliteConnector::new();
    let mut session = connector
        .connect(&memory_profile(false), None)
        .await
        .expect("connect");
    session
        .execute("CREATE TABLE items (id INTEGER)")
        .await
        .expect("create");

    session.begin().await.expect("begin");
    session
        .execute("INSERT INTO items VALUES (1)")
        .await
        .expect("insert");
    session.rollback().await.expect("rollback");

    session.begin().await.expect("begin");
    session
        .execute("INSERT INTO items VALUES (2)")
        .await
        .expect("insert");
    session.commit().await.expect("commit");

    let result = session
        .execute("SELECT id FROM items")
        .await
        .expect("select");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Value::Int(2));
}

#[tokio::test]
async fn cancel_is_reported_as_unsupported_for_sqlite() {
    let connector = SqliteConnector::new();
    let session = connector
        .connect(&memory_profile(false), None)
        .await
        .expect("connect");
    let error = session.cancel().await.expect_err("sqlite cannot cancel");
    assert_eq!(error.code(), "cancel_unsupported");
}
