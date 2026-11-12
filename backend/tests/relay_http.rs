//! Hermetic HTTP tests for the relay's upstream fetch.
//!
//! Every test spins up a throwaway `axum` server (or a hand-written one-shot
//! HTTP/1.1 socket when the wire bytes must be exact) on `127.0.0.1:0`. No test
//! touches the network, and none depends on the wall clock beyond the 1 s
//! timeout case.
//!
//! # Why the network cases call `fetch_ics` directly
//!
//! Wave 1 ships `guard::validate_target` as a stub that refuses every target
//! with `ForbiddenHost`, so `GET /ics` cannot reach the fetch yet. The
//! fetch-level cases below build the `ValidatedTarget` the guard will hand over
//! (URL + allowlisted suffix + the addresses it resolved) and drive the real
//! `fetch_ics`. Once `agent/backend-guard` lands, these five must be re-run
//! through `app(state)` and assert the response headers instead of the bytes:
//!
//! * `happy_path_returns_the_upstream_bytes_and_the_contract_etag` → 200,
//!   `content-type: text/calendar; charset=utf-8`, `ETag`,
//!   `cache-control: private, max-age=300`, `X-Content-Type-Options: nosniff`.
//! * a replay of the returned `ETag` in `If-None-Match` → 304 with an empty
//!   body (the 304 branch is pure router logic and already unit-tested in
//!   `routes`, but it has no fetch-level counterpart here).
//! * `a_redirect_…` / `an_upstream_5xx_…` → 502 `upstream_error`.
//! * `a_declared_length_…` / `a_chunked_body_…` → 413 `upstream_too_large`.
//! * `a_slow_upstream_is_a_504` → 504 `timeout`.
//!
//! The router-level cases at the bottom (`400`, `403`, `405`) work today
//! because they never reach the guard's resolution step, and they stay valid
//! after the guard lands.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use creno_relay::config::Config;
use creno_relay::error::RelayError;
use creno_relay::guard::ValidatedTarget;
use creno_relay::upstream::fetch_ics;
use creno_relay::{AppState, app};
use http_body_util::BodyExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tower::ServiceExt;
use url::Url;

mod common;

/// Valid calendar served by the happy-path upstream.
const CALENDAR: &[u8] = include_bytes!("fixtures/relay-calendar.ics");
/// `sha256sum tests/fixtures/relay-calendar.ics`, i.e. the hex half of the
/// contract's `ETag: "sha256-<hex>"`.
const CALENDAR_SHA256: &str =
    "sha256-231cf997442883c25a979105c94b5622acdaf6b617b8d619a63c69ca9921edae";

/// Allowlisted suffix reported in the debug trace (never the URL itself).
const ALLOWED_SUFFIX: &str = "test.invalid";

/// Runtime configuration used by the fetch-level tests.
fn config(max_bytes: usize, timeout_secs: u64) -> Config {
    Config {
        bind: "127.0.0.1:0".parse().expect("literal socket address"),
        // Only the router-level `403` case consults this list; the guard stub
        // refuses everything regardless, and the fetch tests bypass the guard.
        allowed_hosts: vec!["localhost".to_string()],
        max_bytes,
        timeout: Duration::from_secs(timeout_secs),
        rate_limit_per_minute: 10_000,
        rate_burst: 10_000,
        allow_extension: false,
        extra_origins: Vec::new(),
    }
}

/// A local upstream and the number of requests it actually answered.
struct Upstream {
    name: &'static str,
    addr: SocketAddr,
    hits: Arc<AtomicUsize>,
}

impl Upstream {
    /// The target the guard would have produced: the URL, the allowlisted
    /// suffix, and exactly one validated address (loopback).
    fn target(&self, path: &str) -> ValidatedTarget {
        ValidatedTarget {
            url: Url::parse(&format!("http://{}:{}{path}", self.name, self.addr.port()))
                .expect("test URL"),
            host: ALLOWED_SUFFIX.to_string(),
            ips: vec![IpAddr::from([127, 0, 0, 1])],
        }
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

/// Starts a local upstream whose fake host name is never resolvable.
async fn start_upstream(name: &'static str) -> Upstream {
    let hits = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route("/calendar.ics", get(calendar))
        .route("/slow.ics", get(slow))
        .route("/boom.ics", get(boom))
        .route("/redirect.ics", get(redirect))
        .with_state(hits.clone());

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind test upstream");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Upstream { name, addr, hits }
}

async fn calendar(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/calendar; charset=utf-8")],
        CALENDAR.to_vec(),
    )
        .into_response()
}

async fn slow(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_secs(3)).await;
    (StatusCode::OK, CALENDAR.to_vec()).into_response()
}

