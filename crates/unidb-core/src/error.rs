//! One error type shared by the engine, store, and AI layers.
//!
//! Safety rule from spec 0001: SQL text and secrets never appear in `Display`,
//! in log fields, or in the crash export. `detail` is expected to be redacted
//! by the caller before it is stored here.

use std::error::Error;
use std::fmt;

/// Coarse failure family. Drives retry policy and UI messaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Auth,
    Network,
    Tls,
    Timeout,
    Syntax,
    Permission,
    Unsupported,
    ConnectionLost,
    Cancelled,
    Internal,
}

impl ErrorCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Network => "network",
            Self::Tls => "tls",
            Self::Timeout => "timeout",
            Self::Syntax => "syntax",
            Self::Permission => "permission",
            Self::Unsupported => "unsupported",
            Self::ConnectionLost => "connection_lost",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }
}

/// A stable, safe error. `user_message` is shown to the user; `detail` is for
/// local diagnostics and must not contain SQL text or secrets.
#[derive(Debug)]
pub struct UniDbError {
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    user_message: String,
    detail: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl UniDbError {
    pub fn new(
        code: &'static str,
        category: ErrorCategory,
        user_message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            category,
            retryable: false,
            user_message: user_message.into(),
            detail: String::new(),
            source: None,
        }
    }

    pub fn internal(user_message: impl Into<String>) -> Self {
        Self::new("internal", ErrorCategory::Internal, user_message)
    }

    #[must_use]
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    #[must_use]
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn category(&self) -> ErrorCategory {
        self.category
    }

    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    pub fn user_message(&self) -> &str {
        &self.user_message
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for UniDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.user_message)
    }
}

impl Error for UniDbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_shows_only_the_safe_message() {
        let error = UniDbError::new("auth_failed", ErrorCategory::Auth, "Sign in failed")
            .with_detail("password=hunter2")
            .retryable(false);
        assert_eq!(error.to_string(), "Sign in failed");
        assert!(!error.to_string().contains("hunter2"));
        assert_eq!(error.code(), "auth_failed");
        assert_eq!(error.category(), ErrorCategory::Auth);
        assert!(!error.is_retryable());
    }

    #[test]
    fn source_chain_is_preserved() {
        let io = std::io::Error::other("disk full");
        let error = UniDbError::internal("Could not write the file").with_source(io);
        assert!(Error::source(&error).is_some());
    }
}
