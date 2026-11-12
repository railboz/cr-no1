//! Upstream fetch of a guard-validated target.
//!
//! The relay is a dumb proxy: it returns the upstream body verbatim and never
//! interprets the calendar (the PWA already validates `BEGIN:VCALENDAR`, and
//! keeping the relay dumb keeps one parser out of the security boundary).
//!
//! # Threat model
//!
//! * **DNS rebinding.** [`crate::guard`] resolves the target and rejects every
//!   address that is not global. Between that check and our own connect, a
//!   second lookup could return `127.0.0.1`. So this module never lets the
//!   system resolver answer for an upstream name: [`PinTable`] is the client's
//!   only resolver, [`fetch_ics`] writes the guard-validated addresses into it
//!   right before the request, and any name that is not in the table fails
//!   closed. A rebinding answer therefore cannot be used even if the attacker
//!   controls the DNS server.
//! * **SSRF via redirects.** `redirect::Policy::none()`: a `3xx` is a refusal,
//!   never a new destination the guard has not seen.
//! * **Resource exhaustion.** The body is read chunk by chunk and the read is
//!   aborted as soon as it exceeds `cfg.max_bytes`; a declared
//!   `Content-Length` above the cap is refused before the first byte is read.
//! * **Secret leakage.** The target URL carries the user's private token, so no
//!   log line, error or response ever contains it: failures collapse into
//!   [`RelayError`] codes, and the debug trace only carries the allowlisted
//!   host suffix, the status and the byte count.

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::{Client, header, redirect};
use url::Host;

use crate::config::Config;
use crate::error::RelayError;
use crate::guard::ValidatedTarget;
use crate::obs;

/// `User-Agent` sent upstream. Fixed: client headers are never forwarded.
pub const USER_AGENT: &str = "Creno-Relay/1.0";
/// `Accept` sent upstream. Fixed for the same reason.
pub const ACCEPT: &str = "text/calendar, text/plain";
/// Upper bound on the TCP/TLS handshake, whatever `cfg.timeout` is.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Host → addresses decided by [`crate::guard`], shared by every request.
///
/// This is the client's resolver: it is consulted for **every** name, so an
/// absent entry is an error rather than a fallback to the system resolver.
#[derive(Debug, Default)]
struct PinTable {
    pinned: Mutex<std::collections::HashMap<String, Arc<[IpAddr]>>>,
}

impl PinTable {
    /// Records the addresses validated for `name`, replacing any previous pin.
    fn pin(&self, name: &str, ips: &[IpAddr]) {
        let mut table = self.pinned.lock().unwrap_or_else(PoisonError::into_inner);
        table.insert(normalise_name(name), Arc::from(ips));
    }

    /// Addresses for `name`, if the guard pinned this very name.
    fn lookup(&self, name: &str) -> Option<Arc<[IpAddr]>> {
        let table = self.pinned.lock().unwrap_or_else(PoisonError::into_inner);
        table.get(&normalise_name(name)).cloned()
    }
}

impl Resolve for PinTable {
    fn resolve(&self, name: Name) -> Resolving {
        let wanted = normalise_name(name.as_str());
        let found = self.lookup(&wanted);
        Box::pin(async move {
            match found {
                Some(ips) if !ips.is_empty() => {
                    let addrs: Vec<SocketAddr> =
                        ips.iter().map(|ip| SocketAddr::new(*ip, 0)).collect();
                    let addrs: Addrs = Box::new(addrs.into_iter());
                    Ok(addrs)
                }
                // Fail closed: never let the system resolver answer for an
                // upstream name, it would reopen the rebinding window.
                _ => Err(format!("upstream host {wanted:?} is not pinned").into()),
            }
        })
    }
}

/// Lowercase and drop the FQDN trailing dot, so `Example.COM.` and
/// `example.com` share one entry.
fn normalise_name(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

/// The client plus its pin table, built once per process.
///
/// A single client keeps connection pooling and TLS configuration out of the
/// request path. Pinning is per *target* rather than per client, which is why
/// the addresses live in [`PinTable`] instead of `ClientBuilder::resolve_to_addrs`:
/// the latter only exists at build time, and rebuilding a client per request
/// would throw the pool away.
#[derive(Debug)]
struct Shared {
    pins: Arc<PinTable>,
    client: Client,
}

fn shared() -> Result<&'static Shared, RelayError> {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    if let Some(shared) = SHARED.get() {
        return Ok(shared);
    }

    let pins = Arc::new(PinTable::default());
    let client = Client::builder()
        // One fixed identity for every upstream call; the client's own headers
        // are never forwarded.
        .user_agent(USER_AGENT)
        // A 3xx is a refusal: following it would reach a host the guard never
        // validated (SSRF) and could loop.
        .redirect(redirect::Policy::none())
        // Never go through an environment proxy: the address we connect to
        // must be the pinned one, not whatever an `HTTP_PROXY` decides.
        .no_proxy()
        // Explicit, though it is already the default: certificates are always
        // verified against the URL host (see the SNI note in `fetch_ics`).
        .danger_accept_invalid_certs(false)
        // Bounds the handshake even when `cfg.timeout` is the 120 s maximum.
        // Smaller `cfg.timeout` values tighten the deadline through the
        // per-request timeout below, so the effective connect budget is
        // `min(cfg.timeout, 5 s)`.
        .connect_timeout(CONNECT_TIMEOUT)
        // HTTP/1.1 only. One GET every five minutes gains nothing from h2, and
        // a single pinned plain-text connection per origin is easier to audit
        // than an ALPN-negotiated, multiplexed one.
        .http1_only()
        // The only resolver: see the module docs. Any name without a pin fails.
        .dns_resolver(pins.clone())
        .build()
        .map_err(|_| RelayError::UpstreamError)?;

    // If another thread won the race, its (equivalent) client is used.
    let _ = SHARED.set(Shared { pins, client });
    SHARED.get().ok_or(RelayError::UpstreamError)
}

