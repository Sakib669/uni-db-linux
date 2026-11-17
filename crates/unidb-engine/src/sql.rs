//! Small helpers shared by the SQL drivers: statement classification and one
//! error mapping that keeps SQL text out of messages.

use unidb_core::{ErrorCategory, UniDbError};

/// True for statements that return rows. Used for the read only guard and for
/// choosing between `fetch_all` and `execute`.
pub(crate) fn is_query_statement(sql: &str) -> bool {
    let statement = sql.trim_start();
    [
        "select", "with", "values", "show", "describe", "desc", "explain", "pragma", "table",
    ]
    .iter()
    .any(|keyword| {
        statement
            .get(..keyword.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
    })
}

/// Maps a driver error without copying the SQL statement or parameter values
/// into the message. Only the database error code is kept as detail.
pub(crate) fn driver_error(
    code: &'static str,
    user_message: &'static str,
    error: sqlx::Error,
) -> UniDbError {
    let detail = error
        .as_database_error()
        .map(|database_error| {
            format!(
                "database error {}",
                database_error.code().unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "driver error".to_string());
    UniDbError::new(code, ErrorCategory::Internal, user_message)
        .with_detail(detail)
        .with_source(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_queries_and_statements() {
        assert!(is_query_statement("SELECT 1"));
        assert!(is_query_statement("  with x as (select 1) select * from x"));
        assert!(is_query_statement("SHOW TABLES"));
        assert!(is_query_statement("EXPLAIN SELECT 1"));
        assert!(!is_query_statement("INSERT INTO t VALUES (1)"));
        assert!(!is_query_statement("update t set a = 1"));
        assert!(!is_query_statement("CREATE TABLE t (id int)"));
    }
}
