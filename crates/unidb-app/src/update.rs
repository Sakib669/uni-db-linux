//! Update notice: parse a GitHub style release payload, compare versions, and
//! fetch the feed over HTTP. The check never downloads or installs anything.

use unidb_core::{ErrorCategory, UniDbError};

/// One release from the update feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub url: Option<String>,
    pub published_at: Option<String>,
}

/// Parses a GitHub releases API payload (`tag_name`, `html_url`,
/// `published_at`) into a release, stripping a leading `v` from the tag.
pub fn parse_release(payload: &str) -> Result<ReleaseInfo, UniDbError> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|error| {
        UniDbError::new(
            "update_feed_invalid",
            ErrorCategory::Syntax,
            "The update feed is not valid JSON",
        )
        .with_detail(error.to_string())
    })?;
    let tag = value
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .ok_or_else(|| {
            UniDbError::new(
                "update_feed_invalid",
                ErrorCategory::Syntax,
                "The update feed has no tag_name",
            )
        })?;
    let version = tag.trim_start_matches('v').trim().to_string();
    if version.is_empty() {
        return Err(UniDbError::new(
            "update_feed_invalid",
            ErrorCategory::Syntax,
            "The update feed has an empty version",
        ));
    }
    Ok(ReleaseInfo {
        version,
        url: value
            .get("html_url")
            .and_then(|url| url.as_str())
            .map(str::to_string),
        published_at: value
            .get("published_at")
            .and_then(|date| date.as_str())
            .map(str::to_string),
    })
}

/// True when `candidate` is a newer dotted numeric version than `current`.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    compare_versions(candidate, current) == std::cmp::Ordering::Greater
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let parse = |value: &str| {
        value
            .split(['.', '-', '+'])
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<u64>>()
    };
    let mut left_parts = parse(left);
    let mut right_parts = parse(right);
    let width = left_parts.len().max(right_parts.len());
    left_parts.resize(width, 0);
    right_parts.resize(width, 0);
    left_parts.cmp(&right_parts)
}

/// Fetches the release feed and parses it. Runs the blocking HTTP call on a
/// blocking thread so the worker runtime stays responsive.
pub async fn fetch_latest(feed_url: &str) -> Result<ReleaseInfo, UniDbError> {
    let url = feed_url.to_string();
    let payload = tokio::task::spawn_blocking(move || {
        let mut response = ureq::get(&url).call().map_err(|error| {
            UniDbError::new(
                "update_feed_unreachable",
                ErrorCategory::Network,
                "The update feed could not be reached",
            )
            .with_detail(error.to_string())
        })?;
        response.body_mut().read_to_string().map_err(|error| {
            UniDbError::new(
                "update_feed_unreachable",
                ErrorCategory::Network,
                "The update feed could not be read",
            )
            .with_detail(error.to_string())
        })
    })
    .await
    .map_err(|error| {
        UniDbError::internal("The update check task stopped").with_detail(error.to_string())
    })??;
    parse_release(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_github_release_payload() {
        let release = parse_release(
            r#"{"tag_name":"v0.2.0","html_url":"https://example.test/r","published_at":"2026-12-01T00:00:00Z"}"#,
        )
        .expect("parsed");
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.url.as_deref(), Some("https://example.test/r"));
        assert!(parse_release("{}").is_err());
        assert!(parse_release("not json").is_err());
    }

    #[test]
    fn compares_dotted_versions() {
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.9", "0.1.10"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("2.0.0", "1.9.9"));
    }

    #[tokio::test]
    async fn fetches_from_a_local_http_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let body = r#"{"tag_name":"v0.9.0","html_url":"https://example.test/0.9.0"}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let release = fetch_latest(&format!("http://{address}/latest"))
            .await
            .expect("fetched");
        assert_eq!(release.version, "0.9.0");
    }
}
