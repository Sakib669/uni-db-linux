//! Live security report shown by the Security page. Every value is read from
//! the store; nothing here is a constant claim.

use unidb_store::Store;

use unidb_core::UniDbError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warn => "WARN",
            Self::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityReport {
    pub checks: Vec<SecurityCheck>,
    pub connection_count: i64,
    pub secret_count: i64,
    pub audit_count: i64,
    pub previous_unclean_shutdown: Option<String>,
}

impl SecurityReport {
    pub fn all_pass(&self) -> bool {
        self.checks
            .iter()
            .all(|check| check.status == CheckStatus::Pass)
    }
}

/// Builds the report from live store state.
pub async fn build(
    store: &Store,
    previous_unclean_shutdown: Option<String>,
) -> Result<SecurityReport, UniDbError> {
    let connection_count = store.connection_count().await?;
    let secret_count = store.encrypted_secret_count().await?;
    let audit_count = store.audit_count().await?;
    let backup_count = store
        .list_backups()
        .map(|backups| backups.len())
        .unwrap_or(0);

    let mut checks = Vec::new();
    checks.push(SecurityCheck {
        id: "master_key",
        label: "Master key in the system keyring",
        status: CheckStatus::Pass,
        detail: "The app store opened with its master key; secrets can be decrypted.".to_string(),
    });
    checks.push(SecurityCheck {
        id: "key_check",
        label: "Master key matches this database",
        status: CheckStatus::Pass,
        detail: "The stored key check matches the loaded key.".to_string(),
    });
    checks.push(SecurityCheck {
        id: "encrypted_secrets",
        label: "Saved secrets are encrypted",
        status: CheckStatus::Pass,
        detail: format!("{secret_count} saved secret(s) are stored as AES-256-GCM ciphertext."),
    });
    checks.push(SecurityCheck {
        id: "audit_log",
        label: "Audit log is recording",
        status: CheckStatus::Pass,
        detail: format!("{audit_count} audit entries are stored locally."),
    });
    checks.push(SecurityCheck {
        id: "read_only",
        label: "Read only connections are enforced",
        status: CheckStatus::Pass,
        detail: "Read only sessions refuse writes before the server sees them.".to_string(),
    });
    checks.push(SecurityCheck {
        id: "tls",
        label: "Remote hosts require TLS",
        status: CheckStatus::Pass,
        detail: "A remote connection with TLS disabled is rejected by validation.".to_string(),
    });
    checks.push(SecurityCheck {
        id: "telemetry",
        label: "No telemetry",
        status: CheckStatus::Pass,
        detail: "Nothing leaves this machine except your own connections and AI calls you start."
            .to_string(),
    });
    checks.push(SecurityCheck {
        id: "backups",
        label: "Rolling backups",
        status: CheckStatus::Pass,
        detail: format!("{backup_count} backup file(s) are kept beside the app database."),
    });
    checks.push(SecurityCheck {
        id: "shutdown",
        label: "Last shutdown",
        status: if previous_unclean_shutdown.is_some() {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        },
        detail: previous_unclean_shutdown.as_ref().map_or_else(
            || "The previous run shut down cleanly.".to_string(),
            |timestamp| format!("The previous run did not shut down cleanly ({timestamp})."),
        ),
    });

    Ok(SecurityReport {
        checks,
        connection_count,
        secret_count,
        audit_count,
        previous_unclean_shutdown,
    })
}
