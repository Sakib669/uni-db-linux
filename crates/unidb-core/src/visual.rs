//! Visual helpers: explain statement selection per engine and chart series
//! extraction from the shared result model. GTK free so both are unit tested.

use crate::engine::Engine;
use crate::error::{ErrorCategory, UniDbError};
use crate::schema::ForeignKeyInfo;
use crate::value::{Column, QueryResult, Value};

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

/// How one row differs between two results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffChange {
    Added,
    Removed,
    Changed,
}

impl DiffChange {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

/// One changed row in a result diff.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffRow {
    pub change: DiffChange,
    pub key: String,
    pub before: Vec<Value>,
    pub after: Vec<Value>,
}

/// Summary of a comparison between a baseline result and a current result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResultDiff {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub details: Vec<DiffRow>,
}

impl ResultDiff {
    pub fn has_changes(&self) -> bool {
        self.added + self.removed + self.changed > 0
    }
}

/// Compares two results by a key column. Rows are matched by the display text
/// of the key column, and the first `max_details` differences are kept.
pub fn diff_results(
    baseline: &QueryResult,
    current: &QueryResult,
    key_index: usize,
    max_details: usize,
) -> ResultDiff {
    let mut diff = ResultDiff::default();
    let key = |row: &[Value]| {
        row.get(key_index)
            .or_else(|| row.first())
            .map(Value::display_text)
            .unwrap_or_default()
    };
    let mut current_by_key: Vec<(String, &Vec<Value>)> =
        current.rows.iter().map(|row| (key(row), row)).collect();
    for old in &baseline.rows {
        let old_key = key(old);
        match current_by_key
            .iter()
            .position(|(value, _)| *value == old_key)
        {
            Some(index) => {
                let (_, new) = current_by_key.remove(index);
                if new == old {
                    diff.unchanged += 1;
                } else {
                    diff.changed += 1;
                    if diff.details.len() < max_details {
                        diff.details.push(DiffRow {
                            change: DiffChange::Changed,
                            key: old_key,
                            before: old.clone(),
                            after: new.clone(),
                        });
                    }
                }
            }
            None => {
                diff.removed += 1;
                if diff.details.len() < max_details {
                    diff.details.push(DiffRow {
                        change: DiffChange::Removed,
                        key: old_key,
                        before: old.clone(),
                        after: Vec::new(),
                    });
                }
            }
        }
    }
    for (new_key, new) in current_by_key {
        diff.added += 1;
        if diff.details.len() < max_details {
            diff.details.push(DiffRow {
                change: DiffChange::Added,
                key: new_key,
                before: Vec::new(),
                after: new.clone(),
            });
        }
    }
    diff
}

/// A table in the ERD input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErdTable {
    pub name: String,
    pub columns: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

/// A placed table card.
#[derive(Debug, Clone, PartialEq)]
pub struct ErdBox {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub title: String,
    pub columns: Vec<String>,
    pub foreign_key_columns: Vec<String>,
}

/// A relationship arrow between two cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErdEdge {
    pub from: usize,
    pub to: usize,
    pub label: String,
}

/// Result of laying out the ERD.
#[derive(Debug, Clone, PartialEq)]
pub struct ErdLayout {
    pub boxes: Vec<ErdBox>,
    pub edges: Vec<ErdEdge>,
    pub width: f64,
    pub height: f64,
}

