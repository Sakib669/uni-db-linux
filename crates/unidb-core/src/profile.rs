//! Connection profiles. The profile holds no secret; secrets live encrypted in
//! the app store and are addressed by the profile id.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::Engine;
use crate::error::{ErrorCategory, UniDbError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslMode {
    /// Local hosts default to no TLS. Anything that looks remote defaults to
    /// full verification, including certificate and host name.
    pub fn for_host(host: &str) -> Self {
        if is_local_host(host) {
            Self::Disable
        } else {
            Self::VerifyFull
        }
    }

    pub fn requires_tls(self) -> bool {
        !matches!(self, Self::Disable)
    }
}

pub fn is_local_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host.is_empty() || host == "localhost" || host == "127.0.0.1" || host == "::1"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: Uuid,
    pub name: String,
    pub engine: Engine,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub ssl_mode: SslMode,
    pub ssh_profile_id: Option<Uuid>,
    pub read_only: bool,
}

impl ConnectionProfile {
    pub fn new(
        name: impl Into<String>,
        engine: Engine,
        host: impl Into<String>,
        database: impl Into<String>,
    ) -> Self {
        let host = host.into();
        let port = engine.default_port().unwrap_or(0);
        let ssl_mode = SslMode::for_host(&host);
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            engine,
            host,
            port,
            database: database.into(),
            username: String::new(),
            ssl_mode,
            ssh_profile_id: None,
            read_only: false,
        }
    }

    pub fn is_cloud(&self) -> bool {
        !is_local_host(&self.host)
    }

    /// Validation rules from spec 0001: a name is required, a host is required
    /// for every engine except SQLite, and cloud connections require TLS.
    pub fn validate(&self) -> Result<(), UniDbError> {
        if self.name.trim().is_empty() {
            return Err(UniDbError::new(
                "connection_name_required",
                ErrorCategory::Internal,
                "Connection name is required",
            ));
        }
        if self.engine != Engine::SQLite && self.host.trim().is_empty() {
            return Err(UniDbError::new(
                "connection_host_required",
                ErrorCategory::Internal,
                "Host is required for this engine",
            ));
        }
        if self.is_cloud() && !self.ssl_mode.requires_tls() {
            return Err(UniDbError::new(
                "cloud_requires_tls",
                ErrorCategory::Tls,
                "SSL is required for cloud connections",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_postgres_defaults_to_no_tls() {
        let profile = ConnectionProfile::new("Local", Engine::PostgreSQL, "localhost", "app");
        assert_eq!(profile.ssl_mode, SslMode::Disable);
        assert!(!profile.is_cloud());
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn cloud_postgres_defaults_to_full_verification() {
        let profile = ConnectionProfile::new("Neon", Engine::PostgreSQL, "db.neon.tech", "app");
        assert_eq!(profile.ssl_mode, SslMode::VerifyFull);
        assert!(profile.is_cloud());
    }

    #[test]
    fn cloud_connection_without_tls_is_rejected() {
        let mut profile = ConnectionProfile::new("Neon", Engine::PostgreSQL, "db.neon.tech", "app");
        profile.ssl_mode = SslMode::Disable;
        let error = profile.validate().expect_err("cloud requires TLS");
        assert_eq!(error.code(), "cloud_requires_tls");
        assert_eq!(error.category(), ErrorCategory::Tls);
    }

    #[test]
    fn sqlite_does_not_need_a_host() {
        let profile = ConnectionProfile::new("File", Engine::SQLite, "", "app.db");
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn name_is_required() {
        let profile = ConnectionProfile::new("  ", Engine::SQLite, "", "app.db");
        assert!(profile.validate().is_err());
    }
}
