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

/// Result of parsing a connection URL: a profile plus a password when the URL
/// carried one. The password is never stored in the profile struct.
#[derive(Debug, Clone)]
pub struct ParsedConnectionUrl {
    pub profile: ConnectionProfile,
    pub password: Option<String>,
}

impl ConnectionProfile {
    /// Parses a connection URL into a profile. Schemes:
    /// postgresql/postgres, cockroachdb, mysql, mariadb, redis, mongodb,
    /// mssql/sqlserver, and sqlite. Query parameters `sslmode` and
    /// `readonly`/`read_only` are understood.
    pub fn from_url(name: impl Into<String>, raw: &str) -> Result<ParsedConnectionUrl, UniDbError> {
        let raw = raw.trim();
        if let Some(rest) = raw.strip_prefix("sqlite:") {
            let path = rest.trim_start_matches('/').to_string();
            let path = if raw.starts_with("sqlite:///") {
                format!("/{path}")
            } else {
                path
            };
            if path.is_empty() {
                return Err(UniDbError::new(
                    "url_invalid",
                    ErrorCategory::Syntax,
                    "The SQLite URL has no file path",
                ));
            }
            let profile = ConnectionProfile::new(name, Engine::SQLite, "", path);
            return Ok(ParsedConnectionUrl {
                profile,
                password: None,
            });
        }

        let url = url::Url::parse(raw).map_err(|error| {
            UniDbError::new(
                "url_invalid",
                ErrorCategory::Syntax,
                "The connection URL could not be parsed",
            )
            .with_detail(error.to_string())
        })?;
        let engine = match url.scheme() {
            "postgresql" | "postgres" => Engine::PostgreSQL,
            "cockroachdb" => Engine::CockroachDB,
            "mysql" => Engine::MySQL,
            "mariadb" => Engine::MariaDB,
            "redis" => Engine::Redis,
            "mongodb" | "mongodb+srv" => Engine::MongoDB,
            "mssql" | "sqlserver" => Engine::MSSQL,
            "sqlite" => Engine::SQLite,
            other => {
                return Err(UniDbError::new(
                    "url_scheme_unsupported",
                    ErrorCategory::Unsupported,
                    format!("The scheme {other} is not a supported database engine"),
                ));
            }
        };

        let host = url.host_str().unwrap_or_default().to_string();
        let port = url.port().or_else(|| engine.default_port()).unwrap_or(0);
        let database = url.path().trim_start_matches('/').to_string();
        let mut profile = ConnectionProfile::new(name, engine, host, database);
        profile.port = port;
        profile.username = url.username().to_string();
        let password = url.password().map(str::to_string);
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "sslmode" | "ssl_mode" => {
                    profile.ssl_mode = ssl_mode_from_value(&value)?;
                }
                "readonly" | "read_only" => {
                    profile.read_only = matches!(value.as_ref(), "1" | "true" | "yes");
                }
                _ => {}
            }
        }
        Ok(ParsedConnectionUrl { profile, password })
    }
}

fn ssl_mode_from_value(value: &str) -> Result<SslMode, UniDbError> {
    match value.to_ascii_lowercase().as_str() {
        "disable" | "disabled" | "false" => Ok(SslMode::Disable),
        "prefer" => Ok(SslMode::Prefer),
        "require" | "required" | "true" => Ok(SslMode::Require),
        "verify-ca" | "verify_ca" => Ok(SslMode::VerifyCa),
        "verify-full" | "verify_full" => Ok(SslMode::VerifyFull),
        other => Err(UniDbError::new(
            "url_ssl_mode_unsupported",
            ErrorCategory::Unsupported,
            format!("The TLS mode {other} is not supported"),
        )),
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

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn parses_a_postgres_url_with_tls_and_credentials() {
        let parsed = ConnectionProfile::from_url(
            "Cloud",
            "postgresql://ada:secret@db.example.com:5433/app?sslmode=verify-full&readonly=true",
        )
        .expect("valid url");
        assert_eq!(parsed.profile.engine, Engine::PostgreSQL);
        assert_eq!(parsed.profile.host, "db.example.com");
        assert_eq!(parsed.profile.port, 5433);
        assert_eq!(parsed.profile.database, "app");
        assert_eq!(parsed.profile.username, "ada");
        assert_eq!(parsed.profile.ssl_mode, SslMode::VerifyFull);
        assert!(parsed.profile.read_only);
        assert_eq!(parsed.password.as_deref(), Some("secret"));
    }

    #[test]
    fn every_network_scheme_maps_to_an_engine() {
        let cases = [
            ("postgres://h/db", Engine::PostgreSQL),
            ("cockroachdb://h/db", Engine::CockroachDB),
            ("mysql://h/db", Engine::MySQL),
            ("mariadb://h/db", Engine::MariaDB),
            ("redis://h/0", Engine::Redis),
            ("mongodb://h/db", Engine::MongoDB),
            ("mssql://h/db", Engine::MSSQL),
            ("sqlserver://h/db", Engine::MSSQL),
        ];
        for (url, expected) in cases {
            let parsed = ConnectionProfile::from_url("Test", url).expect("valid url");
            assert_eq!(parsed.profile.engine, expected, "{url}");
        }
    }

    #[test]
    fn parses_sqlite_file_urls() {
        let absolute = ConnectionProfile::from_url("File", "sqlite:///home/ada/app.db")
            .expect("absolute sqlite url");
        assert_eq!(absolute.profile.engine, Engine::SQLite);
        assert_eq!(absolute.profile.database, "/home/ada/app.db");

        let relative =
            ConnectionProfile::from_url("File", "sqlite:app.db").expect("relative sqlite url");
        assert_eq!(relative.profile.database, "app.db");
    }

    #[test]
    fn rejects_unknown_schemes_and_tls_modes() {
        let scheme =
            ConnectionProfile::from_url("Bad", "ftp://host/db").expect_err("unknown scheme");
        assert_eq!(scheme.code(), "url_scheme_unsupported");

        let tls = ConnectionProfile::from_url("Bad", "postgres://h/db?sslmode=bogus")
            .expect_err("unknown tls mode");
        assert_eq!(tls.code(), "url_ssl_mode_unsupported");
    }

    #[test]
    fn defaults_the_port_from_the_engine() {
        let parsed =
            ConnectionProfile::from_url("Local", "postgres://localhost/app").expect("valid url");
        assert_eq!(parsed.profile.port, 5432);
        assert_eq!(parsed.profile.ssl_mode, SslMode::Disable);
    }
}