/// Fetches the ICS document for a validated target.
///
/// Returns the raw event calendar bytes, verbatim. Every failure maps to one of
/// [`RelayError::UpstreamError`], [`RelayError::UpstreamTooLarge`] or
/// [`RelayError::Timeout`]; upstream status codes and messages never escape.
pub async fn fetch_ics(target: &ValidatedTarget, cfg: &Config) -> Result<Vec<u8>, RelayError> {
    // Wall-clock deadline for the whole exchange, body included. reqwest also
    // gets the same deadline per request, so a hung socket is abandoned either
    // way; this outer guard is what makes the bound independent of reqwest
    // internals.
    match tokio::time::timeout(cfg.timeout, fetch_pinned(target, cfg)).await {
        Ok(result) => result,
        Err(_elapsed) => Err(RelayError::Timeout),
    }
}

async fn fetch_pinned(target: &ValidatedTarget, cfg: &Config) -> Result<Vec<u8>, RelayError> {
    let shared = shared()?;

    // Pin the *URL host* (not `target.host`, which is the allowlisted suffix and
    // may be a parent domain) to the addresses the guard checked. TLS keeps
    // using the URL host for SNI and certificate verification, so pinning the
    // connection never weakens certificate checks: only the IP is forced.
    match target.url.host() {
        Some(Host::Domain(name)) => {
            if target.ips.is_empty() {
                return Err(RelayError::UpstreamError);
            }
            shared.pins.pin(name, &target.ips);
        }
        // An IP literal needs no pin: there is nothing a DNS answer could
        // change, and hyper connects to it directly.
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => {}
        None => return Err(RelayError::UpstreamError),
    }

    let mut response = shared
        .client
        .get(target.url.clone())
        .header(header::ACCEPT, ACCEPT)
        .timeout(cfg.timeout)
        .send()
        .await
        .map_err(map_transport_error)?;

    let status = response.status();
    if !status.is_success() {
        tracing::debug!(
            target: obs::TARGET,
            host = %target.host,
            status = status.as_u16(),
            "upstream refused the fetch"
        );
        return Err(RelayError::UpstreamError);
    }

    // Declared length first: refuse without reading a single byte when the
    // upstream already announces an oversized body.
    let declared = response.content_length();
    if let Some(len) = declared.filter(|len| *len > cfg.max_bytes as u64) {
        tracing::debug!(
            target: obs::TARGET,
            host = %target.host,
            status = status.as_u16(),
            declared = len,
            "upstream body announced above the cap"
        );
        return Err(RelayError::UpstreamTooLarge);
    }

    // Then stream, so a lying (or absent) `Content-Length` cannot make us
    // buffer more than the cap: the read stops the moment the cap is crossed,
    // and the connection is dropped while the upstream is still writing.
    let mut body = Vec::new();
    if let Some(len) = declared {
        body.reserve(len.min(cfg.max_bytes as u64) as usize);
    }
    loop {
        let Some(chunk) = response.chunk().await.map_err(map_transport_error)? else {
            break;
        };
        if body.len().saturating_add(chunk.len()) > cfg.max_bytes {
            tracing::debug!(
                target: obs::TARGET,
                host = %target.host,
                status = status.as_u16(),
                read = body.len().saturating_add(chunk.len()),
                "upstream body crossing the cap"
            );
            return Err(RelayError::UpstreamTooLarge);
        }
        body.extend_from_slice(&chunk);
    }

    tracing::debug!(
        target: obs::TARGET,
        host = %target.host,
        status = status.as_u16(),
        bytes = body.len(),
        "upstream fetched"
    );
    Ok(body)
}

/// Transport failures: a timeout keeps its code, everything else is a 502.
fn map_transport_error(error: reqwest::Error) -> RelayError {
    if error.is_timeout() {
        RelayError::Timeout
    } else {
        RelayError::UpstreamError
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &str) -> Name {
        raw.parse().expect("valid DNS name")
    }

    #[tokio::test]
    async fn pin_table_answers_only_pinned_names() {
        let table = PinTable::default();
        table.pin("API.Example.COM.", &[IpAddr::from([203, 0, 113, 7])]);

        let resolved: Vec<SocketAddr> = table
            .resolve(name("api.example.com"))
            .await
            .expect("pinned name resolves")
            .collect();
        assert_eq!(resolved, vec![SocketAddr::new([203, 0, 113, 7].into(), 0)]);

        assert!(
            table.resolve(name("evil.example.com")).await.is_err(),
            "an unpinned name must fail closed, never fall back to the system resolver"
        );
    }

    #[tokio::test]
    async fn an_empty_pin_list_is_an_error() {
        let table = PinTable::default();
        table.pin("example.com", &[]);
        assert!(table.resolve(name("example.com")).await.is_err());
    }

    #[test]
    fn names_are_normalised() {
        assert_eq!(normalise_name("API.Example.COM."), "api.example.com");
        assert_eq!(normalise_name("example.com"), "example.com");
    }
}
