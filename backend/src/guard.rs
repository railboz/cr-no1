//! Target validation (wave 1 stub).
//!
//! Wave 2 owns this file: it must reject everything that is not an allowlisted
//! `https` host resolving exclusively to public IPs (no loopback, no private
//! range, no link-local, no CGNAT), then hand the resolved addresses to
//! [`crate::upstream`] so the connection is pinned to them (no DNS rebinding).

use std::net::IpAddr;

use url::Url;

use crate::config::Config;
use crate::error::RelayError;

/// A request target that passed every check.
///
/// `url` is the normalised absolute URL, `host` the allowlisted suffix that
/// matched it, and `ips` the addresses the connection must be pinned to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTarget {
    /// Normalised absolute URL (`https` only, standard port, no credentials).
    pub url: Url,
    /// Allowlisted host suffix that accepted `url`.
    pub host: String,
    /// Resolved addresses; the upstream client must connect to one of these.
    pub ips: Vec<IpAddr>,
}

/// Validates the raw `url` query parameter against `cfg`.
///
/// Wave 1 stub: always refuses with [`RelayError::ForbiddenHost`], so the relay
/// can never fetch anything until wave 2 lands the real checks.
pub async fn validate_target(raw: &str, _cfg: &Config) -> Result<ValidatedTarget, RelayError> {
    let _ = raw;
    // TODO(wave2): parse with `Url::parse`, require `https`, reject userinfo,
    // reject non-default port, match `cfg.allowed_hosts` on label boundaries,
    // resolve A/AAAA, reject non-global IPs (`ipnet`), return pinned `ips`.
    Err(RelayError::ForbiddenHost)
}
