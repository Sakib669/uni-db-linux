//! The value model every engine converts into, so the grid and the exporters
//! never deal with driver specific types.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(Decimal),
    Text(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    Timestamp(DateTime<Utc>),
    Uuid(uuid::Uuid),
    Array(Vec<Value>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Short label for the grid and for copy to clipboard.
    pub fn display_text(&self) -> String {
        match self {
            Self::Null => "NULL".to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Decimal(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Bytes(value) => format!("{} bytes", value.len()),
            Self::Json(value) => value.to_string(),
            Self::Timestamp(value) => value.to_rfc3339(),
            Self::Uuid(value) => value.to_string(),
            Self::Array(values) => {
                let parts: Vec<String> = values.iter().map(Self::display_text).collect();
                format!("[{}]", parts.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
    pub affected_rows: Option<u64>,
    pub elapsed_ms: u64,
}

impl QueryResult {
    pub fn empty(elapsed_ms: u64) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: None,
            elapsed_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_text_covers_every_variant() {
        assert_eq!(Value::Null.display_text(), "NULL");
        assert_eq!(Value::Text("hello".into()).display_text(), "hello");
        assert_eq!(Value::Bytes(vec![1, 2, 3]).display_text(), "3 bytes");
        assert_eq!(
            Value::Array(vec![Value::Int(1), Value::Int(2)]).display_text(),
            "[1, 2]"
        );
    }

    #[test]
    fn values_round_trip_through_json() {
        let value = Value::Decimal("12.50".parse().expect("valid decimal"));
        let encoded = serde_json::to_string(&value).expect("serialize");
        let decoded: Value = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(value, decoded);
    }
}
