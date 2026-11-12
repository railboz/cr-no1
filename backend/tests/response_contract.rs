//! HTTP response contract of `GET /ics`, at the seam the handler uses.
//!
//! [`CONTRACT.md`] promises exact headers for `200` and `304`, and an upstream
//! fetch that carries only two fixed request headers. Both were only asserted
//! indirectly (the ETag unit test, the upstream fetch tests), so this file
//! pins them byte for byte.
//!
//! # Why the responses are built through [`calendar_response`]
//!
//! The full `GET /ics` path cannot be driven hermetically: [`guard`] accepts
//! `https` on port `443` only and refuses every non-global address, so a local
//! plain-HTTP upstream on `127.0.0.1` is — by design — unreachable through the
//! handler, and bypassing the guard in a test would defeat the point. The
//! handler therefore delegates *all* of its response construction to one pure
//! function, and these tests exercise that function. The only thing left to the
//! handler is parsing the query string and forwarding `If-None-Match`, which the
//! existing `routes` unit tests cover.
//!
//! [`CONTRACT.md`]: ../CONTRACT.md
//! [`guard`]: creno_relay::guard
//! [`calendar_response`]: creno_relay::routes::calendar_response

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Request as AxumRequest, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use creno_relay::config::Config;
use creno_relay::guard::ValidatedTarget;
use creno_relay::routes::{
    ICS_CACHE_CONTROL, ICS_CONTENT_TYPE, calendar_response, etag_for, matches_etag,
};
use creno_relay::upstream::{ACCEPT, USER_AGENT, fetch_ics};
use http_body_util::BodyExt;
use tokio::net::TcpListener;
use url::Url;

/// A calendar body used as the "upstream" answer.
const BODY: &[u8] = b"BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Creno//contract test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:contract-test@example.invalid\r\n\
DTSTAMP:20260101T000000Z\r\n\
SUMMARY:Creneau\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

/// `sha256sum` of exactly the bytes above, i.e. the hex half of the contract's
/// `ETag: "sha256-<hex>"`. Hardcoded on purpose: the test must not recompute
/// the value with the very function it is checking.
const BODY_SHA256: &str = "82f61aed21005f71d2900d4a565c26a402d22f851d5b08a9177d6cb59997808d";

/// The full ETag the contract mandates for [`BODY`].
fn body_etag() -> String {
    format!("\"sha256-{BODY_SHA256}\"")
}

/// A response, split into the three things the contract talks about.
struct Received {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Received {
    async fn of(response: Response) -> Self {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("the test body collects")
            .to_bytes()
            .to_vec();
        Self {
            status,
            headers,
            body,
        }
    }

    fn header(&self, name: header::HeaderName) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }
}

// ---------------------------------------------------------------------------
// `200`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_200_carries_exactly_the_contract_headers_and_the_verbatim_body() {
    let received = Received::of(calendar_response(BODY.to_vec(), None)).await;

    assert_eq!(received.status, StatusCode::OK);
    assert_eq!(
        received.header(header::CONTENT_TYPE).as_deref(),
        Some(ICS_CONTENT_TYPE),
        "content-type must be the contract literal"
    );
    assert_eq!(
        received.header(header::ETAG),
        Some(body_etag()),
        "etag must be \"sha256-<hex of the body>\""
    );
    assert_eq!(
        received.header(header::CACHE_CONTROL).as_deref(),
        Some(ICS_CACHE_CONTROL),
        "cache-control must be the contract literal"
    );
    assert_eq!(
        received.header(header::X_CONTENT_TYPE_OPTIONS).as_deref(),
        Some("nosniff")
    );
    assert_eq!(received.body, BODY, "the body must be returned verbatim");

    // Sanity check of the fixture against the crate's own implementation: the
    // hardcoded hex above and `etag_for` must agree, otherwise one of them is
    // stale.
    assert_eq!(etag_for(BODY), body_etag());
}

// ---------------------------------------------------------------------------
// `304`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_matching_if_none_match_is_a_304_without_a_body() {
    let etag = body_etag();
    let received = Received::of(calendar_response(BODY.to_vec(), Some(&etag))).await;

    assert_eq!(received.status, StatusCode::NOT_MODIFIED);
    assert!(
        received.body.is_empty(),
        "a 304 must not carry a body, got {} bytes",
        received.body.len()
    );
    assert_eq!(
        received.header(header::ETAG),
        Some(etag),
        "the etag must survive the 304"
    );
    assert_eq!(
        received.header(header::CACHE_CONTROL).as_deref(),
        Some(ICS_CACHE_CONTROL)
    );
    assert_eq!(
        received.header(header::X_CONTENT_TYPE_OPTIONS).as_deref(),
        Some("nosniff")
    );
    // The contract lists three headers for a 304, not four: no media type on a
    // body that does not exist.
    assert_eq!(received.header(header::CONTENT_TYPE), None);
}

#[tokio::test]
async fn a_foreign_if_none_match_is_a_200_again() {
    let received = Received::of(calendar_response(
        BODY.to_vec(),
        Some("\"sha256-0000000000000000000000000000000000000000000000000000000000000000\""),
    ))
    .await;

    assert_eq!(received.status, StatusCode::OK);
    assert_eq!(received.body, BODY);
    assert_eq!(received.header(header::ETAG), Some(body_etag()));
}

