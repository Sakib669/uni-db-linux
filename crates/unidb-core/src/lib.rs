//! Domain layer for UniDB: engine metadata, the value model, the error model,
//! and connection profiles. No database driver and no UI types live here.

pub mod engine;
pub mod error;
pub mod profile;
pub mod schema;
pub mod value;

pub use engine::{Capabilities, Engine};
pub use error::{ErrorCategory, UniDbError};
pub use profile::{ConnectionProfile, ParsedConnectionUrl, SslMode};
pub use schema::{
    ColumnDesign, SqlDialect, TableColumnInfo, TableDesign, alter_table_sql, create_table_sql,
};
pub use value::{Column, QueryResult, Value};
