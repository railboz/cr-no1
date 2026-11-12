//! Upstream fetch (wave 1 stub).
//!
//! Wave 2 owns this file: build a single shared `reqwest::Client`
//! (`rustls`, no system proxy, `redirect::Policy::none`), connect only to the
//! IPs pinned by [`crate::guard::ValidatedTarget`], enforce `cfg.timeout`, and
//! stream the body with a hard cap of `cfg.max_bytes` before buffering it.

use crate::config::Config;
use crate::error::RelayError;
use crate::guard::ValidatedTarget;

/// Fetches the ICS document for a validated target.
///
/// Returns the raw event calendar bytes, verbatim. Every failure maps to one of
/// [`RelayError::UpstreamError`], [`RelayError::UpstreamTooLarge`] or
/// [`RelayError::Timeout`]; upstream status codes and messages never escape.
pub async fn fetch_ics(_target: &ValidatedTarget, _cfg: &Config) -> Result<Vec<u8>, RelayError> {
    // TODO(wave2): real HTTP GET with timeout + size cap + pinned resolution.
    Err(RelayError::UpstreamError)
}
