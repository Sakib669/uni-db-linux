//! Schema Doctor: structural analysis, severity ranked findings, and dialect
//! aware DDL suggestions. Operates strictly on schema metadata with zero row
//! payloads or secrets.

use crate::schema::{ForeignKeyInfo, SqlDialect, TableColumnInfo};

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSeverity {
    Error,
    Warning,
    Info,
}

impl FindingSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Info => "Info",
        }
    }
}

/// Introspected metadata for one table fed into [`diagnose_schema`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchemaInput {
    pub table_name: String,
    pub columns: Vec<TableColumnInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

/// One schema issue with optional dialect SQL remediation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaFinding {
    pub severity: FindingSeverity,
    pub table: String,
    pub column: Option<String>,
    pub title: String,
    pub explanation: String,
    pub suggested_sql: Option<String>,
}

/// Aggregated schema health report across scanned tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaHealthReport {
    pub score: u8,
    pub tables_scanned: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub findings: Vec<SchemaFinding>,
    pub prompt_summary: String,
}

/// Analyzes table schemas and foreign keys, returning a scored health report
/// and ready to run SQL fixes.
pub fn diagnose_schema(dialect: SqlDialect, tables: &[TableSchemaInput]) -> SchemaHealthReport {
    let mut findings = Vec::new();

    for table in tables {
        let q_table = dialect.quote(&table.table_name);
        if table.columns.is_empty() {
            findings.push(SchemaFinding {
                severity: FindingSeverity::Error,
                table: table.table_name.clone(),
                column: None,
                title: "Table has no introspected columns".to_string(),
                explanation: "No columns were returned for this table.".to_string(),
                suggested_sql: None,
            });
            continue;
        }

        let has_pk = table.columns.iter().any(|col| col.primary_key);
        if !has_pk {
            let id_col = table
                .columns
                .iter()
                .find(|col| col.name.eq_ignore_ascii_case("id"));
            let fix = match id_col {
                Some(col) => Some(format!(
                    "CREATE UNIQUE INDEX idx_{}_id ON {} ({});",
                    sanitize_ident(&table.table_name),
                    q_table,
                    dialect.quote(&col.name)
                )),
                None => Some(match dialect {
                    SqlDialect::Mysql => format!(
                        "ALTER TABLE {q_table} ADD COLUMN id BIGINT AUTO_INCREMENT PRIMARY KEY;"
                    ),
                    SqlDialect::Postgres => {
                        format!("ALTER TABLE {q_table} ADD COLUMN id BIGSERIAL PRIMARY KEY;")
                    }
                    SqlDialect::Sqlite => {
                        format!("ALTER TABLE {q_table} ADD COLUMN external_id TEXT;")
                    }
                }),
            };
            findings.push(SchemaFinding {
                severity: FindingSeverity::Warning,
                table: table.table_name.clone(),
                column: id_col.map(|col| col.name.clone()),
                title: "Missing primary key".to_string(),
                explanation: "Tables without a primary key cannot enforce row identity cleanly or support efficient replication."
                    .to_string(),
                suggested_sql: fix,
            });
        }

        for fk in &table.foreign_keys {
            let col = table
                .columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&fk.from_column));
            let is_indexed = col.is_some_and(|c| c.primary_key);
            if !is_indexed {
                let q_col = dialect.quote(&fk.from_column);
                let idx_name = format!(
                    "idx_{}_{}",
                    sanitize_ident(&table.table_name),
                    sanitize_ident(&fk.from_column)
                );
                findings.push(SchemaFinding {
                    severity: FindingSeverity::Warning,
                    table: table.table_name.clone(),
                    column: Some(fk.from_column.clone()),
                    title: format!("Unindexed foreign key '{}'", fk.from_column),
                    explanation: format!(
                        "References '{}.{}'; adding an index avoids full table scans on joins and deletes.",
                        fk.to_table, fk.to_column
                    ),
                    suggested_sql: Some(format!("CREATE INDEX {idx_name} ON {q_table} ({q_col});")),
                });
            }
        }

        for col in &table.columns {
            if col.primary_key && col.type_name.to_ascii_lowercase().contains("text") {
                findings.push(SchemaFinding {
                    severity: FindingSeverity::Info,
                    table: table.table_name.clone(),
                    column: Some(col.name.clone()),
                    title: format!("Text primary key '{}'", col.name),
                    explanation:
                        "Variable length text primary keys enlarge secondary indexes; consider integer or UUID keys for write heavy tables."
                            .to_string(),
                    suggested_sql: None,
                });
            }
            if col.nullable && col.name.eq_ignore_ascii_case("created_at") && col.default.is_none()
            {
                findings.push(SchemaFinding {
                    severity: FindingSeverity::Info,
                    table: table.table_name.clone(),
                    column: Some(col.name.clone()),
                    title: "Timestamp column 'created_at' has no default".to_string(),
                    explanation:
                        "Consider setting a CURRENT_TIMESTAMP default so audit times are populated automatically."
                            .to_string(),
                    suggested_sql: Some(match dialect {
                        SqlDialect::Postgres => format!(
                            "ALTER TABLE {q_table} ALTER COLUMN {} SET DEFAULT CURRENT_TIMESTAMP;",
                            dialect.quote(&col.name)
                        ),
                        SqlDialect::Mysql => format!(
                            "ALTER TABLE {q_table} MODIFY COLUMN {} DATETIME DEFAULT CURRENT_TIMESTAMP;",
                            dialect.quote(&col.name)
                        ),
                        SqlDialect::Sqlite => format!(
                            "-- Recreate or backfill {} with DEFAULT CURRENT_TIMESTAMP on {}",
                            col.name, table.table_name
                        ),
                    }),
                });
            }
        }
    }

    let errors = findings
        .iter()
        .filter(|f| f.severity == FindingSeverity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == FindingSeverity::Warning)
        .count();
    let infos = findings
        .iter()
        .filter(|f| f.severity == FindingSeverity::Info)
        .count();

    let penalty = errors
        .saturating_mul(15)
        .saturating_add(warnings.saturating_mul(8))
        .saturating_add(infos.saturating_mul(2));
    let score = 100usize.saturating_sub(penalty) as u8;

    let prompt_summary = build_redacted_summary(tables, score, errors, warnings, infos);

    SchemaHealthReport {
        score,
        tables_scanned: tables.len(),
        errors,
        warnings,
        infos,
        findings,
        prompt_summary,
    }
}

