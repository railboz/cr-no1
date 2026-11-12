//! End-to-end proof that the request log cannot leak the target URL.
//!
//! The relay's query string carries the private Skello ICS token, so no log line
//! may ever contain it. This test installs a real `tracing` subscriber, sends a
//! request whose query string holds a canary token, and inspects everything the
//! relay printed.

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use creno_relay::config::Config;
use creno_relay::{AppState, app};
use tower::ServiceExt;

const CANARY: &str = "canary-6f1a2f83-1c2d-4a5e";

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    fn contents(&self) -> String {
        let guard = self.0.lock().expect("sink lock");
        String::from_utf8_lossy(&guard).into_owned()
    }
}

impl Write for Sink {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink lock").extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn request_log_never_contains_the_target_url() {
    let sink = Sink::default();
    // Process-wide subscriber: every thread of the test process logs into `sink`.
    tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .init();

    let uri = format!("/ics?url=https%3A%2F%2Fapi.skello.io%2Fics%2Fabc%3Ftoken%3D{CANARY}");
    let response = app(AppState::new(Config::default()))
        .oneshot(
            Request::builder()
                .uri(&uri)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("infallible");

    // Wave 1: the guard stub refuses every target; the point of this test is the
    // log line, not the status.
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

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
