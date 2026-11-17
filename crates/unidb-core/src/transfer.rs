//! Import and export: file parsing, type coercion, insert generation, and
//! result formatting. Pure functions so the interface never touches file
//! formats directly.

use serde_json::json;

use crate::error::{ErrorCategory, UniDbError};
use crate::schema::SqlDialect;
use crate::value::{Column, Value};

/// Supported transfer formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferFormat {
    Csv,
    Json,
}

impl TransferFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
        }
    }
}

/// Parsed import data. `None` in a row means an explicit null (JSON null).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// Parses a CSV or JSON file into a table of strings.
pub fn parse_import(
    bytes: &[u8],
    format: TransferFormat,
    has_headers: bool,
    delimiter: u8,
) -> Result<ImportData, UniDbError> {
    match format {
        TransferFormat::Csv => parse_csv(bytes, has_headers, delimiter),
        TransferFormat::Json => parse_json(bytes),
    }
}

fn parse_csv(bytes: &[u8], has_headers: bool, delimiter: u8) -> Result<ImportData, UniDbError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(bytes);
    let mut records: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| {
            UniDbError::new(
                "transfer_parse_failed",
                ErrorCategory::Syntax,
                "The CSV file could not be parsed",
            )
            .with_detail(error.to_string())
        })?;
        records.push(record.iter().map(str::to_string).collect());
    }
    let headers = if has_headers && !records.is_empty() {
        records
            .remove(0)
            .into_iter()
            .map(|header| header.trim_start_matches('\u{feff}').trim().to_string())
            .collect()
    } else {
        let width = records.first().map_or(0, Vec::len);
        (1..=width)
            .map(|index| format!("column_{index}"))
            .collect::<Vec<String>>()
    };
    if headers.is_empty() {
        return Err(UniDbError::new(
            "transfer_parse_failed",
            ErrorCategory::Syntax,
            "The file has no columns",
        ));
    }
    let rows = records
        .into_iter()
        .map(|mut record| {
            record.resize(headers.len(), String::new());
            record
                .into_iter()
                .take(headers.len())
                .map(Some)
                .collect::<Vec<Option<String>>>()
        })
        .collect();
    Ok(ImportData { headers, rows })
}

fn parse_json(bytes: &[u8]) -> Result<ImportData, UniDbError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        UniDbError::new(
            "transfer_parse_failed",
            ErrorCategory::Syntax,
            "The JSON file is not valid UTF-8",
        )
        .with_detail(error.to_string())
    })?;
    let mut objects: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(serde_json::Value::Array(items)) => {
            for item in items {
                match item {
                    serde_json::Value::Object(map) => objects.push(map),
                    _ => {
                        return Err(UniDbError::new(
                            "transfer_parse_failed",
                            ErrorCategory::Syntax,
                            "JSON arrays must contain objects",
                        ));
                    }
                }
            }
        }
        Ok(serde_json::Value::Object(map)) => objects.push(map),
        Ok(_) => {
            return Err(UniDbError::new(
                "transfer_parse_failed",
                ErrorCategory::Syntax,
                "The JSON file must contain an object or an array of objects",
            ));
        }
        Err(_) => {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(serde_json::Value::Object(map)) => objects.push(map),
                    _ => {
                        return Err(UniDbError::new(
                            "transfer_parse_failed",
                            ErrorCategory::Syntax,
                            "The JSON file could not be parsed",
                        ));
                    }
                }
            }
        }
    }
    if objects.is_empty() {
        return Err(UniDbError::new(
            "transfer_parse_failed",
            ErrorCategory::Syntax,
            "The JSON file has no records",
        ));
    }
    let mut headers: Vec<String> = Vec::new();
    for object in &objects {
        for key in object.keys() {
            if !headers.contains(key) {
                headers.push(key.clone());
            }
        }
    }
    let rows = objects
        .into_iter()
        .map(|object| {
            headers
                .iter()
                .map(|header| object.get(header).and_then(json_field))
                .collect()
        })
        .collect();
    Ok(ImportData { headers, rows })
}

fn json_field(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

/// Coerces one text field to the shared value model for a target column type.
pub fn coerce_value(
    field: Option<&str>,
    type_name: &str,
    null_marker: Option<&str>,
) -> Result<Value, String> {
    let Some(text) = field else {
        return Ok(Value::Null);
    };
    if let Some(marker) = null_marker
        && text == marker
    {
        return Ok(Value::Null);
    }
    if text.is_empty() && is_textual(type_name) {
        return Ok(Value::Text(String::new()));
    }
    if text.is_empty() {
        return Err("the value is empty".to_string());
    }
    let lower = type_name.to_ascii_lowercase();
    if lower.contains("bool") || lower == "bit" {
        return match text.to_ascii_lowercase().as_str() {
            "true" | "1" | "t" | "yes" => Ok(Value::Bool(true)),
            "false" | "0" | "f" | "no" => Ok(Value::Bool(false)),
            _ => Err(format!("{text} is not a boolean")),
        };
    }
    if is_integer(&lower) {
        return text
            .trim()
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("{text} is not an integer"));
    }
    if is_float(&lower) {
        return text
            .trim()
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("{text} is not a number"));
    }
    Ok(Value::Text(text.to_string()))
}

