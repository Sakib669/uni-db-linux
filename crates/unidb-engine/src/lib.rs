//! The engine seam. Every database engine implements the same traits, returns
//! the same `Value` model, and reports the same error taxonomy. The UI and the
//! app services never see a driver specific type.

pub mod cassandra;
pub mod mongodb;
pub mod mssql;
pub mod mysql;
pub mod postgres;
pub mod redis;
mod sql;
pub mod sqlite;
pub mod traits;
mod uri;

pub use cassandra::CassandraConnector;
pub use mongodb::MongoConnector;
pub use mssql::MssqlConnector;
pub use mysql::MySqlConnector;
pub use postgres::PostgresConnector;
pub use redis::RedisConnector;
pub use sqlite::SqliteConnector;
pub use traits::{Connector, ConnectorRegistry, Session};