async fn boom(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    (StatusCode::INTERNAL_SERVER_ERROR, "upstream exploded").into_response()
}

async fn redirect(State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    (StatusCode::FOUND, [(header::LOCATION, "/calendar.ics")]).into_response()
}

/// Reads an HTTP/1.1 request head (up to the blank line) and drops it.
async fn read_request_head(stream: &mut TcpStream) {
    let mut seen = Vec::new();
    let mut buf = [0_u8; 512];
    while !seen.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(read) => seen.extend_from_slice(&buf[..read]),
        }
    }
}

/// One-shot upstream that writes `head` verbatim and then keeps the socket
/// open: the client, not the server, must decide when the read is over.
async fn spawn_raw_upstream(head: &'static str) -> SocketAddr {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind raw upstream");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        read_request_head(&mut stream).await;
        if stream.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        let _ = stream.flush().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        let _ = stream.shutdown().await;
    });
    addr
}

/// Upstream that streams a chunked body forever, 8 KiB per chunk.
///
/// Only an implementation that stops reading at the cap can answer quickly: a
/// buffering one would either hang until its timeout or exhaust memory.
async fn spawn_endless_chunked_upstream() -> SocketAddr {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind chunked upstream");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        read_request_head(&mut stream).await;
        let head = "HTTP/1.1 200 OK\r\n\
                    Content-Type: text/calendar\r\n\
                    Transfer-Encoding: chunked\r\n\r\n";
        if stream.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        let data = vec![b'x'; 8192];
        let header = format!("{:x}\r\n", data.len());
        for _ in 0..4096 {
            if stream.write_all(header.as_bytes()).await.is_err()
                || stream.write_all(&data).await.is_err()
                || stream.write_all(b"\r\n").await.is_err()
            {
                return;
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n").await;
    });
    addr
}

fn raw_target(name: &'static str, addr: SocketAddr) -> ValidatedTarget {
    ValidatedTarget {
        url: Url::parse(&format!("http://{name}:{}/raw.ics", addr.port())).expect("test URL"),
        host: ALLOWED_SUFFIX.to_string(),
        ips: vec![IpAddr::from([127, 0, 0, 1])],
    }
}

// ---------------------------------------------------------------------------
// Fetch level
// ---------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_returns_the_upstream_bytes_and_the_contract_etag() {
    let _logs = logs();
    let upstream = start_upstream("relay-happy.invalid").await;
    let body = fetch_ics(&upstream.target("/calendar.ics"), &config(2_097_152, 5))
        .await
        .expect("the local upstream answers 200");

    assert_eq!(body, CALENDAR, "upstream body must be returned verbatim");
    assert!(body.starts_with(b"BEGIN:VCALENDAR"));
    assert_eq!(upstream.hits(), 1);

    // The router derives the ETag from exactly these bytes.
    assert_eq!(
        creno_relay::routes::etag_for(&body),
        format!("\"{CALENDAR_SHA256}\"")
    );
}

#[tokio::test]
async fn dns_pinning_uses_the_validated_addresses_not_the_system_resolver() {
    let _logs = logs();
    // `.invalid` is guaranteed never to resolve (RFC 6761), so this 200 can
    // only be produced by the pinned loopback address handed over by the guard.
    let upstream = start_upstream("relay-pinned.invalid").await;
    let body = fetch_ics(&upstream.target("/calendar.ics"), &config(2_097_152, 5))
        .await
        .expect("the pinned address answers");
    assert_eq!(body, CALENDAR);
    assert_eq!(upstream.hits(), 1);
}

#[tokio::test]
async fn a_redirect_is_a_502_and_the_upstream_is_hit_exactly_once() {
    let _logs = logs();
    let upstream = start_upstream("relay-redirect.invalid").await;
    let error = fetch_ics(&upstream.target("/redirect.ics"), &config(2_097_152, 5))
        .await
        .expect_err("a 302 must not be followed");

    let hits = upstream.hits();
    println!("redirect case: upstream requests observed = {hits} (expected 1)");
    assert_eq!(error, RelayError::UpstreamError);
    assert_eq!(
        hits, 1,
        "the Location target must never be requested: redirects are disabled"
    );
}

#[tokio::test]
async fn an_upstream_5xx_is_a_502() {
    let _logs = logs();
    let upstream = start_upstream("relay-boom.invalid").await;
    let error = fetch_ics(&upstream.target("/boom.ics"), &config(2_097_152, 5))
        .await
        .expect_err("a 500 is not a calendar");
    assert_eq!(error, RelayError::UpstreamError);
    assert_eq!(upstream.hits(), 1);
}

