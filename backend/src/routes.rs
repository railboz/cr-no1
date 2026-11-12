//! HTTP routes of the relay.
//!
//! * `GET /healthz` — liveness probe, `{"status":"ok"}`.
//! * `GET /ics?url=…` — fetch an allowlisted calendar and return it verbatim,
//!   with a strong `ETag` (sha256 of the body) so the PWA can revalidate.
//! * `OPTIONS` — answered by [`crate::cors`] before routing.
//! * anything else — `404 {"error":"not_found"}`, like every other error.
//!
//! Errors are always [`crate::error::RelayError`], i.e. `{"error":"<code>"}`,
//! `Content-Type: application/json; charset=utf-8`: there is no bodiless
//! response and no `text/plain` fallback anywhere in the router.

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
    // A pure parameter check first: a malformed request must never be answered
    // "busy" because the relay happens to be saturated. From here on the
    // request does I/O (DNS, then TLS), so it takes one of the
    // [`crate::MAX_CONCURRENT_FETCHES`] slots — without waiting, so a burst is
    // shed instead of exhausting the task pool.
    let _permit = state
        .permits
        .clone()
        .try_acquire_owned()
        .map_err(|_| RelayError::Busy)?;
    let target = guard::validate_target(&url, &state.cfg).await?;
    let body = upstream::fetch_ics(&target, &state.cfg).await?;

    let if_none_match = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());

    Ok(calendar_response(body, if_none_match))
}

/// Builds the `/ics` response for an upstream body and an `If-None-Match` value.
///
/// This is the whole HTTP contract of a successful fetch (`CONTRACT.md`):
///
/// * `200` with the body verbatim, [`ICS_CONTENT_TYPE`], the strong ETag of
///   those bytes, [`ICS_CACHE_CONTROL`] and `X-Content-Type-Options: nosniff`.
/// * `304` with no body when the header revalidates the same bytes; the ETag
///   and the cache/nosniff headers are kept so the client can trust its copy.
///
/// Pure: no I/O, no clock, no allocation beyond the ETag. Public so the
/// contract can be tested from an integration test without an upstream.
///
/// The body is taken by value so the success path never copies it: the bytes
/// have already been buffered by [`crate::upstream::fetch_ics`].
///
/// ```
/// use creno_relay::routes::calendar_response;
///
/// let body = b"BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".to_vec();
/// let fresh = calendar_response(body.clone(), None);
/// assert_eq!(fresh.status(), 200);
///
/// // Replaying the ETag of those exact bytes revalidates the copy.
/// let etag = fresh.headers()["etag"].to_str().expect("ascii etag").to_string();
/// let replayed = calendar_response(body, Some(&etag));
/// assert_eq!(replayed.status(), 304);
/// ```
pub fn calendar_response(body: Vec<u8>, if_none_match: Option<&str>) -> Response {
    let etag = etag_for(&body);

    if matches_etag(if_none_match, &etag) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        calendar_headers(response.headers_mut(), &etag);
        return response;
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
    calendar_headers(response.headers_mut(), &etag);
    response
}

