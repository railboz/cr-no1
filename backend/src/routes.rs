//! HTTP routes of the relay.
//!
//! * `GET /healthz` — liveness probe, `{"status":"ok"}`.
//! * `GET /ics?url=…` — fetch an allowlisted calendar and return it verbatim,
//!   with a strong `ETag` (sha256 of the body) so the PWA can revalidate.
//! * `OPTIONS` — answered by [`crate::cors`] before routing.
//!
//! Errors are always [`crate::error::RelayError`], i.e. `{"error":"<code>"}`.

use axum::Router;
use axum::body::Body;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::error::RelayError;
use crate::guard;
use crate::upstream;

/// Hard limit on the `url` query parameter, in bytes.
pub const MAX_URL_LEN: usize = 2048;
/// Content type of a successful ICS response.
pub const ICS_CONTENT_TYPE: &str = "text/calendar; charset=utf-8";
/// Cache directive of a successful ICS response.
pub const ICS_CACHE_CONTROL: &str = "private, max-age=300";

/// Builds the application routes (layers are applied in [`crate::app`]).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/ics", get(ics))
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
}

/// `GET /healthz` → `200 {"status":"ok"}`.
async fn healthz() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        )],
        Body::from(r#"{"status":"ok"}"#),
    )
        .into_response()
}

/// `GET /ics?url=…`
async fn ics(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    RawQuery(raw_query): RawQuery,
) -> Result<Response, RelayError> {
    let url = extract_url(raw_query.as_deref())?;
    let target = guard::validate_target(&url, &state.cfg).await?;
    let body = upstream::fetch_ics(&target, &state.cfg).await?;

    let etag = etag_for(&body);
    let if_none_match = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());

    if matches_etag(if_none_match, &etag) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        let parts = response.headers_mut();
        parts.insert(header::ETAG, etag_header(&etag));
        parts.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(ICS_CACHE_CONTROL),
        );
        parts.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        return Ok(response);
    }

    let mut response = (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(ICS_CONTENT_TYPE),
        )],
        Body::from(body),
    )
        .into_response();
    let parts = response.headers_mut();
    parts.insert(header::ETAG, etag_header(&etag));
    parts.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(ICS_CACHE_CONTROL),
    );
    parts.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// Reads the single `url` parameter out of the raw query string.
///
/// Parsed by hand (never through a catch-all extractor) so every malformed
/// input maps onto a documented code: `missing_url` for absent/empty and
/// `invalid_url` for an over-long value.
fn extract_url(raw_query: Option<&str>) -> Result<String, RelayError> {
    let Some(raw_query) = raw_query else {
        return Err(RelayError::MissingUrl);
    };
    let Some(value) = url::form_urlencoded::parse(raw_query.as_bytes())
        .find(|(key, _)| key == "url")
        .map(|(_, value)| value.into_owned())
    else {
        return Err(RelayError::MissingUrl);
    };
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(RelayError::MissingUrl);
    }
    if value.len() > MAX_URL_LEN {
        return Err(RelayError::InvalidUrl);
    }
    Ok(value)
}

/// `"sha256-<hex>"` of an upstream body (without the HTTP quoting).
pub fn etag_for(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        // Writing into a `String` cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    format!("\"sha256-{hex}\"")
}

/// Turns a generated ETag into a header value. The input is hex and quotes
/// only, so the fallback is unreachable; it exists to avoid `unwrap`.
fn etag_header(etag: &str) -> HeaderValue {
    HeaderValue::from_str(etag).unwrap_or_else(|_| HeaderValue::from_static("\"sha256\""))
}

/// Weak comparison against a possibly comma-separated `If-None-Match` value.
pub fn matches_etag(if_none_match: Option<&str>, etag: &str) -> bool {
    let Some(raw) = if_none_match else {
        return false;
    };
    raw.split(',').any(|candidate| {
        let candidate = candidate.trim();
        let candidate = candidate.strip_prefix("W/").unwrap_or(candidate);
        candidate == etag
    })
}

/// 405 for a known path with an unsupported method.
async fn method_not_allowed(method: Method) -> Response {
    if method == Method::OPTIONS {
        return options_response();
    }
    RelayError::MethodNotAllowed.into_response()
}