#[tokio::test]
async fn a_declared_length_above_the_cap_is_refused_before_reading() {
    let _logs = logs();
    // 10 MiB announced and no body at all: only the header can explain the
    // 413, so a length-first check is the only way to pass.
    let addr = spawn_raw_upstream(
        "HTTP/1.1 200 OK\r\nContent-Type: text/calendar\r\nContent-Length: 10485760\r\n\r\n",
    )
    .await;

    let error = fetch_ics(
        &raw_target("relay-declared.invalid", addr),
        &config(1024, 5),
    )
    .await
    .expect_err("the announced body is above CRENO_MAX_BYTES");
    assert_eq!(error, RelayError::UpstreamTooLarge);
}

#[tokio::test]
async fn a_chunked_body_over_the_cap_aborts_mid_stream() {
    let _logs = logs();
    // 2 MiB+ with no `Content-Length` and no end in sight: the read must stop
    // the moment the cap is crossed, not at the end of the body.
    let addr = spawn_endless_chunked_upstream().await;
    let started = Instant::now();
    let error = fetch_ics(
        &raw_target("relay-chunked.invalid", addr),
        &config(2 * 1024 * 1024, 5),
    )
    .await
    .expect_err("the streamed body crosses CRENO_MAX_BYTES");

    assert_eq!(error, RelayError::UpstreamTooLarge);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the fetch must abort at the cap instead of waiting for the upstream: took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn a_slow_upstream_is_a_504() {
    let _logs = logs();
    let upstream = start_upstream("relay-slow.invalid").await;
    let error = fetch_ics(&upstream.target("/slow.ics"), &config(2_097_152, 1))
        .await
        .expect_err("the upstream sleeps three seconds");
    assert_eq!(error, RelayError::Timeout);
}

// ---------------------------------------------------------------------------
// Log hygiene
// ---------------------------------------------------------------------------

/// Installs the shared sink at `DEBUG` on first use.
///
/// It has to be installed before the first fetch: `tracing` caches callsite
/// interest process-wide, so a subscriber added later needs a rebuild.
fn logs() -> &'static common::Sink {
    common::logs(tracing::Level::DEBUG)
}

#[tokio::test]
async fn the_debug_trace_names_the_allowlisted_host_but_never_the_url() {
    const CANARY: &str = "canary-6f1a2f83-1c2d-4a5e";

    let sink = logs();
    let upstream = start_upstream("relay-logs.invalid").await;
    let target = upstream.target(&format!("/calendar.ics?token={CANARY}"));
    let body = fetch_ics(&target, &config(2_097_152, 5))
        .await
        .expect("the local upstream answers 200");
    assert_eq!(body, CALENDAR);

    let logs = sink.contents();
    assert!(logs.contains("host=test.invalid"), "missing host: {logs}");
    assert!(logs.contains("status=200"), "missing status: {logs}");
    assert!(
        logs.contains(&format!("bytes={}", CALENDAR.len())),
        "missing byte count: {logs}"
    );
    assert!(!logs.contains(CANARY), "token leaked into logs: {logs}");
    assert!(!logs.contains("token"), "query string leaked: {logs}");
    assert!(!logs.contains("calendar.ics"), "path leaked: {logs}");
}

// ---------------------------------------------------------------------------
// Router level (reachable today)
// ---------------------------------------------------------------------------

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: String,
}

async fn call(request: Request<Body>) -> Reply {
    let response = app(AppState::new(config(2_097_152, 5)))
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

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds")
}

#[tokio::test]
async fn a_get_without_a_url_is_a_400() {
    let reply = call(get_request("/ics")).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body, r#"{"error":"missing_url"}"#);
}

#[tokio::test]
async fn a_post_is_a_405_json_with_allow() {
    let request = Request::builder()
        .method("POST")
        .uri("/ics")
        .body(Body::empty())
        .expect("request builds");
    let reply: Reply = call(request).await;
    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(reply.body, r#"{"error":"method_not_allowed"}"#);
    assert_eq!(
        reply
            .headers
            .get(header::ALLOW)
            .and_then(|value| value.to_str().ok()),
        Some("GET, OPTIONS")
    );
}

#[tokio::test]
async fn a_host_outside_the_allowlist_is_a_403() {
    // Valid before and after the guard lands: `evil.example.org` is not in
    // `CRENO_ALLOWED_HOSTS` (which is `localhost` here).
    let encoded: String =
        url::form_urlencoded::byte_serialize(b"https://evil.example.org/calendar.ics").collect();
    let reply = call(get_request(&format!("/ics?url={encoded}"))).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.body, r#"{"error":"forbidden_host"}"#);
    assert_eq!(
        reply
            .headers
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
}
