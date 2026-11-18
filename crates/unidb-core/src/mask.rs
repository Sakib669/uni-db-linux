//! Masking for exports: replace text shaped values with safe constants while
//! keeping numeric and boolean values usable.

use crate::value::Value;

pub const TEXT_MASK: &str = "[masked]";
pub const EMAIL_MASK: &str = "masked@example.test";

/// Masks one value for export.
pub fn mask_value(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::Decimal(_) => value.clone(),
        Value::Text(text) if text.contains('@') => Value::Text(EMAIL_MASK.to_string()),
        Value::Text(_)
        | Value::Uuid(_)
        | Value::Timestamp(_)
        | Value::Json(_)
        | Value::Bytes(_)
        | Value::Array(_) => Value::Text(TEXT_MASK.to_string()),
    }
}

/// Masks every value in every row, preserving shape.
pub fn mask_rows(rows: &[Vec<Value>]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|row| row.iter().map(mask_value).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_text_and_emails_but_keeps_numbers() {
        assert_eq!(
            mask_value(&Value::Text("Ada".to_string())),
            Value::Text(TEXT_MASK.to_string())
        );
        assert_eq!(
            mask_value(&Value::Text("ada@example.com".to_string())),
            Value::Text(EMAIL_MASK.to_string())
        );
        assert_eq!(mask_value(&Value::Int(3)), Value::Int(3));
        assert_eq!(mask_value(&Value::Bool(true)), Value::Bool(true));
        assert_eq!(mask_value(&Value::Null), Value::Null);
        assert_eq!(
            mask_value(&Value::Timestamp(chrono::Utc::now())),
            Value::Text(TEXT_MASK.to_string())
        );
    }

    #[test]
    fn masks_rows_and_keeps_shape() {
        let rows = vec![
            vec![Value::Int(1), Value::Text("Ada".to_string())],
            vec![Value::Int(2), Value::Null],
        ];
        let masked = mask_rows(&rows);
        assert_eq!(masked.len(), 2);
        assert_eq!(masked[0][0], Value::Int(1));
        assert_eq!(masked[0][1], Value::Text(TEXT_MASK.to_string()));
        assert_eq!(masked[1][1], Value::Null);
    }
}
