//! The engine seam. Every database engine implements the same traits, returns
//! the same `Value` model, and reports the same error taxonomy. The UI and the
//! app services never see a driver specific type.

pub mod mysql;
pub mod postgres;
mod sql;
pub mod sqlite;
pub mod traits;

pub use mysql::MySqlConnector;
pub use postgres::PostgresConnector;
pub use sqlite::SqliteConnector;
pub use traits::{Connector, ConnectorRegistry, Session};
