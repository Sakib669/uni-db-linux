//! Engine metadata and capability flags. PostgreSQL on Neon, Supabase, or RDS
//! is still PostgreSQL, so cloud is a property of the profile, not the engine.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    PostgreSQL,
    MySQL,
    MariaDB,
    CockroachDB,
    Cassandra,
    Redis,
    MongoDB,
    MSSQL,
    SQLite,
}

impl Engine {
    pub const ALL: [Self; 9] = [
        Self::PostgreSQL,
        Self::MySQL,
        Self::MariaDB,
        Self::CockroachDB,
        Self::Cassandra,
        Self::Redis,
        Self::MongoDB,
        Self::MSSQL,
        Self::SQLite,
    ];

    pub fn display(self) -> &'static str {
        match self {
            Self::PostgreSQL => "PostgreSQL",
            Self::MySQL => "MySQL",
            Self::MariaDB => "MariaDB",
            Self::CockroachDB => "CockroachDB",
            Self::Cassandra => "Cassandra",
            Self::Redis => "Redis",
            Self::MongoDB => "MongoDB",
            Self::MSSQL => "MSSQL",
            Self::SQLite => "SQLite",
        }
    }

    /// Default port, `None` for engines without a network port.
    pub fn default_port(self) -> Option<u16> {
        match self {
            Self::PostgreSQL | Self::CockroachDB => Some(5432),
            Self::MySQL | Self::MariaDB => Some(3306),
            Self::Cassandra => Some(9042),
            Self::Redis => Some(6379),
            Self::MongoDB => Some(27017),
            Self::MSSQL => Some(1433),
            Self::SQLite => None,
        }
    }

    /// URL scheme used by the "use a URL" connection mode.
    pub fn scheme(self) -> &'static str {
        match self {
            Self::PostgreSQL => "postgresql",
            Self::MySQL => "mysql",
            Self::MariaDB => "mysql",
            Self::CockroachDB => "postgresql",
            Self::Cassandra => "cassandra",
            Self::Redis => "redis",
            Self::MongoDB => "mongodb",
            Self::MSSQL => "mssql",
            Self::SQLite => "sqlite",
        }
    }

    /// Per engine dot color used by the UI.
    pub fn color(self) -> &'static str {
        match self {
            Self::PostgreSQL => "#336791",
            Self::MySQL => "#00758F",
            Self::MariaDB => "#C0765A",
            Self::CockroachDB => "#6933FF",
            Self::Cassandra => "#1287B1",
            Self::Redis => "#DC382D",
            Self::MongoDB => "#00ED64",
            Self::MSSQL => "#CC2927",
            Self::SQLite => "#003B57",
        }
    }

    /// What this engine can do. Engines that cannot support a feature say so
    /// here instead of pretending in the session layer.
    pub fn capabilities(self) -> Capabilities {
        match self {
            Self::PostgreSQL | Self::CockroachDB => Capabilities {
                transactions: true,
                cancel: true,
                schema_edit: true,
                multiple_statements: true,
                explain: true,
            },
            Self::MySQL | Self::MariaDB => Capabilities {
                transactions: true,
                cancel: true,
                schema_edit: true,
                multiple_statements: true,
                explain: true,
            },
            Self::MSSQL => Capabilities {
                transactions: true,
                cancel: true,
                schema_edit: true,
                multiple_statements: true,
                explain: true,
            },
            Self::SQLite => Capabilities {
                transactions: true,
                cancel: false,
                schema_edit: true,
                multiple_statements: true,
                explain: true,
            },
            Self::Cassandra => Capabilities {
                transactions: false,
                cancel: false,
                schema_edit: true,
                multiple_statements: false,
                explain: false,
            },
            Self::Redis => Capabilities {
                transactions: true,
                cancel: false,
                schema_edit: false,
                multiple_statements: false,
                explain: false,
            },
            Self::MongoDB => Capabilities {
                transactions: true,
                cancel: true,
                schema_edit: true,
                multiple_statements: false,
                explain: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub transactions: bool,
    pub cancel: bool,
    pub schema_edit: bool,
    pub multiple_statements: bool,
    pub explain: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_has_display_and_color() {
        for engine in Engine::ALL {
            assert!(!engine.display().is_empty());
            assert!(engine.color().starts_with('#'));
        }
    }

    #[test]
    fn sqlite_has_no_port_and_no_server_cancel() {
        assert_eq!(Engine::SQLite.default_port(), None);
        assert!(!Engine::SQLite.capabilities().cancel);
        assert!(Engine::SQLite.capabilities().transactions);
    }

    #[test]
    fn postgres_and_cockroach_share_the_wire_protocol() {
        assert_eq!(Engine::PostgreSQL.scheme(), Engine::CockroachDB.scheme());
        assert_eq!(
            Engine::PostgreSQL.default_port(),
            Engine::CockroachDB.default_port()
        );
    }
}