/// Unknown path: 404, and a bare 204 for a preflight on an unknown path so a
/// browser gets a clean CORS answer instead of a routing error.
async fn not_found(method: Method) -> Response {
    if method == Method::OPTIONS {
        return options_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

fn options_response() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Origin"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::time::Duration;
    use tower::ServiceExt;

    fn config() -> Config {
        Config {
            bind: "127.0.0.1:8080".parse().expect("literal"),
            allowed_hosts: vec!["api.skello.io".to_string()],
            max_bytes: 4096,
            timeout: Duration::from_secs(1),
            rate_limit_per_minute: 1000,
            rate_burst: 1000,
            allow_extension: false,
            extra_origins: vec!["https://app.example.fr".to_string()],
        }
    }

    fn state() -> AppState {
        AppState::new(config())
    }

    struct Reply {
        status: StatusCode,
        headers: axum::http::HeaderMap,
        body: String,
    }

    async fn call(request: Request<Body>) -> Reply {
        let response = crate::app(state())
            .oneshot(request)
            .await
            .expect("infallible");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body collects")
            .to_bytes();
        Reply {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    fn get(uri: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("request builds")
    }

    fn header(reply: &Reply, name: axum::http::header::HeaderName) -> Option<String> {
        reply
            .headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    #[tokio::test]
    async fn healthz_returns_exactly_ok() {
        let reply = call(get("/healthz")).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.body, r#"{"status":"ok"}"#);
        assert_eq!(
            header(&reply, header::CONTENT_TYPE).as_deref(),
            Some("application/json; charset=utf-8")
        );
    }

    #[tokio::test]
    async fn ics_without_url_is_missing_url() {
        let reply = call(get("/ics")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"missing_url"}"#);
        assert_eq!(
            header(&reply, header::CONTENT_TYPE).as_deref(),
            Some("application/json; charset=utf-8")
        );
    }

    #[tokio::test]
    async fn ics_with_empty_or_foreign_param_is_missing_url() {
        for uri in ["/ics?url=", "/ics?noturl=x", "/ics?URL=x"] {
            let reply = call(get(uri)).await;
            assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{uri}");
            assert_eq!(reply.body, r#"{"error":"missing_url"}"#, "{uri}");
        }
    }

    #[tokio::test]
    async fn ics_with_overlong_url_is_invalid_url() {
        let target = format!("https://api.skello.io/ics?token={}", "a".repeat(2100));
        let encoded: String = url::form_urlencoded::byte_serialize(target.as_bytes()).collect();
        let reply = call(get(&format!("/ics?url={encoded}"))).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"invalid_url"}"#);
    }

    #[tokio::test]
    async fn ics_with_valid_length_url_reaches_the_guard() {
        // A well-formed URL of a legal length still goes through the guard. The
        // host is outside the allowlist, so the guard refuses it before any
        // resolution: deterministic 403, no DNS, no upstream call.
        let encoded: String =
            url::form_urlencoded::byte_serialize(b"https://evil.example/x").collect();
        let reply = call(get(&format!("/ics?url={encoded}"))).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN);
        assert_eq!(reply.body, r#"{"error":"forbidden_host"}"#);
        assert_eq!(
            header(&reply, header::X_CONTENT_TYPE_OPTIONS).as_deref(),
            Some("nosniff")
        );

        // Same request shape above the 2048-byte limit: still a parameter shape
        // error, so `invalid_url` and never a host refusal.
        let target = format!("https://api.skello.io/ics?token={}", "a".repeat(2100));
        let encoded: String = url::form_urlencoded::byte_serialize(target.as_bytes()).collect();
        let reply = call(get(&format!("/ics?url={encoded}"))).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"invalid_url"}"#);
    }

    #[tokio::test]
    async fn unsupported_methods_are_405_json() {
        for uri in ["/ics", "/healthz"] {
            let request = Request::builder()
                .method(Method::POST)
                .uri(uri)
                .body(Body::empty())
                .expect("request builds");
            let reply = call(request).await;
            assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED, "{uri}");
            assert_eq!(reply.body, r#"{"error":"method_not_allowed"}"#, "{uri}");
            assert_eq!(
                header(&reply, header::ALLOW).as_deref(),
                Some("GET, OPTIONS"),
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        let reply = call(get("/nope")).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert!(reply.body.is_empty());
    }

    #[tokio::test]
    async fn preflight_answers_204_with_a_null_origin() {
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/ics")
            .header(header::ORIGIN, "null")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .expect("request builds");
        let reply = call(request).await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        assert!(reply.body.is_empty());
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).as_deref(),
            Some("null")
        );
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_ALLOW_METHODS).as_deref(),
            Some("GET, OPTIONS")
        );
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_ALLOW_HEADERS).as_deref(),
            Some("If-None-Match")
        );
    }

    #[tokio::test]
    async fn preflight_on_unknown_path_still_answers_204() {
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/whatever")
            .header(header::ORIGIN, "null")
            .body(Body::empty())
            .expect("request builds");
        let reply = call(request).await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn cors_headers_are_echoed_for_allowed_origins_only() {
        let request = Request::builder()
            .uri("/healthz")
            .header(header::ORIGIN, "http://localhost:5173")
            .body(Body::empty())
            .expect("request builds");
        let reply = call(request).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).as_deref(),
            Some("http://localhost:5173")
        );
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_EXPOSE_HEADERS).as_deref(),
            Some("ETag")
        );
        assert!(
            header(&reply, header::VARY)
                .unwrap_or_default()
                .contains("Origin")
        );

        let request = Request::builder()
            .uri("/healthz")
            .header(header::ORIGIN, "https://evil.example.com")
            .body(Body::empty())
            .expect("request builds");
        let reply = call(request).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert!(header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert!(header(&reply, header::ACCESS_CONTROL_EXPOSE_HEADERS).is_none());

        // `null` (file://) is allowed, and the answer is never `*`.
        let request = Request::builder()
            .uri("/healthz")
            .header(header::ORIGIN, "null")
            .body(Body::empty())
            .expect("request builds");
        let reply = call(request).await;
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).as_deref(),
            Some("null")
        );
    }

    #[tokio::test]
    async fn extension_origin_follows_the_flag() {
        let request = || {
            Request::builder()
                .uri("/healthz")
                .header(header::ORIGIN, "moz-extension://6f1a2f83-1c2d-4a5e")
                .body(Body::empty())
                .expect("request builds")
        };
        let reply = call(request()).await;
        assert!(header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

        let mut cfg = config();
        cfg.allow_extension = true;
        let response = crate::app(AppState::new(cfg))
            .oneshot(request())
            .await
            .expect("infallible");
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|value| value.to_str().ok()),
            Some("moz-extension://6f1a2f83-1c2d-4a5e")
        );
    }

    #[test]
    fn etag_is_stable_and_strong() {
        let first = etag_for(b"BEGIN:VCALENDAR");
        let second = etag_for(b"BEGIN:VCALENDAR");
        let other = etag_for(b"BEGIN:VCALENDAR\n");
        assert_eq!(first, second);
        assert_ne!(first, other);
        assert!(first.starts_with("\"sha256-"));
        assert!(first.ends_with('"'));
        // 1 (opening quote) + 7 ("sha256-") + 64 hex + 1 (closing quote).
        assert_eq!(first.len(), 73);
        // Known vector: sha256("") is e3b0c442…
        assert_eq!(
            etag_for(b""),
            "\"sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\""
        );
    }

    #[test]
    fn if_none_match_comparison() {
        let etag = etag_for(b"calendar");
        assert!(matches_etag(Some(&etag), &etag));
        assert!(matches_etag(Some(&format!("W/{etag}")), &etag));
        assert!(matches_etag(Some(&format!("\"other\", {etag}")), &etag));
        assert!(!matches_etag(Some("\"other\""), &etag));
        assert!(!matches_etag(None, &etag));
        assert!(!matches_etag(Some(""), &etag));
    }

    #[tokio::test]
    async fn rate_limit_returns_429_with_retry_after() {
        let mut config = config();
        config.rate_limit_per_minute = 1;
        config.rate_burst = 1;

        let app = crate::app(AppState::new(config));
        let first = app
            .clone()
            .oneshot(get("/healthz"))
            .await
            .expect("infallible");
        assert_eq!(first.status(), StatusCode::OK);

        let second = app.oneshot(get("/healthz")).await.expect("infallible");
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            second
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|seconds| seconds >= 1)
        );
        let bytes = second
            .into_body()
            .collect()
            .await
            .expect("body collects")
            .to_bytes();
        assert_eq!(
            String::from_utf8_lossy(&bytes),
            r#"{"error":"rate_limited"}"#
        );
    }
}
