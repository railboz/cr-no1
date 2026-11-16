//! Per-IP rate limiting: a token bucket per client address.
//!
//! `CRENO_RATE_LIMIT` requests per minute with a fixed burst of
//! [`crate::config::RATE_BURST`]. Client addresses come from the TCP peer only:
//! `X-Forwarded-For` is *not* trusted, because a spoofable header would turn the
//! limiter into a no-op. Behind a reverse proxy, the proxy must enforce its own
//! limit or the relay must bind to loopback.
//!
//! The liveness probe (`GET`/`HEAD /healthz`) is exempt: it spends no token, so
//! a Docker healthcheck sharing the client IP with browse traffic can never be
//! answered `429` and restart an otherwise healthy container. Every other path
//! and method is charged as usual.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{Method, Request as HttpRequest};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::RelayError;

/// Buckets kept at most; beyond that the oldest is evicted.
const MAX_TRACKED: usize = 8192;
/// Buckets untouched for this long are dropped when the table is pruned.
const IDLE_TTL: Duration = Duration::from_secs(600);

/// Liveness path that bypasses the limiter (see [`is_exempt`]).
pub(crate) const HEALTHZ_PATH: &str = "/healthz";

/// Whether `request` skips the limiter.
///
/// Only the liveness probe, and only with the methods its route serves: a
/// `POST /healthz` is charged (and then rejected by the router), so the
/// exemption cannot be used to flood an arbitrary method. Preflights never reach
/// this layer — CORS answers them first — and must not be exempted here.
fn is_exempt<B>(request: &HttpRequest<B>) -> bool {
    let method = request.method();
    (method == Method::GET || method == Method::HEAD) && request.uri().path() == HEALTHZ_PATH
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Token-bucket limiter keyed by client IP.
#[derive(Debug)]
pub struct RateLimiter {
    refill_per_sec: f64,
    capacity: f64,
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
}

impl RateLimiter {
    /// `per_minute` and `burst` are clamped to at least 1.
    pub fn new(per_minute: u32, burst: u32) -> Self {
        Self {
            refill_per_sec: f64::from(per_minute.max(1)) / 60.0,
            capacity: f64::from(burst.max(1)),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Charges one request to `ip`. `Err` carries the seconds to wait.
    pub fn check(&self, ip: IpAddr) -> Result<(), u64> {
        self.check_at(ip, Instant::now())
    }

    /// Deterministic variant used by the tests.
    pub fn check_at(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let mut buckets = match self.buckets.lock() {
            Ok(guard) => guard,
            // A panic while holding the lock must not turn the limiter off.
            Err(poisoned) => poisoned.into_inner(),
        };

        if buckets.len() >= MAX_TRACKED {
            buckets.retain(|_, bucket| now.saturating_duration_since(bucket.last) < IDLE_TTL);
            if buckets.len() >= MAX_TRACKED
                && let Some(oldest) = buckets
                    .iter()
                    .min_by_key(|(_, bucket)| bucket.last)
                    .map(|(ip, _)| *ip)
            {
                buckets.remove(&oldest);
            }
        }

        let bucket = buckets.entry(ip).or_insert(Bucket {
            tokens: self.capacity,
            last: now,
        });

        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        bucket.last = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            let missing = 1.0 - bucket.tokens;
            let seconds = (missing / self.refill_per_sec).ceil().max(1.0);
            // `f64 -> u64` saturates on absurd values instead of wrapping.
            Err(seconds as u64)
        }
    }
}

/// Axum middleware charging one token per request to the peer address.
///
/// The liveness probe is answered without touching the bucket (see
/// [`is_exempt`]), so a healthcheck never competes with browse traffic.
pub async fn middleware(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Response {
    if is_exempt(&request) {
        return next.run(request).await;
    }

    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |info| info.0.ip());

    match limiter.check(ip) {
        Ok(()) => next.run(request).await,
        Err(retry_after_secs) => RelayError::RateLimited { retry_after_secs }.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, last))
    }

    #[test]
    fn burst_is_allowed_then_throttled() {
        let limiter = RateLimiter::new(30, 10);
        let now = Instant::now();
        for index in 0..10 {
            assert!(limiter.check_at(ip(1), now).is_ok(), "request {index}");
        }
        let wait = limiter.check_at(ip(1), now).expect_err("11th is throttled");
        assert!(wait >= 1, "retry_after must be at least one second");
        assert!(wait <= 2, "30/min means about one token every two seconds");
    }

    #[test]
    fn refill_restores_tokens_over_time() {
        let limiter = RateLimiter::new(60, 10);
        let start = Instant::now();
        for _ in 0..10 {
            limiter.check_at(ip(2), start).expect("burst");
        }
        assert!(limiter.check_at(ip(2), start).is_err());
        // 60/min = 1 token per second.
        assert!(
            limiter
                .check_at(ip(2), start + Duration::from_secs(1))
                .is_ok()
        );
        assert!(
            limiter
                .check_at(ip(2), start + Duration::from_secs(1))
                .is_err()
        );
        // Long sleep saturates at the burst capacity, never above it.
        let later = start + Duration::from_secs(3600);
        for _ in 0..10 {
            limiter.check_at(ip(2), later).expect("burst after idle");
        }
        assert!(limiter.check_at(ip(2), later).is_err());
    }

    #[test]
    fn buckets_are_per_ip() {
        let limiter = RateLimiter::new(1, 1);
        let now = Instant::now();
        limiter.check_at(ip(3), now).expect("first ip");
        assert!(limiter.check_at(ip(3), now).is_err());
        limiter
            .check_at(ip(4), now)
            .expect("second ip is independent");
    }

    #[test]
    fn retry_after_matches_the_configured_rate() {
        let limiter = RateLimiter::new(6, 1);
        let now = Instant::now();
        limiter.check_at(ip(5), now).expect("burst");
        // 6/min = one token every 10 seconds.
        assert_eq!(limiter.check_at(ip(5), now), Err(10));
    }

    #[test]
    fn zero_configuration_is_clamped() {
        let limiter = RateLimiter::new(0, 0);
        let now = Instant::now();
        assert!(limiter.check_at(ip(6), now).is_ok());
        assert!(limiter.check_at(ip(6), now).is_err());
    }

    fn probe_request(method: Method, uri: &str) -> HttpRequest<()> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(())
            .expect("request builds")
    }

    #[test]
    fn only_the_liveness_probe_is_exempt() {
        for method in [Method::GET, Method::HEAD] {
            assert!(is_exempt(&probe_request(method.clone(), HEALTHZ_PATH)));
            // The exemption is path based: a query string changes nothing.
            assert!(is_exempt(&probe_request(
                method.clone(),
                "/healthz?probe=1"
            )));
            // Neither a near miss nor another route may slip through.
            assert!(!is_exempt(&probe_request(method.clone(), "/healthz/")));
            assert!(!is_exempt(&probe_request(method.clone(), "/ics")));
        }
        for method in [Method::POST, Method::OPTIONS, Method::PUT, Method::DELETE] {
            assert!(
                !is_exempt(&probe_request(method.clone(), HEALTHZ_PATH)),
                "{method} /healthz must stay charged"
            );
        }
    }
}