/// Headers carried by both a `200` and a `304`: the ETag, the cache directive
/// and `nosniff`, so a revalidated copy stays as trustworthy as a fresh one.
fn calendar_headers(headers: &mut axum::http::HeaderMap, etag: &str) {
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(ICS_CACHE_CONTROL),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::ETAG, etag_header(etag));
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
///
/// `*` matches any current representation (RFC 9110 §13.1.2): `/ics` only runs
/// when it can produce a representation, so `If-None-Match: *` means "I already
/// have this calendar" and revalidates with a `304`.
pub fn matches_etag(if_none_match: Option<&str>, etag: &str) -> bool {
    let Some(raw) = if_none_match else {
        return false;
    };
    raw.split(',').any(|candidate| {
        let candidate = candidate.trim();
        if candidate == "*" {
            return true;
        }
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

/// Unknown path: a JSON `404 {"error":"not_found"}`, and a bare 204 for a
/// preflight on an unknown path so a browser gets a clean CORS answer instead of
/// a routing error.
async fn not_found(method: Method) -> Response {
    if method == Method::OPTIONS {
        return options_response();
    }
    RelayError::NotFound.into_response()
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
            allow_file_origin: false,
            extra_origins: vec!["https://app.example.fr".to_string()],
        }
    }

    fn state() -> AppState {
        AppState::new(config())
    }

    /// State whose config lets a `file://` document (`Origin: null`) through.
    fn state_allow_file_origin() -> AppState {
        AppState::new(Config {
            allow_file_origin: true,
            ..config()
        })
    }

    async fn call_with(state: AppState, request: Request<Body>) -> Reply {
        let response = crate::app(state)
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
    async fn query_parsing_ambiguities_are_pinned() {
        // The contract spells these four out; each one is a way a caller could
        // try to smuggle a second target or a different decoding past the relay.

        // 1. A duplicated `url` keeps the **first** occurrence (and the first
        //    one is refused here, before any DNS: a hermetic assertion).
        let reply = call(get(
            "/ics?url=not+a+url&url=https%3A%2F%2Fapi.skello.io%2Fics",
        ))
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "first value wins");
        assert_eq!(reply.body, r#"{"error":"invalid_url"}"#);

        // 2. `+` is a space, not a plus: a lone `+` decodes to a blank value,
        //    which is trimmed away and reads as missing.
        let reply = call(get("/ics?url=+")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"missing_url"}"#);

        //    A literal plus therefore has to be sent as `%2B`, and reaches the
        //    guard as a URL that does not parse.
        let reply = call(get("/ics?url=%2B")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"invalid_url"}"#);

        // 3. The parameter name is case-sensitive: `URL` is not `url`.
        let reply = call(get("/ics?URL=https%3A%2F%2Fapi.skello.io%2Fics")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"missing_url"}"#);

        // 4. An empty `url=` is missing, not invalid.
        let reply = call(get("/ics?url=")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"missing_url"}"#);

        //    ...and so is a value made only of whitespace, which the trim would
        //    otherwise pass on to the guard as an empty URL.
        let reply = call(get("/ics?url=%20%20")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"missing_url"}"#);
    }

    #[tokio::test]
    async fn a_saturated_relay_is_a_503_busy_but_keeps_answering_healthz() {
        let state = state();
        // Hold every permit, exactly as sixteen in-flight `/ics` would.
        let held: Vec<_> = (0..crate::MAX_CONCURRENT_FETCHES)
            .map(|_| {
                state
                    .permits
                    .clone()
                    .try_acquire_owned()
                    .expect("free slot")
            })
            .collect();
        assert!(state.permits.clone().try_acquire_owned().is_err());

        // `/healthz` takes no permit: liveness survives saturation.
        let reply = call_with(state.clone(), get("/healthz")).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.body, r#"{"status":"ok"}"#);

        // A parameter error is still decided before the permit, so a malformed
        // request is never told "busy".
        let reply = call_with(state.clone(), get("/ics")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body, r#"{"error":"missing_url"}"#);

        // A real fetch has no slot left: shed, with a JSON body and Retry-After.
        let encoded: String =
            url::form_urlencoded::byte_serialize(b"https://api.skello.io/ics").collect();
        let reply = call_with(state.clone(), get(&format!("/ics?url={encoded}"))).await;
        assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reply.body, r#"{"error":"busy"}"#);
        assert_eq!(
            header(&reply, header::CONTENT_TYPE).as_deref(),
            Some("application/json; charset=utf-8")
        );
        assert_eq!(header(&reply, header::RETRY_AFTER).as_deref(), Some("1"));

        // Releasing one slot lets the request through again: it now reaches the
        // guard, which refuses a foreign host before any DNS.
        drop(held.into_iter().next());
        let encoded: String =
            url::form_urlencoded::byte_serialize(b"https://evil.example/ics").collect();
        let reply = call_with(state, get(&format!("/ics?url={encoded}"))).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN);
        assert_eq!(reply.body, r#"{"error":"forbidden_host"}"#);
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
    async fn unknown_path_is_a_json_404() {
        for uri in ["/nope", "/ics/extra", "/"] {
            let reply = call(get(uri)).await;
            assert_eq!(reply.status, StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(reply.body, r#"{"error":"not_found"}"#, "{uri}");
            assert_eq!(
                header(&reply, header::CONTENT_TYPE).as_deref(),
                Some("application/json; charset=utf-8"),
                "{uri}"
            );
            assert_eq!(
                header(&reply, header::X_CONTENT_TYPE_OPTIONS).as_deref(),
                Some("nosniff"),
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn a_known_path_still_wins_over_the_fallback() {
        // The 404 must come from routing, not from a swallowed handler: the two
        // real paths keep their own answers.
        assert_eq!(call(get("/healthz")).await.status, StatusCode::OK);
        assert_eq!(call(get("/ics")).await.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn preflight_answers_204_with_a_null_origin_when_enabled() {
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/ics")
            .header(header::ORIGIN, "null")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .expect("request builds");
        let reply = call_with(state_allow_file_origin(), request).await;
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
    async fn file_origin_is_refused_until_it_is_enabled() {
        let request = || {
            Request::builder()
                .uri("/healthz")
                .header(header::ORIGIN, "null")
                .body(Body::empty())
                .expect("request builds")
        };

        // Default (`CRENO_ALLOW_FILE_ORIGIN=0`): served, but no CORS grant, so
        // the browser refuses to hand the body to the `file://` page.
        let reply = call(request()).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert!(header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert!(header(&reply, header::ACCESS_CONTROL_EXPOSE_HEADERS).is_none());
        assert!(
            header(&reply, header::VARY)
                .unwrap_or_default()
                .contains("Origin")
        );

        // Opted in: the exact `null` origin is echoed, never `*`.
        let reply = call_with(state_allow_file_origin(), request()).await;
        assert_eq!(
            header(&reply, header::ACCESS_CONTROL_ALLOW_ORIGIN).as_deref(),
            Some("null")
        );
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

        // `null` (file://) needs the opt-in flag, and the answer is never `*`.
        let request = Request::builder()
            .uri("/healthz")
            .header(header::ORIGIN, "null")
            .body(Body::empty())
            .expect("request builds");
        let reply = call_with(state_allow_file_origin(), request).await;
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
        // RFC 9110 §13.1.2: `*` matches any current representation, and may
        // appear inside a list.
        assert!(matches_etag(Some("*"), &etag));
        assert!(matches_etag(Some(" * "), &etag));
        assert!(matches_etag(Some("*,"), &etag));
        assert!(matches_etag(Some("\"other\", *"), &etag));
        // A bare `*` is not a tag, so it never equals a tag literally.
        assert!(!matches_etag(Some("\"*\""), &etag));
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
