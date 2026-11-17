//! Visual helpers: explain statement selection per engine and chart series
//! extraction from the shared result model. GTK free so both are unit tested.

use crate::engine::Engine;
use crate::error::{ErrorCategory, UniDbError};
use crate::value::{Column, Value};

/// One bar in a chart.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

/// Builds the engine specific explain statement for one SQL statement.
pub fn explain_statement(engine: Engine, sql: &str) -> Result<String, UniDbError> {
    let statement = sql.trim().trim_end_matches(';').trim();
    if statement.is_empty() {
        return Err(UniDbError::new(
            "explain_empty",
            ErrorCategory::Syntax,
            "There is no statement to explain",
        ));
    }
    let explain = match engine {
        Engine::SQLite => format!("EXPLAIN QUERY PLAN {statement}"),
        Engine::PostgreSQL | Engine::CockroachDB => {
            format!("EXPLAIN (FORMAT JSON) {statement}")
        }
        Engine::MySQL | Engine::MariaDB => format!("EXPLAIN FORMAT=JSON {statement}"),
        other => {
            return Err(UniDbError::new(
                "explain_unsupported",
                ErrorCategory::Unsupported,
                format!("Explain plans are not available for {}", other.display()),
            ));
        }
    };
    Ok(explain)
}

/// True when the declared type is a numeric family.
pub fn numeric_type(type_name: &str) -> bool {
    let lower = type_name.to_ascii_lowercase();
    lower.contains("int")
        || lower.contains("serial")
        || lower.contains("float")
        || lower.contains("double")
        || lower.contains("real")
        || lower.contains("numeric")
        || lower.contains("decimal")
        || lower.contains("money")
}

/// Extracts labelled values for a bar chart: the first numeric column supplies
/// values, the first non numeric column supplies labels, nulls are skipped, and
/// the result is capped at `max` points.
pub fn chart_series(columns: &[Column], rows: &[Vec<Value>], max: usize) -> Vec<ChartPoint> {
    if columns.is_empty() || rows.is_empty() {
        return Vec::new();
    }
    let value_index = columns
        .iter()
        .position(|column| numeric_type(&column.type_name))
        .or_else(|| {
            (0..columns.len()).find(|index| {
                rows.iter().any(|row| {
                    matches!(
                        row.get(*index),
                        Some(Value::Int(_)) | Some(Value::Float(_)) | Some(Value::Decimal(_))
                    )
                })
            })
        });
    let Some(value_index) = value_index else {
        return Vec::new();
    };
    let label_index = (0..columns.len())
        .find(|index| *index != value_index && !numeric_type(&columns[*index].type_name))
        .or_else(|| (0..columns.len()).find(|index| *index != value_index));

    let mut points = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let value = match row.get(value_index) {
            Some(Value::Int(value)) => *value as f64,
            Some(Value::Float(value)) if value.is_finite() => *value,
            Some(Value::Decimal(value)) => value.to_string().parse::<f64>().unwrap_or(0.0),
            _ => continue,
        };
        let label = label_index
            .and_then(|index| row.get(index))
            .map(Value::display_text)
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| format!("#{}", row_index + 1));
        points.push(ChartPoint { label, value });
        if points.len() >= max {
            break;
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_explain_statements_per_engine() {
        assert_eq!(
            explain_statement(Engine::SQLite, "SELECT 1;").expect("sqlite"),
            "EXPLAIN QUERY PLAN SELECT 1"
        );
        assert_eq!(
            explain_statement(Engine::PostgreSQL, "SELECT 1").expect("postgres"),
            "EXPLAIN (FORMAT JSON) SELECT 1"
        );
        assert_eq!(
            explain_statement(Engine::MySQL, "SELECT 1").expect("mysql"),
            "EXPLAIN FORMAT=JSON SELECT 1"
        );
        assert_eq!(
            explain_statement(Engine::CockroachDB, "SELECT 1").expect("cockroach"),
            "EXPLAIN (FORMAT JSON) SELECT 1"
        );
        assert_eq!(
            explain_statement(Engine::Redis, "GET key")
                .expect_err("unsupported")
                .code(),
            "explain_unsupported"
        );
        assert!(explain_statement(Engine::SQLite, "   ; ").is_err());
    }

    fn columns() -> Vec<Column> {
        vec![
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
            Column {
                name: "score".to_string(),
                type_name: "DOUBLE PRECISION".to_string(),
                nullable: true,
            },
        ]
    }

    #[test]
    fn chart_series_uses_the_first_numeric_column_and_a_text_label() {
        let rows = vec![
            vec![
                Value::Int(1),
                Value::Text("Ada".to_string()),
                Value::Float(9.5),
            ],
            vec![Value::Int(2), Value::Text("Grace".to_string()), Value::Null],
        ];
        let series = chart_series(&columns(), &rows, 50);
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].label, "Ada");
        assert_eq!(series[0].value, 1.0);
        assert_eq!(series[1].label, "Grace");
    }

    #[test]
    fn chart_series_skips_nulls_and_caps_points() {
        let rows: Vec<Vec<Value>> = (0..80)
            .map(|index| {
                vec![
                    Value::Int(index),
                    Value::Text(format!("row {index}")),
                    Value::Null,
                ]
            })
            .collect();
        let series = chart_series(&columns(), &rows, 50);
        assert_eq!(series.len(), 50);

        let no_numeric = vec![vec![
            Value::Text("a".to_string()),
            Value::Text("b".to_string()),
            Value::Null,
        ]];
        assert!(chart_series(&columns(), &no_numeric, 50).is_empty());
    }

    #[test]
    fn chart_series_falls_back_to_row_numbers_for_labels() {
        let columns = vec![Column {
            name: "value".to_string(),
            type_name: "INTEGER".to_string(),
            nullable: false,
        }];
        let rows = vec![vec![Value::Int(4)], vec![Value::Int(7)]];
        let series = chart_series(&columns, &rows, 50);
        assert_eq!(series[0].label, "#1");
        assert_eq!(series[1].value, 7.0);
    }
}
