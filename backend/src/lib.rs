//! Créno relay — optional, self-hosted replacement for public CORS relays.
//!
//! The PWA stores a private Skello ICS URL (`https://api.skello.io/…`). When it
//! is opened from `file://` or from a browser extension, a direct `fetch` is
//! blocked by CORS; the app then fell back to third-party relays, which see the
//! token. This crate is the self-hosted answer: it validates the target host,
//! fetches the calendar server-side, and returns it with CORS headers and an
//! `ETag`, so revalidation is cheap and the token never reaches a third party.
//!
//! Nothing here logs the target URL (see [`obs`]).

#![forbid(unsafe_code)]

pub mod config;
pub mod cors;
pub mod error;
pub mod guard;
pub mod obs;
pub mod ratelimit;
pub mod routes;
pub mod upstream;

use std::sync::Arc;

use axum::Router;
use axum::middleware;

use crate::config::Config;
use crate::ratelimit::RateLimiter;

/// Shared state handed to every handler.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Validated configuration, read-only for the whole process lifetime.
    pub cfg: Arc<Config>,
    /// Per-IP token bucket.
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    /// Builds the runtime state from a validated configuration.
    pub fn new(cfg: Config) -> Self {
        let limiter = RateLimiter::new(cfg.rate_limit_per_minute, cfg.rate_burst);
        Self {
            cfg: Arc::new(cfg),
            limiter: Arc::new(limiter),
        }
    }
}

/// Assembles the full router: routes plus request log, CORS and rate limiting.
///
/// Layer order, outermost first: request log → CORS → rate limit → routes. CORS
/// therefore answers preflight before the limiter is charged, and every
/// response (including errors) is logged and carries CORS headers.
pub fn app(state: AppState) -> Router {
    routes::router()
        .layer(middleware::from_fn_with_state(
            state.limiter.clone(),
            ratelimit::middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.cfg.clone(),
            cors::cors_middleware,
        ))
        .layer(middleware::from_fn(obs::log_requests))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn router_serves_healthz() {
        let response = app(AppState::new(Config::default()))
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::OK);
    }
}
