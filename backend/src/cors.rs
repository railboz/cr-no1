//! CORS allowlist.
//!
//! The relay is called from three kinds of origins only:
//! * `null` — the PWA opened from `file://`,
//! * `moz-extension://<uuid>` — the browser-extension build, opt-in,
//! * `http://localhost:<port>` / `http://127.0.0.1:<port>` — local dev,
//! * plus exact origins explicitly listed in `CRENO_ALLOWED_ORIGINS`.
//!
//! `*` is never sent: the relay answers with the caller's own origin, and only
//! when that origin is allowlisted.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::config::Config;

/// Methods advertised on preflight.
pub const ALLOW_METHODS: &str = "GET, OPTIONS";
/// Request headers advertised on preflight.
pub const ALLOW_HEADERS: &str = "If-None-Match";
/// Response headers exposed to scripts.
pub const EXPOSE_HEADERS: &str = "ETag";

/// Origin granted to `file://` documents.
pub const NULL_ORIGIN: &str = "null";
const EXTENSION_SCHEME: &str = "moz-extension://";

/// Returns `true` when `origin` may receive CORS headers.
///
/// Matching is deliberately strict: exact equality for `null` and for the
/// configured extra origins, one of the two loopback hosts for dev, and a
/// `moz-extension://` scheme prefix only when explicitly enabled.
pub fn origin_allowed(origin: &str, cfg: &Config) -> bool {
    let origin = origin.trim();
    if origin.is_empty() || origin.len() > 512 {
        return false;
    }
    if origin == NULL_ORIGIN {
        return true;
    }

    let lower = origin.to_ascii_lowercase();

    if cfg.allow_extension
        && let Some(rest) = lower.strip_prefix(EXTENSION_SCHEME)
    {
        // Any UUID; an empty host is not an extension origin.
        return !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_');
    }

    if let Some(authority) = lower.strip_prefix("http://") {
        if let Some(port) = authority.strip_prefix("localhost") {
            return is_optional_port(port);
        }
        if let Some(port) = authority.strip_prefix("127.0.0.1") {
            return is_optional_port(port);
        }
    }

    cfg.extra_origins
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(origin))
}

/// Accepts `""` (implicit port 80) or `":<1-65535>"` and nothing else.
fn is_optional_port(rest: &str) -> bool {
    if rest.is_empty() {
        return true;
    }
    let Some(port) = rest.strip_prefix(':') else {
        return false;
    };
    if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    matches!(port.parse::<u16>(), Ok(value) if value >= 1)
}

fn insert_str(headers: &mut HeaderMap, name: header::HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}

fn insert_static(headers: &mut HeaderMap, name: header::HeaderName, value: &'static str) {
    headers.insert(name, HeaderValue::from_static(value));
}

/// Headers shared by preflight and actual responses.
fn apply_cors_headers(headers: &mut HeaderMap, origin: &str) {
    insert_str(headers, header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    insert_static(headers, header::ACCESS_CONTROL_ALLOW_METHODS, ALLOW_METHODS);
    insert_static(headers, header::ACCESS_CONTROL_ALLOW_HEADERS, ALLOW_HEADERS);
}

/// Axum middleware: answers preflight itself and decorates real responses.
pub async fn cors_middleware(
    State(cfg): State<Arc<Config>>,
    request: Request,
    next: Next,
) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let allowed = origin
        .as_deref()
        .filter(|value| origin_allowed(value, &cfg))
        .map(str::to_owned);

    if request.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        if let Some(origin) = allowed.as_deref() {
            apply_cors_headers(response.headers_mut(), origin);
        }
        // `Vary` is always present so no shared cache can mix two origins.
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static("Origin"));
        return response;
    }

    let mut response = next.run(request).await;
    if let Some(origin) = allowed.as_deref() {
        let headers = response.headers_mut();
        apply_cors_headers(headers, origin);
        insert_static(
            headers,
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            EXPOSE_HEADERS,
        );
    }
    response
        .headers_mut()
        .append(header::VARY, HeaderValue::from_static("Origin"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::time::Duration;

    fn cfg() -> Config {
        Config {
            bind: "127.0.0.1:8080".parse().expect("literal"),
            allowed_hosts: vec!["api.skello.io".to_string()],
            max_bytes: 4096,
            timeout: Duration::from_secs(1),
            rate_limit_per_minute: 30,
            rate_burst: 10,
            allow_extension: false,
            extra_origins: Vec::new(),
        }
    }

    #[test]
    fn file_origin_is_allowed() {
        assert!(origin_allowed("null", &cfg()));
        assert!(!origin_allowed("Null", &cfg()));
        assert!(!origin_allowed("", &cfg()));
    }

    #[test]
    fn loopback_origins_are_allowed() {
        let cfg = cfg();
        for origin in [
            "http://localhost:5173",
            "http://localhost:8080",
            "http://LOCALHOST:3000",
            "http://127.0.0.1:4173",
            "http://localhost",
            "http://127.0.0.1",
        ] {
            assert!(origin_allowed(origin, &cfg), "{origin} should be allowed");
        }
        for origin in [
            "http://localhost:0",
            "http://localhost:99999999",
            "http://localhost:abc",
            "http://localhost:80/path",
            "https://localhost:8443",
            "http://127.0.0.1.evil.com:8080",
            "http://localhost.evil.com",
            "http://192.168.1.10:8080",
        ] {
            assert!(!origin_allowed(origin, &cfg), "{origin} should be refused");
        }
    }

    #[test]
    fn extension_origin_is_opt_in() {
        let mut cfg = cfg();
        assert!(!origin_allowed(
            "moz-extension://6f1a2f83-1c2d-4a5e-9b1f-2d3c4b5a6e7f",
            &cfg
        ));
        cfg.allow_extension = true;
        assert!(origin_allowed(
            "moz-extension://6f1a2f83-1c2d-4a5e-9b1f-2d3c4b5a6e7f",
            &cfg
        ));
        assert!(!origin_allowed("moz-extension://", &cfg));
        assert!(!origin_allowed("moz-extension://a/b", &cfg));
    }

    #[test]
    fn extra_origins_are_exact_and_case_insensitive() {
        let mut cfg = cfg();
        cfg.extra_origins = vec!["https://app.example.fr".to_string()];
        assert!(origin_allowed("https://app.example.fr", &cfg));
        assert!(origin_allowed("https://APP.example.fr", &cfg));
        assert!(!origin_allowed("https://evil.example.fr", &cfg));
        assert!(!origin_allowed("https://app.example.fr.evil.com", &cfg));
        assert!(!origin_allowed("https://app.example.fr:8443", &cfg));
    }

    #[test]
    fn nothing_matches_the_wildcard() {
        let cfg = cfg();
        assert!(!origin_allowed("*", &cfg));
        assert!(!origin_allowed("https://random.example.com", &cfg));
    }

    #[test]
    fn headers_never_contain_a_wildcard() {
        let mut headers = HeaderMap::new();
        apply_cors_headers(&mut headers, "null");
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|value| value.to_str().ok()),
            Some("null")
        );
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .and_then(|value| value.to_str().ok()),
            Some("GET, OPTIONS")
        );
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .and_then(|value| value.to_str().ok()),
            Some("If-None-Match")
        );
    }
}
