//! Table design model and dialect specific DDL generation. The table designer
//! asks this module for statements, so the preview and the executed SQL are
//! always the same text.

use crate::engine::Engine;
use crate::error::{ErrorCategory, UniDbError};

/// SQL dialects the designer knows how to speak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Sqlite,
    Postgres,
    Mysql,
}

impl SqlDialect {
    pub fn for_engine(engine: Engine) -> Option<Self> {
        match engine {
            Engine::SQLite => Some(Self::Sqlite),
            Engine::PostgreSQL | Engine::CockroachDB => Some(Self::Postgres),
            Engine::MySQL | Engine::MariaDB => Some(Self::Mysql),
            _ => None,
        }
    }

    pub fn quote(self, identifier: &str) -> String {
        match self {
            Self::Mysql => format!("`{identifier}`"),
            _ => format!("\"{identifier}\""),
        }
    }
}

/// One column in a design. `original_name` is set when the column was loaded
/// from an existing table; the designer uses it to detect renames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDesign {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
    pub unique: bool,
    pub auto_increment: bool,
    pub original_name: Option<String>,
}

impl ColumnDesign {
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            nullable: true,
            default: None,
            primary_key: false,
            unique: false,
            auto_increment: false,
            original_name: None,
        }
    }
}

/// A full table design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDesign {
    pub name: String,
    pub columns: Vec<ColumnDesign>,
}

impl TableDesign {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
        }
    }
}

/// One foreign key relationship between two tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeyInfo {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

/// Column metadata returned by introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnInfo {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default: Option<String>,
}

impl TableColumnInfo {
    pub fn to_design(&self) -> ColumnDesign {
        ColumnDesign {
            name: self.name.clone(),
            type_name: self.type_name.clone(),
            nullable: self.nullable,
            default: self.default.clone(),
            primary_key: self.primary_key,
            unique: false,
            auto_increment: false,
            original_name: Some(self.name.clone()),
        }
    }
}

/// Validates a design and returns its statements.
pub fn create_table_sql(
    dialect: SqlDialect,
    design: &TableDesign,
) -> Result<Vec<String>, UniDbError> {
    validate(design)?;
    let mut definitions: Vec<String> = design
        .columns
        .iter()
        .map(|column| column_definition(dialect, column, design))
        .collect();
    let primary: Vec<&ColumnDesign> = design
        .columns
        .iter()
        .filter(|column| column.primary_key)
        .collect();
    let inline_primary =
        matches!(dialect, SqlDialect::Sqlite) && primary.len() == 1 && primary[0].auto_increment;
    if !primary.is_empty() && !inline_primary {
        let names: Vec<String> = primary
            .iter()
            .map(|column| dialect.quote(&column.name))
            .collect();
        definitions.push(format!("PRIMARY KEY ({})", names.join(", ")));
    }
    let statement = format!(
        "CREATE TABLE {} (\n  {}\n)",
        dialect.quote(&design.name),
        definitions.join(",\n  ")
    );
    Ok(vec![statement])
}

/// Generates the statements that turn `original` into the current design.
/// Type and default changes are not part of this version.
pub fn alter_table_sql(
    dialect: SqlDialect,
    table: &str,
    original: &[TableColumnInfo],
    design: &TableDesign,
) -> Result<Vec<String>, UniDbError> {
    validate(design)?;
    let table_name = dialect.quote(table);
    let mut statements = Vec::new();

    let mut matched = vec![false; design.columns.len()];
    for old in original {
        let found =
            design
                .columns
                .iter()
                .enumerate()
                .find(|(_, column)| column.original_name.as_deref() == Some(old.name.as_str()))
                .or_else(|| {
                    design.columns.iter().enumerate().find(|(_, column)| {
                        column.original_name.is_none() && column.name == old.name
                    })
                });
        match found {
            Some((index, column)) => {
                matched[index] = true;
                if column.name != old.name {
                    statements.push(format!(
                        "ALTER TABLE {table_name} RENAME COLUMN {} TO {}",
                        dialect.quote(&old.name),
                        dialect.quote(&column.name)
                    ));
                }
            }
            None => {
                let quoted = dialect.quote(&old.name);
                let statement = match dialect {
                    SqlDialect::Mysql => format!("ALTER TABLE {table_name} DROP COLUMN {quoted}"),
                    _ => format!("ALTER TABLE {table_name} DROP COLUMN {quoted}"),
                };
                statements.push(statement);
            }
        }
    }

    for (index, column) in design.columns.iter().enumerate() {
        if matched[index] || column.original_name.is_some() {
            continue;
        }
        statements.push(format!(
            "ALTER TABLE {table_name} ADD COLUMN {}",
            column_definition(dialect, column, design)
        ));
    }
    Ok(statements)
}