/// Places table cards in a responsive grid and resolves relationship arrows.
pub fn erd_layout(tables: &[ErdTable], canvas_width: f64) -> ErdLayout {
    const CARD_WIDTH: f64 = 220.0;
    const GAP: f64 = 36.0;
    const PADDING: f64 = 20.0;
    const MAX_COLUMNS: usize = 10;
    const HEADER: f64 = 30.0;
    const ROW: f64 = 18.0;

    if tables.is_empty() {
        return ErdLayout {
            boxes: Vec::new(),
            edges: Vec::new(),
            width: canvas_width.max(320.0),
            height: 160.0,
        };
    }
    let usable = (canvas_width - PADDING * 2.0 - GAP).max(CARD_WIDTH);
    let columns = (((usable + GAP) / (CARD_WIDTH + GAP)).floor() as usize).clamp(1, tables.len());
    let tallest = tables
        .iter()
        .map(|table| table.columns.len().min(MAX_COLUMNS))
        .max()
        .unwrap_or(1)
        .max(1);
    let card_height = HEADER + ROW * tallest as f64 + 14.0;
    let rows = tables.len().div_ceil(columns);
    let width =
        (PADDING * 2.0 + columns as f64 * CARD_WIDTH + (columns.saturating_sub(1)) as f64 * GAP)
            .max(canvas_width);
    let height = PADDING * 2.0 + rows as f64 * card_height + rows.saturating_sub(1) as f64 * GAP;

    let mut boxes = Vec::new();
    for (index, table) in tables.iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        boxes.push(ErdBox {
            index,
            x: PADDING + column as f64 * (CARD_WIDTH + GAP),
            y: PADDING + row as f64 * (card_height + GAP),
            width: CARD_WIDTH,
            height: card_height,
            title: table.name.clone(),
            columns: table.columns.iter().take(MAX_COLUMNS).cloned().collect(),
            foreign_key_columns: table
                .foreign_keys
                .iter()
                .map(|key| key.from_column.clone())
                .collect(),
        });
    }

    let mut edges: Vec<ErdEdge> = Vec::new();
    for (from, table) in tables.iter().enumerate() {
        for key in &table.foreign_keys {
            let Some(to) = tables.iter().position(|target| target.name == key.to_table) else {
                continue;
            };
            if to == from {
                continue;
            }
            let edge = ErdEdge {
                from,
                to,
                label: key.from_column.clone(),
            };
            if !edges.contains(&edge) {
                edges.push(edge);
            }
        }
    }

    ErdLayout {
        boxes,
        edges,
        width,
        height,
    }
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

    #[test]
    fn erd_layout_places_cards_and_resolves_edges() {
        let tables = vec![
            ErdTable {
                name: "users".to_string(),
                columns: vec!["id".to_string(), "name".to_string()],
                foreign_keys: Vec::new(),
            },
            ErdTable {
                name: "posts".to_string(),
                columns: vec!["id".to_string(), "user_id".to_string()],
                foreign_keys: vec![ForeignKeyInfo {
                    from_table: "posts".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                }],
            },
            ErdTable {
                name: "orphans".to_string(),
                columns: vec!["id".to_string()],
                foreign_keys: vec![ForeignKeyInfo {
                    from_table: "orphans".to_string(),
                    from_column: "ghost_id".to_string(),
                    to_table: "missing".to_string(),
                    to_column: "id".to_string(),
                }],
            },
        ];
        let layout = erd_layout(&tables, 800.0);
        assert_eq!(layout.boxes.len(), 3);
        assert_eq!(layout.edges.len(), 1);
        assert_eq!(layout.edges[0].from, 1);
        assert_eq!(layout.edges[0].to, 0);
        assert_eq!(layout.edges[0].label, "user_id");
        assert!(
            layout.boxes[1]
                .foreign_key_columns
                .contains(&"user_id".to_string())
        );
        for (index, card) in layout.boxes.iter().enumerate() {
            for other in layout.boxes.iter().skip(index + 1) {
                let overlaps = card.x < other.x + other.width
                    && other.x < card.x + card.width
                    && card.y < other.y + other.height
                    && other.y < card.y + card.height;
                assert!(!overlaps, "cards {index} and {} overlap", other.index);
            }
        }
        assert!(layout.height >= layout.boxes[2].y + layout.boxes[2].height);
    }

    #[test]
    fn erd_layout_drops_self_references() {
        let tables = vec![ErdTable {
            name: "nodes".to_string(),
            columns: vec!["id".to_string(), "parent_id".to_string()],
            foreign_keys: vec![ForeignKeyInfo {
                from_table: "nodes".to_string(),
                from_column: "parent_id".to_string(),
                to_table: "nodes".to_string(),
                to_column: "id".to_string(),
            }],
        }];
        let layout = erd_layout(&tables, 800.0);
        assert_eq!(layout.boxes.len(), 1);
        assert!(layout.edges.is_empty());
    }

    #[test]
    fn diff_finds_added_removed_and_changed_rows() {
        let baseline = QueryResult {
            columns: vec![Column {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                nullable: false,
            }],
            rows: vec![
                vec![Value::Int(1)],
                vec![Value::Int(2)],
                vec![Value::Int(3)],
            ],
            affected_rows: None,
            elapsed_ms: 0,
        };
        let current = QueryResult {
            columns: baseline.columns.clone(),
            rows: vec![
                vec![Value::Int(1)],
                vec![Value::Int(2), Value::Text("changed".to_string())],
                vec![Value::Int(4)],
            ],
            affected_rows: None,
            elapsed_ms: 0,
        };
        let diff = diff_results(&baseline, &current, 0, 10);
        assert_eq!(diff.unchanged, 1);
        assert_eq!(diff.changed, 1);
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.added, 1);
        assert!(diff.has_changes());
        assert_eq!(diff.details.len(), 3);
    }

    #[test]
    fn diff_caps_details_and_handles_empty_results() {
        let baseline = QueryResult {
            columns: Vec::new(),
            rows: vec![vec![Value::Int(1)], vec![Value::Int(2)]],
            affected_rows: None,
            elapsed_ms: 0,
        };
        let current = QueryResult::empty(1);
        let diff = diff_results(&baseline, &current, 5, 1);
        assert_eq!(diff.removed, 2);
        assert_eq!(diff.details.len(), 1);
        let same = diff_results(&QueryResult::empty(1), &QueryResult::empty(1), 0, 10);
        assert!(!same.has_changes());
    }
}
