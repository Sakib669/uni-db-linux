//! Domain layer for UniDB: engine metadata, the value model, the error model,
//! and connection profiles. No database driver and no UI types live here.

pub mod builder;
pub mod bundle;
pub mod doctor;
pub mod engine;
pub mod error;
pub mod fake;
pub mod mask;
pub mod multi;
pub mod plugin;
pub mod profile;
pub mod schema;
pub mod sql_split;
pub mod transfer;
pub mod value;
pub mod visual;

pub use builder::{FilterOp, QueryBuilder, QueryFilter, build_query};
pub use bundle::{
    BUNDLE_FORMAT_VERSION, BundleConnection, BundleQuery, BundleVerification, TeamBundle,
    inspect_bundle_json,
};
pub use doctor::{
    FindingSeverity, SchemaFinding, SchemaHealthReport, TableSchemaInput, diagnose_schema,
};
pub use engine::{Capabilities, Engine};
pub use error::{ErrorCategory, UniDbError};
pub use fake::{FakeKind, generate_fake_rows, insertable_columns, kind_for};
pub use mask::{EMAIL_MASK, TEXT_MASK, mask_rows, mask_value};
pub use multi::{MultiTargetOutcome, MultiTargetSummary, merge_multi_results};
pub use plugin::{WasmFuncType, WasmPluginModule, sample_metric_plugin_wasm};
pub use profile::{ConnectionProfile, ParsedConnectionUrl, SslMode};
pub use schema::{
    ColumnDesign, ForeignKeyInfo, SqlDialect, TableColumnInfo, TableDesign, alter_table_sql,
    create_table_sql, valid_identifier,
};
pub use sql_split::split_statements;
pub use transfer::{
    ImportData, TransferFormat, coerce_value, insert_statements, parse_import, rows_to_csv,
    rows_to_json,
};
pub use value::{Column, QueryResult, Value};
pub use visual::{
    ChartPoint, DiffChange, DiffRow, ErdBox, ErdEdge, ErdLayout, ErdTable, ResultDiff,
    chart_series, diff_results, erd_layout, explain_statement, numeric_type,
};
