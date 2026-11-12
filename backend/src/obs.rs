//! Observability: tracing setup and a request log that cannot leak secrets.
//!
//! The relay's security model forbids logging the target URL (it carries the
//! private Skello ICS token). This module therefore logs the *path component
//! only* — never the query string, never an absolute URL — and provides
//! [`redact_url`] for the rare places that must mention a URL at all.

use std::time::Instant;

use axum::extract::Request;
use axum::http::{Uri, header};
use axum::middleware::Next;
use axum::response::Response;
use tracing_subscriber::EnvFilter;
use url::Url;

/// Log target used by the request middleware and by upstream instrumentation.
pub const TARGET: &str = "creno_relay";

/// Installs the global tracing subscriber (`RUST_LOG`, default `info`).
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_ansi(false)
        .init();
}

/// Path component of a request URI.
///
/// This is the only part of the request line that may be logged: the query
/// string of `/ics?url=…` contains the private upstream token, and the
/// fragment never reaches the server anyway.
pub fn loggable_path(uri: &Uri) -> &str {
    uri.path()
}

/// Renders a URL for logs as `scheme://host[:port]/path`, dropping the query
/// string, the fragment and any userinfo. Unparsable input is fully redacted
/// so a token can never slip through on a malformed URL.
pub fn redact_url(raw: &str) -> String {
    let Ok(url) = Url::parse(raw.trim()) else {
        return "<redacted>".to_string();
    };
    if url.cannot_be_a_base() {
        return "<redacted>".to_string();
    }
    let host = url.host_str().unwrap_or("-");
    let port = match url.port() {
        Some(port) => format!(":{port}"),
        None => String::new(),
    };
    format!("{}://{host}{port}{}", url.scheme(), url.path())
}

/// Axum middleware logging method, path, status, duration and body size.
///
/// One line per request, no query string, no headers, no body.
pub async fn log_requests(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = loggable_path(request.uri()).to_string();
    let started = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16();
    let bytes = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    tracing::info!(
        target: TARGET,
        method = %method,
        path = %path,
        status,
        elapsed_ms,
        bytes,
        "request"
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "s3cr3t-token-value";

    #[test]
    fn loggable_path_is_the_only_loggable_part_of_a_request() {
        let uri: Uri = format!("/ics?url=https%3A%2F%2Fapi.skello.io%2Fics%3Ftoken%3D{SECRET}")
            .parse()
            .expect("valid uri");
        assert_eq!(loggable_path(&uri), "/ics");
        assert!(!loggable_path(&uri).contains(SECRET));
    }

    #[test]
    fn redact_url_hides_query_and_userinfo() {
        let redacted = redact_url(&format!("https://api.skello.io/ics/abcdef?token={SECRET}"));
        assert_eq!(redacted, "https://api.skello.io/ics/abcdef");
        assert!(!redacted.contains(SECRET));

        let redacted = redact_url("https://user:hunter2@api.skello.io:8443/ics?t=1#frag");
        assert_eq!(redacted, "https://api.skello.io:8443/ics");
        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("user"));

        assert_eq!(redact_url("not a url with token"), "<redacted>");
        assert_eq!(redact_url(""), "<redacted>");
        assert_eq!(
            redact_url("data:text/calendar,BEGIN:VCALENDAR"),
            "<redacted>"
        );
        assert_eq!(redact_url("mailto:someone@example.com"), "<redacted>");
    }
}
