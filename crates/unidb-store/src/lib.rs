//! SQLite backed app store: connections, settings, history, audit log, and the
//! master key lifecycle behind the `SecretBox` envelope.

pub mod secrets;
pub mod store;

pub use secrets::{KeyringKeyProvider, MasterKeyProvider, MemoryKeyProvider, SecretBox};
pub use store::{AuditEntry, HistoryEntry, SavedQuery, Store};