fn is_integer(type_name: &str) -> bool {
    type_name.contains("int") || type_name.contains("serial") || type_name.contains("smallint")
}

fn is_float(type_name: &str) -> bool {
    type_name.contains("float")
        || type_name.contains("double")
        || type_name.contains("real")
        || type_name.contains("numeric")
        || type_name.contains("decimal")
        || type_name.contains("money")
}

fn is_textual(type_name: &str) -> bool {
    !is_integer(type_name) && !is_float(type_name) && !type_name.contains("bool")
}

/// Builds batched INSERT statements for already coerced rows.
pub fn insert_statements(
    dialect: SqlDialect,
    table: &str,
    columns: &[String],
    rows: &[Vec<Value>],
    batch_size: usize,
) -> Result<Vec<String>, UniDbError> {
    if columns.is_empty() {
        return Err(invalid("An insert needs at least one column"));
    }
    for column in columns {
        if !crate::schema::valid_identifier(column) {
            return Err(invalid(format!("Invalid column name {column}")));
        }
    }
    if !crate::schema::valid_identifier(table) {
        return Err(invalid(format!("Invalid table name {table}")));
    }
    let batch_size = batch_size.max(1);
    let column_list = columns
        .iter()
        .map(|column| quote_identifier(dialect, column))
        .collect::<Vec<String>>()
        .join(", ");
    let table_name = quote_identifier(dialect, table);
    let mut statements = Vec::new();
    for batch in rows.chunks(batch_size) {
        let mut values = Vec::new();
        for row in batch {
            if row.len() != columns.len() {
                return Err(invalid("A row does not match the column list"));
            }
            let literals = row
                .iter()
                .map(|value| literal(dialect, value))
                .collect::<Result<Vec<String>, UniDbError>>()?;
            values.push(format!("({})", literals.join(", ")));
        }
        statements.push(format!(
            "INSERT INTO {table_name} ({column_list}) VALUES\n  {}",
            values.join(",\n  ")
        ));
    }
    Ok(statements)
}

fn literal(dialect: SqlDialect, value: &Value) -> Result<String, UniDbError> {
    match value {
        Value::Null => Ok("NULL".to_string()),
        Value::Int(number) => Ok(number.to_string()),
        Value::Float(number) => {
            if number.is_finite() {
                Ok(number.to_string())
            } else {
                Err(invalid("The value is not a finite number"))
            }
        }
        Value::Decimal(number) => Ok(number.to_string()),
        Value::Bool(flag) => Ok(if *flag { "TRUE" } else { "FALSE" }.to_string()),
        Value::Text(text) => Ok(quote_text(dialect, text)),
        other => Err(UniDbError::new(
            "transfer_value_unsupported",
            ErrorCategory::Unsupported,
            format!("Importing {} values is not supported", other.display_text()),
        )),
    }
}

fn quote_text(dialect: SqlDialect, text: &str) -> String {
    match dialect {
        SqlDialect::Mysql => format!("'{}'", text.replace('\\', "\\\\").replace('\'', "''")),
        _ => format!("'{}'", text.replace('\'', "''")),
    }
}

pub(crate) fn quote_identifier(dialect: SqlDialect, identifier: &str) -> String {
    match dialect {
        SqlDialect::Mysql => format!("`{identifier}`"),
        _ => format!("\"{identifier}\""),
    }
}

/// Formats query rows as CSV with a header row.
pub fn rows_to_csv(columns: &[Column], rows: &[Vec<Value>]) -> Result<String, UniDbError> {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record(columns.iter().map(|column| column.name.as_str()))
        .map_err(|error| transfer_error("The CSV could not be written", error))?;
    for row in rows {
        let record: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(index, _)| match row.get(index) {
                Some(Value::Null) | None => String::new(),
                Some(value) => value.display_text(),
            })
            .collect();
        writer
            .write_record(record)
            .map_err(|error| transfer_error("The CSV could not be written", error))?;
    }
    writer
        .into_inner()
        .map_err(|error| transfer_error("The CSV could not be written", error))
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| invalid("The CSV writer produced invalid UTF-8"))
        })
}

