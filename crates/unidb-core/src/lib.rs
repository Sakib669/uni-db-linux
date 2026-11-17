//! Domain layer for UniDB: engine metadata, the value model, the error model,
//! and connection profiles. No database driver and no UI types live here.

pub mod engine;
pub mod error;
pub mod profile;
pub mod schema;
pub mod transfer;
pub mod value;

pub use engine::{Capabilities, Engine};
pub use error::{ErrorCategory, UniDbError};
pub use profile::{ConnectionProfile, ParsedConnectionUrl, SslMode};
pub use schema::{
    ColumnDesign, SqlDialect, TableColumnInfo, TableDesign, alter_table_sql, create_table_sql,
};
pub use transfer::{
    ImportData, TransferFormat, coerce_value, insert_statements, parse_import, rows_to_csv,
    rows_to_json,
};
pub use value::{Column, QueryResult, Value};