fn column_definition(dialect: SqlDialect, column: &ColumnDesign, design: &TableDesign) -> String {
    let quoted = dialect.quote(&column.name);
    let type_name = column.type_name.trim();
    let mut parts = vec![quoted, type_name.to_string()];
    if column.auto_increment {
        parts.retain(|part| part != type_name);
        match dialect {
            SqlDialect::Sqlite => {
                parts.push("INTEGER".to_string());
                parts.push("PRIMARY KEY".to_string());
                parts.push("AUTOINCREMENT".to_string());
            }
            SqlDialect::Postgres => {
                let serial = if type_name.to_ascii_uppercase().contains("BIGINT") {
                    "BIGSERIAL"
                } else {
                    "SERIAL"
                };
                parts.push(serial.to_string());
            }
            SqlDialect::Mysql => {
                parts.push(type_name.to_string());
                parts.push("AUTO_INCREMENT".to_string());
            }
        }
    }
    if !column.nullable
        && !column.auto_increment
        && !(column.primary_key && dialect == SqlDialect::Postgres)
    {
        parts.push("NOT NULL".to_string());
    }
    if dialect == SqlDialect::Sqlite && column.primary_key && !column.auto_increment {
        parts.push("PRIMARY KEY".to_string());
    }
    if column.unique {
        parts.push("UNIQUE".to_string());
    }
    if let Some(default) = &column.default
        && !default.trim().is_empty()
    {
        parts.push(format!("DEFAULT {}", default.trim()));
    }
    let _ = design;
    parts.join(" ")
}

fn validate(design: &TableDesign) -> Result<(), UniDbError> {
    validate_identifier(&design.name, "table")?;
    if design.columns.is_empty() {
        return Err(invalid("A table needs at least one column"));
    }
    let mut seen: Vec<String> = Vec::new();
    let mut auto_columns = 0;
    for column in &design.columns {
        validate_identifier(&column.name, "column")?;
        let key = column.name.to_ascii_lowercase();
        if seen.contains(&key) {
            return Err(invalid(format!("Duplicate column name {}", column.name)));
        }
        seen.push(key);
        if column.type_name.trim().is_empty() {
            return Err(invalid(format!("Column {} has no type", column.name)));
        }
        if let Some(default) = &column.default
            && (default.contains(';') || default.contains('\n'))
        {
            return Err(invalid(format!(
                "The default for {} may not contain semicolons or newlines",
                column.name
            )));
        }
        if column.auto_increment {
            auto_columns += 1;
            let integer = column.type_name.to_ascii_lowercase().contains("int");
            if !integer {
                return Err(invalid(format!(
                    "Auto increment requires an integer type on {}",
                    column.name
                )));
            }
            if !column.primary_key {
                return Err(invalid(format!(
                    "Auto increment requires a primary key on {}",
                    column.name
                )));
            }
        }
    }
    if auto_columns > 1 {
        return Err(invalid("Only one column can auto increment"));
    }
    Ok(())
}

pub(crate) fn validate_identifier(identifier: &str, kind: &str) -> Result<(), UniDbError> {
    let bytes = identifier.as_bytes();
    let valid = !bytes.is_empty()
        && (bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "The {kind} name {identifier} is not a valid identifier"
        )))
    }
}

fn invalid(message: impl Into<String>) -> UniDbError {
    UniDbError::new(
        "table_design_invalid",
        ErrorCategory::Syntax,
        message.into(),
    )
}

