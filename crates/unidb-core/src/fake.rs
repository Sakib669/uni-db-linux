//! Deterministic fake data generation for development rows. A small seeded
//! generator keeps results reproducible without adding a dependency.

use chrono::{Duration, Utc};

use crate::schema::TableColumnInfo;
use crate::value::Value;

/// The kind of value generated for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeKind {
    Name,
    Email,
    Text,
    Integer,
    Float,
    Boolean,
    Uuid,
    Timestamp,
}

/// Picks a fake kind from a declared column type.
pub fn kind_for(type_name: &str) -> FakeKind {
    let lower = type_name.to_ascii_lowercase();
    if lower.contains("email") {
        FakeKind::Email
    } else if lower.contains("name") || lower.contains("title") || lower.contains("label") {
        FakeKind::Name
    } else if lower.contains("uuid") {
        FakeKind::Uuid
    } else if lower.contains("timestamp")
        || lower.contains("datetime")
        || lower.contains("date")
        || lower.contains("time")
    {
        FakeKind::Timestamp
    } else if lower.contains("bool") {
        FakeKind::Boolean
    } else if lower.contains("int") || lower.contains("serial") {
        FakeKind::Integer
    } else if lower.contains("float")
        || lower.contains("double")
        || lower.contains("real")
        || lower.contains("numeric")
        || lower.contains("decimal")
    {
        FakeKind::Float
    } else {
        FakeKind::Text
    }
}

/// A small deterministic generator (numerical recipes LCG).
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_mul(6364136223846793005).wrapping_add(1),
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn pick<'a>(&mut self, values: &'a [&'a str]) -> &'a str {
        values[(self.next() as usize) % values.len()]
    }
}

const FIRST_NAMES: [&str; 8] = [
    "Ada", "Grace", "Alan", "Edsger", "Barbara", "Donald", "Frances", "Ken",
];
const LAST_NAMES: [&str; 8] = [
    "Lovelace", "Hopper", "Turing", "Dijkstra", "Liskov", "Knuth", "Allen", "Thompson",
];
const WORDS: [&str; 8] = [
    "sample",
    "record",
    "seeded",
    "generated",
    "studio",
    "table",
    "workflow",
    "data",
];

/// Generates fake rows for the insertable columns of a table.
pub fn generate_fake_rows(columns: &[TableColumnInfo], rows: usize, seed: u64) -> Vec<Vec<Value>> {
    let mut generator = Lcg::new(seed);
    let mut rows_out = Vec::with_capacity(rows);
    for index in 0..rows {
        let mut row = Vec::new();
        for column in columns {
            let integer_key =
                column.primary_key && kind_for(&column.type_name) == FakeKind::Integer;
            if integer_key {
                continue;
            }
            row.push(value_for(&mut generator, &column.type_name, index));
        }
        if !row.is_empty() {
            rows_out.push(row);
        }
    }
    rows_out
}

/// Columns that receive generated values, in order.
pub fn insertable_columns(columns: &[TableColumnInfo]) -> Vec<&TableColumnInfo> {
    columns
        .iter()
        .filter(|column| !(column.primary_key && kind_for(&column.type_name) == FakeKind::Integer))
        .collect()
}

fn value_for(generator: &mut Lcg, type_name: &str, index: usize) -> Value {
    match kind_for(type_name) {
        FakeKind::Name => Value::Text(format!(
            "{} {}",
            generator.pick(&FIRST_NAMES),
            generator.pick(&LAST_NAMES)
        )),
        FakeKind::Email => Value::Text(format!(
            "{}.{}@example.test",
            generator.pick(&FIRST_NAMES).to_ascii_lowercase(),
            generator.pick(&LAST_NAMES).to_ascii_lowercase()
        )),
        FakeKind::Text => Value::Text(format!("{} {}", generator.pick(&WORDS), index + 1)),
        FakeKind::Integer => Value::Int((generator.next() % 10_000) as i64),
        FakeKind::Float => Value::Float((generator.next() % 100_000) as f64 / 100.0),
        FakeKind::Boolean => Value::Bool(generator.next().is_multiple_of(2)),
        FakeKind::Uuid => {
            let high = generator.next();
            let low = generator.next();
            Value::Uuid(uuid::Uuid::from_u64_pair(high, low))
        }
        FakeKind::Timestamp => {
            let offset = (generator.next() % 86_400) as i64;
            let now = Utc::now() - Duration::seconds(offset);
            Value::Timestamp(now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<TableColumnInfo> {
        vec![
            TableColumnInfo {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                nullable: false,
                primary_key: true,
                default: None,
            },
            TableColumnInfo {
                name: "name".to_string(),
                type_name: "TEXT".to_string(),
                nullable: false,
                primary_key: false,
                default: None,
            },
            TableColumnInfo {
                name: "email".to_string(),
                type_name: "TEXT".to_string(),
                nullable: true,
                primary_key: false,
                default: None,
            },
            TableColumnInfo {
                name: "score".to_string(),
                type_name: "DOUBLE PRECISION".to_string(),
                nullable: true,
                primary_key: false,
                default: None,
            },
        ]
    }

    #[test]
    fn generation_is_deterministic_and_skips_integer_keys() {
        let first = generate_fake_rows(&columns(), 4, 42);
        let second = generate_fake_rows(&columns(), 4, 42);
        assert_eq!(first, second);
        assert_eq!(first.len(), 4);
        assert_eq!(first[0].len(), 3);
        let other = generate_fake_rows(&columns(), 4, 43);
        assert_ne!(first, other);
        let columns = columns();
        let insertable = insertable_columns(&columns);
        assert_eq!(insertable.len(), 3);
        assert!(insertable.iter().all(|column| column.name != "id"));
    }

    #[test]
    fn kinds_follow_type_families() {
        assert_eq!(kind_for("TEXT"), FakeKind::Text);
        assert_eq!(kind_for("varchar(200)"), FakeKind::Text);
        assert_eq!(kind_for("BOOLEAN"), FakeKind::Boolean);
        assert_eq!(kind_for("BIGINT"), FakeKind::Integer);
        assert_eq!(kind_for("TIMESTAMP WITH TIME ZONE"), FakeKind::Timestamp);
        assert_eq!(kind_for("uuid"), FakeKind::Uuid);
    }
}
