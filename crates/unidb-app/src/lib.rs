//! Application services sit between the GTK interface and the engine and store.
//! The UI never touches a driver directly; it sends commands and receives typed
//! events through one bridge.

pub mod bridge;
pub mod paths;
pub mod security;
pub mod service;

pub use bridge::{Bridge, EventSink, QueryEvent};
pub use paths::default_data_dir;
pub use security::{CheckStatus, SecurityCheck, SecurityReport};
pub use service::{AppService, ExportReport, ImportOptions, ImportReport};
pub use unidb_store::AuditEntry;
pub use unidb_store::{HistoryEntry, SavedQuery};

/// Grid and status text for a value, kept here so the interface never formats
/// a driver type itself.
pub fn value_text(value: &unidb_core::Value) -> String {
    value.display_text()
}