/// Returns true when the identifier is a safe unquoted SQL identifier.
pub fn valid_identifier(identifier: &str) -> bool {
    validate_identifier(identifier, "identifier").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn people() -> TableDesign {
        let mut id = ColumnDesign::new("id", "BIGINT");
        id.primary_key = true;
        id.auto_increment = true;
        id.nullable = false;
        let mut name = ColumnDesign::new("name", "TEXT");
        name.nullable = false;
        name.default = Some("'Ada'".to_string());
        let mut email = ColumnDesign::new("email", "TEXT");
        email.unique = true;
        let mut design = TableDesign::new("people");
        design.columns = vec![id, name, email];
        design
    }

    #[test]
    fn sqlite_ddl_uses_autoincrement_primary_key() {
        let sql = create_table_sql(SqlDialect::Sqlite, &people()).expect("valid");
        assert_eq!(sql.len(), 1);
        assert!(sql[0].contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(sql[0].contains("\"name\" TEXT NOT NULL DEFAULT 'Ada'"));
        assert!(sql[0].contains("\"email\" TEXT UNIQUE"));
    }

    #[test]
    fn postgres_ddl_uses_serial_and_table_primary_key() {
        let sql = create_table_sql(SqlDialect::Postgres, &people()).expect("valid");
        assert!(sql[0].contains("\"id\" BIGSERIAL"));
        assert!(sql[0].contains("PRIMARY KEY (\"id\")"));
    }

    #[test]
    fn mysql_ddl_uses_backticks_and_auto_increment() {
        let sql = create_table_sql(SqlDialect::Mysql, &people()).expect("valid");
        assert!(sql[0].contains("`id` BIGINT AUTO_INCREMENT"));
        assert!(sql[0].contains("PRIMARY KEY (`id`)"));
    }

    #[test]
    fn validation_rejects_bad_designs() {
        let mut empty = TableDesign::new("t");
        assert_eq!(
            create_table_sql(SqlDialect::Sqlite, &empty)
                .expect_err("empty")
                .code(),
            "table_design_invalid"
        );
        empty.columns.push(ColumnDesign::new("bad name", "TEXT"));
        assert!(create_table_sql(SqlDialect::Sqlite, &empty).is_err());

        let mut duplicate = TableDesign::new("t");
        duplicate.columns.push(ColumnDesign::new("a", "TEXT"));
        duplicate.columns.push(ColumnDesign::new("A", "INTEGER"));
        assert!(create_table_sql(SqlDialect::Sqlite, &duplicate).is_err());

        let mut inject = TableDesign::new("t");
        let mut column = ColumnDesign::new("a", "TEXT");
        column.default = Some("'x'; DROP TABLE t".to_string());
        inject.columns.push(column);
        assert!(create_table_sql(SqlDialect::Sqlite, &inject).is_err());
    }

    #[test]
    fn alter_detects_drops_renames_and_adds() {
        let original = vec![
            TableColumnInfo {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                nullable: false,
                primary_key: true,
                default: None,
            },
            TableColumnInfo {
                name: "old_name".to_string(),
                type_name: "TEXT".to_string(),
                nullable: true,
                primary_key: false,
                default: None,
            },
            TableColumnInfo {
                name: "gone".to_string(),
                type_name: "TEXT".to_string(),
                nullable: true,
                primary_key: false,
                default: None,
            },
        ];
        let mut design = TableDesign::new("people");
        let mut id = original[0].to_design();
        id.primary_key = true;
        let mut renamed = original[1].to_design();
        renamed.name = "new_name".to_string();
        design.columns = vec![id, renamed, ColumnDesign::new("fresh", "TEXT")];
        let statements =
            alter_table_sql(SqlDialect::Sqlite, "people", &original, &design).expect("valid alter");
        assert!(
            statements
                .iter()
                .any(|sql| sql.contains("RENAME COLUMN \"old_name\" TO \"new_name\""))
        );
        assert!(
            statements
                .iter()
                .any(|sql| sql.contains("DROP COLUMN \"gone\""))
        );
        assert!(
            statements
                .iter()
                .any(|sql| sql.contains("ADD COLUMN \"fresh\" TEXT"))
        );
    }
}
