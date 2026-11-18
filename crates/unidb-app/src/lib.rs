//! Application services sit between the GTK interface and the engine and store.
//! The UI never touches a driver directly; it sends commands and receives typed
//! events through one bridge.

pub mod bridge;
pub mod migrations;
pub mod paths;
pub mod security;
pub mod service;
pub mod update;

pub use bridge::{Bridge, EventSink, QueryEvent};
pub use migrations::{
    GitRepoStatus, MigrationFile, MigrationReport, checksum, git_dir_status, load_migrations,
    plan_migrations,
};
pub use paths::default_data_dir;
pub use security::{CheckStatus, SecurityCheck, SecurityReport};
pub use service::{
    AppService, BundleImportReport, BundlePreview, ExportReport, ImportOptions, ImportReport,
    SyncFlushReport,
};
pub use unidb_core::{
    BundleVerification, FindingSeverity, ForeignKeyInfo, MultiTargetSummary, ResultDiff,
    SchemaFinding, SchemaHealthReport, TeamBundle,
};
pub use unidb_store::AuditEntry;
pub use unidb_store::{
    HistoryEntry, SavedQuery, ScheduledJob, SyncQueueItem, TableSnapshot, WasmPluginRecord,
};
pub use update::{ReleaseInfo, fetch_latest, is_newer, parse_release};

/// Grid and status text for a value, kept here so the interface never formats
/// a driver type itself.
pub fn value_text(value: &unidb_core::Value) -> String {
    value.display_text()
}
