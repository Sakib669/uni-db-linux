//! Visual query builder: a small, dialect aware SELECT generator. The preview
//! and the editor receive the same text because both come from this module.

use crate::error::{ErrorCategory, UniDbError};
use crate::schema::SqlDialect;
use crate::transfer::quote_identifier;

/// A comparison operator in a builder filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterEqual,
    LessThan,
    LessEqual,
    Like,
    IsNull,
    IsNotNull,
}

impl FilterOp {
    pub const ALL: [Self; 9] = [
        Self::Equals,
        Self::NotEquals,
        Self::GreaterThan,
        Self::GreaterEqual,
        Self::LessThan,
        Self::LessEqual,
        Self::Like,
        Self::IsNull,
        Self::IsNotNull,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Equals => "=",
            Self::NotEquals => "<>",
            Self::GreaterThan => ">",
            Self::GreaterEqual => ">=",
            Self::LessThan => "<",
            Self::LessEqual => "<=",
            Self::Like => "LIKE",
            Self::IsNull => "IS NULL",
            Self::IsNotNull => "IS NOT NULL",
        }
    }

    pub fn needs_value(self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

/// One filter in a builder query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryFilter {
    pub column: String,
    pub operator: FilterOp,
    pub value: String,
}

/// A single table SELECT built by the visual builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryBuilder {
    pub table: String,
    pub columns: Vec<String>,
    pub filters: Vec<QueryFilter>,
    pub order_by: Option<String>,
    pub ascending: bool,
    pub limit: Option<u32>,
}

impl QueryBuilder {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            columns: Vec::new(),
            filters: Vec::new(),
            order_by: None,
            ascending: true,
            limit: Some(100),
        }
    }
}

/// Generates the SELECT statement for a builder design.
pub fn build_query(builder: &QueryBuilder, dialect: SqlDialect) -> Result<String, UniDbError> {
    if !crate::schema::valid_identifier(&builder.table) {
        return Err(invalid("Choose a table before building the query"));
    }
    if builder.columns.is_empty() {
        return Err(invalid("Choose at least one column"));
    }
    let mut selected = Vec::new();
    for column in &builder.columns {
        if !crate::schema::valid_identifier(column) {
            return Err(invalid(format!("Invalid column name {column}")));
        }
        selected.push(quote_identifier(dialect, column));
    }
    if let Some(order) = &builder.order_by
        && !crate::schema::valid_identifier(order)
    {
        return Err(invalid(format!("Invalid order column {order}")));
    }
    if matches!(builder.limit, Some(0)) {
        return Err(invalid("The row limit must be greater than zero"));
    }

    let mut sql = format!(
        "SELECT {} FROM {}",
        selected.join(", "),
        quote_identifier(dialect, &builder.table)
    );
    if !builder.filters.is_empty() {
        let mut clauses = Vec::new();
        for filter in &builder.filters {
            if !crate::schema::valid_identifier(&filter.column) {
                return Err(invalid(format!("Invalid filter column {}", filter.column)));
            }
            let column = quote_identifier(dialect, &filter.column);
            if filter.operator.needs_value() {
                if filter.value.is_empty() {
                    return Err(invalid(format!(
                        "The filter on {} needs a value",
                        filter.column
                    )));
                }
                let value = match filter.operator {
                    FilterOp::Like => format!("%{}%", filter.value),
                    _ => filter.value.clone(),
                };
                clauses.push(format!(
                    "{column} {} {}",
                    filter.operator.label(),
                    literal(dialect, &value)
                ));
            } else {
                clauses.push(format!("{column} {}", filter.operator.label()));
            }
        }
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    if let Some(order) = &builder.order_by {
        sql.push_str(&format!(
            " ORDER BY {} {}",
            quote_identifier(dialect, order),
            if builder.ascending { "ASC" } else { "DESC" }
        ));
    }
    if let Some(limit) = builder.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    Ok(sql)
}

fn literal(dialect: SqlDialect, value: &str) -> String {
    match dialect {
        SqlDialect::Mysql => format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''")),
        _ => format!("'{}'", value.replace('\'', "''")),
    }
}

fn invalid(message: impl Into<String>) -> UniDbError {
    UniDbError::new("builder_invalid", ErrorCategory::Syntax, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> QueryBuilder {
        let mut builder = QueryBuilder::new("users");
        builder.columns = vec!["id".to_string(), "name".to_string()];
        builder
    }

    #[test]
    fn builds_a_plain_select_for_each_dialect() {
        for dialect in [SqlDialect::Sqlite, SqlDialect::Postgres, SqlDialect::Mysql] {
            let sql = build_query(&base(), dialect).expect("sql");
            assert!(sql.starts_with("SELECT "));
            assert!(sql.contains("FROM "));
            assert!(sql.contains("id"));
            assert!(sql.contains("LIMIT 100"));
        }
        let mysql = build_query(&base(), SqlDialect::Mysql).expect("mysql");
        assert!(mysql.contains("`users`"), "{mysql}");
        let sqlite = build_query(&base(), SqlDialect::Sqlite).expect("sqlite");
        assert!(sqlite.contains("\"users\""), "{sqlite}");
    }

    #[test]
    fn filters_order_and_limit_are_applied_and_escaped() {
        let mut builder = base();
        builder.filters = vec![
            QueryFilter {
                column: "name".to_string(),
                operator: FilterOp::Equals,
                value: "Ada's".to_string(),
            },
            QueryFilter {
                column: "id".to_string(),
                operator: FilterOp::GreaterThan,
                value: "1".to_string(),
            },
        ];
        builder.order_by = Some("name".to_string());
        builder.ascending = false;
        builder.limit = Some(5);
        let sql = build_query(&builder, SqlDialect::Sqlite).expect("sql");
        assert!(
            sql.contains("WHERE \"name\" = 'Ada''s' AND \"id\" > '1'"),
            "{sql}"
        );
        assert!(sql.contains("ORDER BY \"name\" DESC"), "{sql}");
        assert!(sql.ends_with("LIMIT 5"), "{sql}");
    }

    #[test]
    fn like_null_and_not_null_are_supported() {
        let mut builder = base();
        builder.filters = vec![
            QueryFilter {
                column: "name".to_string(),
                operator: FilterOp::Like,
                value: "da".to_string(),
            },
            QueryFilter {
                column: "id".to_string(),
                operator: FilterOp::IsNull,
                value: String::new(),
            },
        ];
        let sql = build_query(&builder, SqlDialect::Sqlite).expect("sql");
        assert!(sql.contains("\"name\" LIKE '%da%'"), "{sql}");
        assert!(sql.contains("\"id\" IS NULL"), "{sql}");
        builder.filters[1].operator = FilterOp::IsNotNull;
        let sql = build_query(&builder, SqlDialect::Sqlite).expect("sql");
        assert!(sql.contains("\"id\" IS NOT NULL"), "{sql}");
    }

    #[test]
    fn invalid_designs_do_not_produce_sql() {
        let mut builder = QueryBuilder::new("bad name");
        builder.columns = vec!["id".to_string()];
        assert!(build_query(&builder, SqlDialect::Sqlite).is_err());

        let mut empty = base();
        empty.columns.clear();
        assert!(build_query(&empty, SqlDialect::Sqlite).is_err());

        let mut limit = base();
        limit.limit = Some(0);
        assert!(build_query(&limit, SqlDialect::Sqlite).is_err());
    }
}
