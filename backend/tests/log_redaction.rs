//! End-to-end proof that the request log cannot leak the target URL, and that
//! it reports the real response size.
//!
//! The relay's query string carries the private Skello ICS token, so no log line
//! may ever contain it. This test installs a real `tracing` subscriber, sends a
//! request whose query string holds a canary token, and inspects everything the
//! relay printed.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use creno_relay::config::Config;
use creno_relay::{AppState, app};
use tower::ServiceExt;

mod common;

const CANARY: &str = "canary-6f1a2f83-1c2d-4a5e";

/// Installs the shared sink at `TRACE` on first use: the point is to catch any
/// log line at all, not just the levels the relay uses today.
fn logs() -> &'static common::Sink {
    common::logs(tracing::Level::TRACE)
}

async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
    let response = app(AppState::new(Config::default()))
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("infallible");
    let status = response.status();
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("body collects")
        .to_bytes()
        .to_vec();
    (status, bytes)
}

#[tokio::test]
async fn request_log_never_contains_the_target_url() {
    let sink = logs();

    let uri = format!("/ics?url=https%3A%2F%2Fevil.example%2Fics%3Ftoken%3D{CANARY}");
    let (status, _body) = get(&uri).await;

    // The host is outside the default allowlist, so the guard refuses it before
    // any resolution: deterministic 403, no DNS. The point of this test is the
    // log line, not the reason for the refusal.
    assert_eq!(status, StatusCode::FORBIDDEN);

    let logs = sink.contents();
    assert!(logs.contains("path=/ics"), "unexpected logs: {logs}");
    assert!(logs.contains("status=403"), "unexpected logs: {logs}");
    assert!(logs.contains("elapsed_ms="), "unexpected logs: {logs}");
    assert!(!logs.contains(CANARY), "token leaked into logs: {logs}");
    assert!(
        !logs.contains("token"),
        "query string leaked into logs: {logs}"
    );
    assert!(
        !logs.contains("skello"),
        "target host leaked into logs: {logs}"
    );
    assert!(
        !logs.contains("%3F"),
        "percent-encoded query leaked: {logs}"
    );
}

/// The log must report the size of the response it actually sent.
///
/// The bug this pins: reading the *request* `Content-Length` made every `GET`
/// log `bytes=0`, whether it answered with a body or not.
#[tokio::test]
async fn request_log_reports_the_real_response_size() {
    let sink = logs();

    const HEALTHZ: &[u8] = br#"{"status":"ok"}"#;
    let (status, body) = get("/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, HEALTHZ);

    let logs = sink.contents();
    let line = logs
        .lines()
        .find(|line| line.contains("path=/healthz") && line.contains("status=200"))
        .unwrap_or_else(|| panic!("no /healthz log line: {logs}"));
    println!("healthz log line: {line}");
    assert!(
        line.contains(&format!("bytes={}", HEALTHZ.len())),
        "the log must carry the {} body bytes: {line}",
        HEALTHZ.len()
    );
    assert!(!line.contains("bytes=0"), "bytes must not be 0: {line}");

    // A body-less answer still logs 0, and still leaks nothing.
    let (status, body) = get("/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.is_empty());
    let logs = sink.contents();
    let line = logs
        .lines()
        .find(|line| line.contains("path=/nope") && line.contains("status=404"))
        .unwrap_or_else(|| panic!("no /nope log line: {logs}"));
    println!("404 log line: {line}");
    assert!(line.contains("bytes=0"), "empty body logs 0 bytes: {line}");
}
