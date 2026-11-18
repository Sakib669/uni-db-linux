//! Signed shareable team bundles: sanitized connection profiles and saved queries.
//! Never includes passwords, SSH paths, or AI keys.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::Engine;
use crate::error::{ErrorCategory, UniDbError};
use crate::profile::{ConnectionProfile, SslMode};

pub const BUNDLE_FORMAT_VERSION: u32 = 1;

/// Sanitized connection metadata safe for sharing in a team bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleConnection {
    pub name: String,
    pub engine: Engine,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub ssl_mode: SslMode,
    pub read_only: bool,
}

impl BundleConnection {
    pub fn from_profile(profile: &ConnectionProfile) -> Self {
        Self {
            name: profile.name.clone(),
            engine: profile.engine,
            host: profile.host.clone(),
            port: profile.port,
            database: profile.database.clone(),
            username: profile.username.clone(),
            ssl_mode: profile.ssl_mode,
            read_only: profile.read_only,
        }
    }

    pub fn to_profile(&self) -> ConnectionProfile {
        let mut profile =
            ConnectionProfile::new(&self.name, self.engine, &self.host, &self.database);
        profile.port = self.port;
        profile.username = self.username.clone();
        profile.ssl_mode = self.ssl_mode;
        profile.read_only = self.read_only;
        profile
    }
}

/// Saved SQL snippet exported in a team bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleQuery {
    pub name: String,
    pub sql: String,
}

#[derive(Serialize)]
struct CanonicalPayload<'a> {
    version: u32,
    bundle_name: &'a str,
    created_by: &'a str,
    created_at: &'a str,
    connections: &'a [BundleConnection],
    saved_queries: &'a [BundleQuery],
}

/// Signed team bundle document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamBundle {
    pub version: u32,
    pub bundle_name: String,
    pub created_by: String,
    pub created_at: String,
    pub connections: Vec<BundleConnection>,
    pub saved_queries: Vec<BundleQuery>,
    pub signature_sha256: String,
}

/// Verification outcome produced before importing a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleVerification {
    pub signature_valid: bool,
    pub detached_sig_checked: bool,
    pub fingerprint: String,
    pub message: String,
}

impl TeamBundle {
    /// Builds and signs a new team bundle from sanitized profiles and queries.
    pub fn build(
        bundle_name: impl Into<String>,
        created_by: impl Into<String>,
        profiles: &[ConnectionProfile],
        saved_queries: Vec<BundleQuery>,
    ) -> Result<Self, UniDbError> {
        let bundle_name = bundle_name.into().trim().to_string();
        if bundle_name.is_empty() {
            return Err(UniDbError::new(
                "bundle_name_empty",
                ErrorCategory::Syntax,
                "Give the team bundle a name before exporting",
            ));
        }
        let created_by = created_by.into().trim().to_string();
        let created_by = if created_by.is_empty() {
            "UniDB".to_string()
        } else {
            created_by
        };
        let connections: Vec<BundleConnection> = profiles
            .iter()
            .map(BundleConnection::from_profile)
            .collect();
        let mut bundle = Self {
            version: BUNDLE_FORMAT_VERSION,
            bundle_name,
            created_by,
            created_at: Utc::now().to_rfc3339(),
            connections,
            saved_queries,
            signature_sha256: String::new(),
        };
        bundle.signature_sha256 = bundle.compute_payload_sha256()?;
        Ok(bundle)
    }