fn sanitize_ident(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn build_redacted_summary(
    tables: &[TableSchemaInput],
    score: u8,
    errors: usize,
    warnings: usize,
    infos: usize,
) -> String {
    let mut lines = vec![format!(
        "Schema health score: {score}/100 ({} tables, {errors} errors, {warnings} warnings, {infos} info)",
        tables.len()
    )];
    for table in tables {
        let cols: Vec<String> = table
            .columns
            .iter()
            .map(|c| {
                let pk = if c.primary_key { " PK" } else { "" };
                let nul = if c.nullable { "" } else { " NOT NULL" };
                format!("{}:{}{}{}", c.name, c.type_name, pk, nul)
            })
            .collect();
        lines.push(format!("TABLE {} ({})", table.table_name, cols.join(", ")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, type_name: &str, pk: bool, nullable: bool) -> TableColumnInfo {
        TableColumnInfo {
            name: name.to_string(),
            type_name: type_name.to_string(),
            nullable,
            default: None,
            primary_key: pk,
        }
    }

    #[test]
    fn flags_missing_pk_and_unindexed_foreign_keys_with_fixes() {
        let tables = vec![
            TableSchemaInput {
                table_name: "audit_events".to_string(),
                columns: vec![
                    col("event_name", "TEXT", false, false),
                    col("created_at", "TEXT", false, true),
                ],
                foreign_keys: vec![],
            },
            TableSchemaInput {
                table_name: "posts".to_string(),
                columns: vec![
                    col("id", "INTEGER", true, false),
                    col("user_id", "INTEGER", false, false),
                    col("title", "TEXT", false, true),
                ],
                foreign_keys: vec![ForeignKeyInfo {
                    from_table: "posts".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                }],
            },
        ];

        let report = diagnose_schema(SqlDialect::Postgres, &tables);
        assert_eq!(report.tables_scanned, 2);
        assert!(report.warnings >= 2);
        assert!(report.score < 100);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.table == "audit_events" && f.title == "Missing primary key")
        );
        let fk_finding = report
            .findings
            .iter()
            .find(|f| f.table == "posts" && f.column.as_deref() == Some("user_id"))
            .expect("fk finding");
        assert!(
            fk_finding
                .suggested_sql
                .as_deref()
                .unwrap_or("")
                .contains("CREATE INDEX idx_posts_user_id")
        );
        assert!(report.prompt_summary.contains("TABLE posts"));
    }

    #[test]
    fn healthy_schema_scores_100() {
        let tables = vec![TableSchemaInput {
            table_name: "users".to_string(),
            columns: vec![
                col("id", "INTEGER", true, false),
                col("name", "TEXT", false, false),
            ],
            foreign_keys: vec![],
        }];
        let report = diagnose_schema(SqlDialect::Sqlite, &tables);
        assert_eq!(report.score, 100);
        assert!(report.findings.is_empty());
    }
}
