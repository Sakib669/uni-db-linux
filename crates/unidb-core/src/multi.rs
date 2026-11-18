//! Multi database query result merging. Aligns heterogeneous column lists across
//! multiple connections and prefixes each row with `_connection`.

use uuid::Uuid;

use crate::value::{Column, QueryResult, Value};

/// Execution summary for one target connection in a multi database query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiTargetSummary {
    pub connection_id: Uuid,
    pub connection_name: String,
    pub engine: String,
    pub row_count: usize,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// Raw outcome for one connection passed into [`merge_multi_results`].
#[derive(Debug, Clone)]
pub struct MultiTargetOutcome {
    pub connection_id: Uuid,
    pub connection_name: String,
    pub engine: String,
    pub elapsed_ms: u64,
    pub result: Result<QueryResult, String>,
}

/// Merges multiple connection results into one `QueryResult` prefixed with `_connection`.
/// Caps total rows at `max_rows`.
pub fn merge_multi_results(
    outcomes: &[MultiTargetOutcome],
    max_rows: usize,
) -> (Vec<MultiTargetSummary>, QueryResult) {
    let mut summaries = Vec::with_capacity(outcomes.len());
    let mut columns = vec![Column {
        name: "_connection".to_string(),
        type_name: "TEXT".to_string(),
        nullable: false,
    }];
    let mut column_names: Vec<String> = Vec::new();
    let mut total_elapsed: u64 = 0;

    for outcome in outcomes {
        total_elapsed = total_elapsed.max(outcome.elapsed_ms);
        match &outcome.result {
            Ok(result) => {
                for column in &result.columns {
                    if !column_names
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&column.name))
                    {
                        column_names.push(column.name.clone());
                        columns.push(column.clone());
                    }
                }
                summaries.push(MultiTargetSummary {
                    connection_id: outcome.connection_id,
                    connection_name: outcome.connection_name.clone(),
                    engine: outcome.engine.clone(),
                    row_count: result.rows.len(),
                    elapsed_ms: outcome.elapsed_ms,
                    error: None,
                });
            }
            Err(message) => {
                summaries.push(MultiTargetSummary {
                    connection_id: outcome.connection_id,
                    connection_name: outcome.connection_name.clone(),
                    engine: outcome.engine.clone(),
                    row_count: 0,
                    elapsed_ms: outcome.elapsed_ms,
                    error: Some(message.clone()),
                });
            }
        }
    }

    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut total_affected: Option<u64> = None;

    for outcome in outcomes {
        let Ok(result) = &outcome.result else {
            continue;
        };
        if let Some(affected) = result.affected_rows {
            total_affected = Some(total_affected.unwrap_or(0).saturating_add(affected));
        }
        let col_positions: Vec<Option<usize>> = column_names
            .iter()
            .map(|target| {
                result
                    .columns
                    .iter()
                    .position(|column| column.name.eq_ignore_ascii_case(target))
            })
            .collect();

        for row in &result.rows {
            if rows.len() >= max_rows {
                break;
            }
            let mut merged = Vec::with_capacity(columns.len());
            merged.push(Value::Text(outcome.connection_name.clone()));
            for pos in &col_positions {
                let value = pos
                    .and_then(|index| row.get(index))
                    .cloned()
                    .unwrap_or(Value::Null);
                merged.push(value);
            }
            rows.push(merged);
        }
    }

    let merged = QueryResult {
        columns: if column_names.is_empty() {
            Vec::new()
        } else {
            columns
        },
        rows,
        affected_rows: total_affected,
        elapsed_ms: total_elapsed,
    };

    (summaries, merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_heterogeneous_results_with_connection_column() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let outcomes = vec![
            MultiTargetOutcome {
                connection_id: id_a,
                connection_name: "Primary".to_string(),
                engine: "SQLite".to_string(),
                elapsed_ms: 4,
                result: Ok(QueryResult {
                    columns: vec![
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
                    ],
                    rows: vec![vec![Value::Int(1), Value::Text("Ada".to_string())]],
                    affected_rows: None,
                    elapsed_ms: 4,
                }),
            },
            MultiTargetOutcome {
                connection_id: id_b,
                connection_name: "Replica".to_string(),
                engine: "PostgreSQL".to_string(),
                elapsed_ms: 9,
                result: Ok(QueryResult {
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            type_name: "INT".to_string(),
                            nullable: false,
                        },
                        Column {
                            name: "email".to_string(),
                            type_name: "TEXT".to_string(),
                            nullable: true,
                        },
                    ],
                    rows: vec![vec![
                        Value::Int(2),
                        Value::Text("grace@example.test".to_string()),
                    ]],
                    affected_rows: None,
                    elapsed_ms: 9,
                }),
            },
        ];

        let (summaries, merged) = merge_multi_results(&outcomes, 500);
        assert_eq!(summaries.len(), 2);
        assert_eq!(merged.columns.len(), 4);
        assert_eq!(merged.columns[0].name, "_connection");
        assert_eq!(merged.columns[1].name, "id");
        assert_eq!(merged.columns[2].name, "name");
        assert_eq!(merged.columns[3].name, "email");
        assert_eq!(merged.rows.len(), 2);
        assert_eq!(merged.rows[0][0], Value::Text("Primary".to_string()));
        assert_eq!(merged.rows[0][3], Value::Null);
        assert_eq!(merged.rows[1][0], Value::Text("Replica".to_string()));
        assert_eq!(merged.rows[1][2], Value::Null);
        assert_eq!(
            merged.rows[1][3],
            Value::Text("grace@example.test".to_string())
        );
    }

    #[test]
    fn captures_errors_from_failing_target_without_dropping_other_targets() {
        let outcomes = vec![
            MultiTargetOutcome {
                connection_id: Uuid::new_v4(),
                connection_name: "Good".to_string(),
                engine: "SQLite".to_string(),
                elapsed_ms: 2,
                result: Ok(QueryResult {
                    columns: vec![Column {
                        name: "n".to_string(),
                        type_name: "INT".to_string(),
                        nullable: false,
                    }],
                    rows: vec![vec![Value::Int(42)]],
                    affected_rows: None,
                    elapsed_ms: 2,
                }),
            },
            MultiTargetOutcome {
                connection_id: Uuid::new_v4(),
                connection_name: "Broken".to_string(),
                engine: "SQLite".to_string(),
                elapsed_ms: 1,
                result: Err("table missing".to_string()),
            },
        ];

        let (summaries, merged) = merge_multi_results(&outcomes, 500);
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].error.is_none());
        assert_eq!(summaries[1].error.as_deref(), Some("table missing"));
        assert_eq!(merged.rows.len(), 1);
    }
}