/// Formats query rows as a JSON array of objects.
pub fn rows_to_json(columns: &[Column], rows: &[Vec<Value>]) -> Result<String, UniDbError> {
    let mut objects = Vec::new();
    for row in rows {
        let mut object = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            let value = match row.get(index) {
                Some(Value::Null) | None => serde_json::Value::Null,
                Some(Value::Int(number)) => json!(number),
                Some(Value::Float(number)) => json!(number),
                Some(Value::Bool(flag)) => json!(flag),
                Some(other) => json!(other.display_text()),
            };
            object.insert(column.name.clone(), value);
        }
        objects.push(serde_json::Value::Object(object));
    }
    serde_json::to_string_pretty(&objects).map_err(|_| invalid("The JSON could not be written"))
}

fn invalid(message: impl Into<String>) -> UniDbError {
    UniDbError::new("transfer_invalid", ErrorCategory::Syntax, message.into())
}

fn transfer_error(message: &str, error: impl std::fmt::Display) -> UniDbError {
    UniDbError::new("transfer_write_failed", ErrorCategory::Internal, message)
        .with_detail(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parsing_handles_quotes_headers_and_delimiters() {
        let data = parse_import(
            b"id,name\n1,\"Ada, Lovelace\"\n2,\"He said \"\"hi\"\"\"\n",
            TransferFormat::Csv,
            true,
            b',',
        )
        .expect("parsed");
        assert_eq!(data.headers, vec!["id", "name"]);
        assert_eq!(data.rows.len(), 2);
        assert_eq!(data.rows[0][1].as_deref(), Some("Ada, Lovelace"));
        assert_eq!(data.rows[1][1].as_deref(), Some("He said \"hi\""));

        let semicolons =
            parse_import(b"1;a\n2;b\n", TransferFormat::Csv, false, b';').expect("parsed");
        assert_eq!(semicolons.headers, vec!["column_1", "column_2"]);
        assert_eq!(semicolons.rows.len(), 2);
    }

    #[test]
    fn json_parsing_accepts_arrays_and_lines() {
        let array = parse_import(
            br#"[{"id": 1, "name": "Ada"}, {"id": 2, "name": null}]"#,
            TransferFormat::Json,
            true,
            b',',
        )
        .expect("parsed");
        assert_eq!(array.headers, vec!["id", "name"]);
        assert_eq!(array.rows[0][0].as_deref(), Some("1"));
        assert_eq!(array.rows[1][1], None);

        let lines = parse_import(
            b"{\"a\": 1}\n{\"a\": 2}\n",
            TransferFormat::Json,
            true,
            b',',
        )
        .expect("parsed");
        assert_eq!(lines.rows.len(), 2);
    }

    #[test]
    fn coercion_respects_types_and_nulls() {
        assert_eq!(
            coerce_value(Some("42"), "INTEGER", None).expect("int"),
            Value::Int(42)
        );
        assert_eq!(
            coerce_value(Some("1.5"), "DOUBLE PRECISION", None).expect("float"),
            Value::Float(1.5)
        );
        assert_eq!(
            coerce_value(Some("true"), "BOOLEAN", None).expect("bool"),
            Value::Bool(true)
        );
        assert_eq!(
            coerce_value(Some("N/A"), "INTEGER", Some("N/A")).expect("null marker"),
            Value::Null
        );
        assert_eq!(
            coerce_value(None, "INTEGER", None).expect("json null"),
            Value::Null
        );
        assert!(coerce_value(Some("abc"), "INTEGER", None).is_err());
    }

    #[test]
    fn insert_statements_quote_and_batch() {
        let columns = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec![Value::Int(1), Value::Text("Ada's".to_string())],
            vec![Value::Int(2), Value::Null],
            vec![Value::Int(3), Value::Text("Grace".to_string())],
        ];
        let statements =
            insert_statements(SqlDialect::Sqlite, "people", &columns, &rows, 2).expect("generated");
        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("'Ada''s'"));
        assert!(statements[0].contains("VALUES"));
        assert!(statements[0].contains("NULL"));
    }

    #[test]
    fn export_formats_rows_honestly() {
        let columns = vec![
            Column {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                nullable: false,
            },
            Column {
                name: "name".to_string(),
                type_name: "TEXT".to_string(),
                nullable: true,
            },
        ];
        let rows = vec![
            vec![Value::Int(1), Value::Text("Ada".to_string())],
            vec![Value::Int(2), Value::Null],
        ];
        let csv = rows_to_csv(&columns, &rows).expect("csv");
        assert!(csv.starts_with("id,name"));
        assert!(csv.contains("1,Ada"));
        assert!(csv.contains("2,"));

        let json = rows_to_json(&columns, &rows).expect("json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed[0]["name"], "Ada");
        assert!(parsed[1]["name"].is_null());
    }
}