    pub fn compute_payload_sha256(&self) -> Result<String, UniDbError> {
        let canonical = CanonicalPayload {
            version: self.version,
            bundle_name: &self.bundle_name,
            created_by: &self.created_by,
            created_at: &self.created_at,
            connections: &self.connections,
            saved_queries: &self.saved_queries,
        };
        let bytes = serde_json::to_vec(&canonical).map_err(|error| {
            UniDbError::internal("Could not serialize team bundle payload").with_source(error)
        })?;
        let digest = Sha256::digest(&bytes);
        Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    /// Verifies the embedded payload signature and optional detached signature text.
    pub fn verify(&self, detached_sig: Option<&str>) -> BundleVerification {
        let expected = match self.compute_payload_sha256() {
            Ok(hash) => hash,
            Err(_) => {
                return BundleVerification {
                    signature_valid: false,
                    detached_sig_checked: false,
                    fingerprint: String::new(),
                    message: "Could not hash bundle payload".to_string(),
                };
            }
        };
        let short_fp = expected.chars().take(12).collect::<String>();
        if self.version != BUNDLE_FORMAT_VERSION {
            return BundleVerification {
                signature_valid: false,
                detached_sig_checked: false,
                fingerprint: short_fp,
                message: format!("Unsupported bundle version {}", self.version),
            };
        }
        if self.signature_sha256 != expected {
            return BundleVerification {
                signature_valid: false,
                detached_sig_checked: false,
                fingerprint: short_fp,
                message: "Embedded SHA-256 signature does not match payload".to_string(),
            };
        }
        if let Some(detached_raw) = detached_sig {
            let trimmed = detached_raw.trim();
            let sig_value = trimmed
                .strip_prefix("unidb-bundle-sha256")
                .map(str::trim)
                .unwrap_or(trimmed);
            if sig_value != expected {
                return BundleVerification {
                    signature_valid: false,
                    detached_sig_checked: true,
                    fingerprint: short_fp,
                    message: "Detached .sig file does not match payload".to_string(),
                };
            }
            return BundleVerification {
                signature_valid: true,
                detached_sig_checked: true,
                fingerprint: short_fp,
                message: "Detached and embedded SHA-256 signatures verified".to_string(),
            };
        }
        BundleVerification {
            signature_valid: true,
            detached_sig_checked: false,
            fingerprint: short_fp,
            message: "Embedded SHA-256 signature verified (no detached .sig file)".to_string(),
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, UniDbError> {
        serde_json::to_string_pretty(self).map_err(|error| {
            UniDbError::internal("Could not format team bundle JSON").with_source(error)
        })
    }

    pub fn detached_signature_content(&self) -> String {
        format!("unidb-bundle-sha256 {}\n", self.signature_sha256)
    }
}

/// Parses raw bundle JSON, rejects disallowed secret fields, and verifies signatures.
pub fn inspect_bundle_json(
    json_text: &str,
    detached_sig: Option<&str>,
) -> Result<(TeamBundle, BundleVerification), UniDbError> {
    let raw_value: serde_json::Value = serde_json::from_str(json_text).map_err(|error| {
        UniDbError::new(
            "bundle_invalid_json",
            ErrorCategory::Syntax,
            "The team bundle is not valid JSON",
        )
        .with_detail(error.to_string())
    })?;
    if contains_secret_field(&raw_value) {
        return Err(UniDbError::new(
            "bundle_contains_secrets",
            ErrorCategory::Permission,
            "Team bundles must never contain password, SSH key, or API token fields",
        ));
    }
    let bundle: TeamBundle = serde_json::from_value(raw_value).map_err(|error| {
        UniDbError::new(
            "bundle_invalid_schema",
            ErrorCategory::Syntax,
            "The team bundle structure is invalid",
        )
        .with_detail(error.to_string())
    })?;
    let verification = bundle.verify(detached_sig);
    Ok((bundle, verification))
}

fn contains_secret_field(value: &serde_json::Value) -> bool {
    const DISALLOWED: [&str; 6] = [
        "password", "ssh_key", "ssh_path", "ai_key", "api_key", "secret",
    ];
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, child)| {
            let lower = key.to_ascii_lowercase();
            DISALLOWED.iter().any(|bad| lower == *bad) || contains_secret_field(child)
        }),
        serde_json::Value::Array(items) => items.iter().any(contains_secret_field),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_signs_verifies_and_strips_secrets() {
        let profile =
            ConnectionProfile::new("Staging", Engine::PostgreSQL, "db.example.com", "app");
        let bundle = TeamBundle::build(
            "Core Team",
            "Ada",
            &[profile],
            vec![BundleQuery {
                name: "Active users".to_string(),
                sql: "SELECT count(*) FROM users;".to_string(),
            }],
        )
        .expect("build bundle");
        let json = bundle.to_json_pretty().expect("json");
        let detached = bundle.detached_signature_content();
        assert!(!json.contains("password"));

        let (parsed, check) = inspect_bundle_json(&json, Some(&detached)).expect("inspect");
        assert!(check.signature_valid);
        assert!(check.detached_sig_checked);
        assert_eq!(parsed.connections.len(), 1);
        assert_eq!(parsed.saved_queries.len(), 1);
    }

    #[test]
    fn tampered_bundle_and_secret_fields_are_detected() {
        let profile = ConnectionProfile::new("Local", Engine::SQLite, "", "/tmp/demo.db");
        let bundle = TeamBundle::build("Team", "Grace", &[profile], vec![]).expect("build");
        let json = bundle.to_json_pretty().expect("json");
        let tampered = json.replace("/tmp/demo.db", "/tmp/evil.db");
        let (_, check) = inspect_bundle_json(&tampered, None).expect("parse");
        assert!(!check.signature_valid);

        let with_secret = r#"{"version":1,"bundle_name":"X","created_by":"Y","created_at":"2026-01-01","connections":[{"name":"A","engine":"sqlite","host":"","port":0,"database":"/tmp/a.db","username":"","ssl_mode":"disable","read_only":false,"password":"leak"}],"saved_queries":[],"signature_sha256":"abc"}"#;
        let err = inspect_bundle_json(with_secret, None).expect_err("reject secrets");
        assert_eq!(err.code(), "bundle_contains_secrets");
    }
}