// ---------------------------------------------------------------------------
// ETag matching: what the relay actually does
// ---------------------------------------------------------------------------
//
// `routes::matches_etag` implements **weak comparison**, which is what RFC 9110
// §13.1.2 mandates for `If-None-Match` on `GET`: the `W/` prefix is ignored, the
// value may be a comma-separated list, and surrounding whitespace is trimmed.
// The bare `*` is the RFC 9110 §13.1.1 wildcard ("represent the current
// representation"), so it always revalidates to a `304`; the quoted `"*"` is an
// ordinary opaque tag and does not.
// Two spellings are *not* normalised, and the tests below pin that too:
//
// * a lowercase `w/` prefix is not the RFC 9110 weak marker (it is
//   case-sensitive), so it does not match;
// * an unquoted opaque tag is not a valid entity-tag, so it does not match.

#[tokio::test]
async fn weak_and_multi_value_if_none_match_revalidate() {
    let etag = body_etag();
    let accepted = [
        // RFC 9110 §13.1.1: `*` matches any current representation, so a
        // cached copy is always revalidated into a `304`.
        "*".to_string(),
        "*,".to_string(),
        "\"sha256-other\", *".to_string(),
        etag.clone(),
        format!("W/{etag}"),
        format!("{etag}, \"sha256-other\""),
        format!("\"sha256-other\", {etag}"),
        format!("  {etag}  "),
        format!(" W/{etag} "),
    ];

    for candidate in accepted {
        let received = Received::of(calendar_response(BODY.to_vec(), Some(&candidate))).await;
        assert_eq!(
            received.status,
            StatusCode::NOT_MODIFIED,
            "If-None-Match: {candidate:?} should revalidate"
        );
        assert!(matches_etag(Some(&candidate), &etag), "{candidate:?}");
    }
}

#[tokio::test]
async fn malformed_or_unrelated_if_none_match_values_do_not_revalidate() {
    let etag = body_etag();
    let refused = [
        // Not the RFC 9110 weak marker: the prefix is case-sensitive.
        format!("w/{etag}"),
        // Not an entity-tag without its quotes.
        etag.trim_matches('"').to_string(),
        // A tag for different bytes.
        format!("\"sha256-{}\"", "0".repeat(64)),
        // Absence and emptiness.
        String::new(),
        // A quoted star is an opaque tag, not the wildcard.
        "\"*\"".to_string(),
    ];

    for candidate in refused {
        let received = Received::of(calendar_response(BODY.to_vec(), Some(&candidate))).await;
        assert_eq!(
            received.status,
            StatusCode::OK,
            "If-None-Match: {candidate:?} must not revalidate"
        );
        assert_eq!(received.body, BODY);
    }

    let received = Received::of(calendar_response(BODY.to_vec(), None)).await;
    assert_eq!(received.status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Outbound request headers
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<HeaderMap>>>);

impl Seen {
    fn headers(&self) -> HeaderMap {
        let guard = self.0.lock().expect("seen lock");
        assert_eq!(guard.len(), 1, "exactly one upstream request expected");
        guard[0].clone()
    }
}

async fn capture(State(seen): State<Seen>, request: AxumRequest) -> Response {
    seen.0
        .lock()
        .expect("seen lock")
        .push(request.headers().clone());
    (StatusCode::OK, BODY.to_vec()).into_response()
}

/// Starts a local upstream that records the headers of the request it receives.
async fn start_capturing_upstream() -> (SocketAddr, Seen) {
    let seen = Seen::default();
    let router = Router::new()
        .route("/calendar.ics", get(capture))
        .with_state(seen.clone());
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind capture upstream");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (addr, seen)
}

#[tokio::test]
async fn the_upstream_request_carries_only_the_two_fixed_headers() {
    let (addr, seen) = start_capturing_upstream().await;

    // Same shape as `tests/relay_http.rs`: the name is unresolvable (RFC 6761
    // `.invalid`), the only address is the pinned loopback, so this 200 proves
    // both the pinning and the header set. The query string carries what a
    // caller would like to smuggle upstream; only the URL may carry it, never a
    // header.
    let target = ValidatedTarget {
        url: Url::parse(&format!(
            "http://relay-headers.invalid:{}/calendar.ics?token=canary-6f1a2f83",
            addr.port()
        ))
        .expect("test URL"),
        host: "test.invalid".to_string(),
        ips: vec![IpAddr::from([127, 0, 0, 1])],
    };
    let body = fetch_ics(&target, &Config::default())
        .await
        .expect("the pinned local upstream answers 200");
    assert_eq!(body, BODY);

    let headers = seen.headers();
    let mut names: Vec<String> = headers
        .keys()
        .map(|name| name.as_str().to_ascii_lowercase())
        .collect();
    names.sort();
    println!("upstream request headers: {names:?}");

    // The security property, stated as an equality: nothing the caller or the
    // target could carry is forwarded. `host` is protocol-framing (from the URL,
    // not from the caller); `user-agent` and `accept` are the two fixed values.
    assert_eq!(
        names,
        vec!["accept", "host", "user-agent"],
        "fetch_ics must send exactly its fixed headers"
    );
    assert_eq!(
        headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some(USER_AGENT)
    );
    assert_eq!(
        headers
            .get(header::ACCEPT)
            .and_then(|value| value.to_str().ok()),
        Some(ACCEPT)
    );
    // Explicitly: no credential, no forwarding, no content negotiation from the
    // caller can appear, because there is no code path that copies them.
    for forbidden in [
        "authorization",
        "cookie",
        "x-forwarded-for",
        "x-real-ip",
        "referer",
        "origin",
        "content-type",
    ] {
        assert!(
            headers.get(forbidden).is_none(),
            "{forbidden} must never be sent upstream"
        );
    }
}
