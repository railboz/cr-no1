//! Target validation: the SSRF gate of the relay.
//!
//! The relay is a server-side fetcher: whatever it accepts, *it* requests from
//! inside the operator's network. This module is therefore the only place that
//! decides whether a URL may be fetched, and it is deliberately paranoid — the
//! fetch itself ([`crate::upstream`]) is only allowed to connect to the
//! addresses returned here, so a name that changes answer between the check and
//! the connection (DNS rebinding) cannot win.
//!
//! # Pipeline (every step fails closed)
//!
//! 1. **Shape** — absolute, at most [`MAX_TARGET_LEN`] bytes, `https` only, no
//!    userinfo, no fragment, no port other than the implicit `443`.
//! 2. **Host form** — a DNS name is required. IP literals are refused in every
//!    spelling the URL standard knows (dotted quad, bracketed IPv6, decimal,
//!    octal and hex integer forms): they can never be allowlisted, and letting
//!    them through would be an SSRF with extra steps.
//! 3. **Allowlist** — the name must equal one of `cfg.allowed_hosts` or be a
//!    subdomain of one, matched on label boundaries and case-insensitively.
//! 4. **Resolution** — the name is resolved *here*, once, and **every** address
//!    must be global unicast. A single private answer poisons the whole name
//!    (a DNS round-robin mixing `10.0.0.1` and a public address would otherwise
//!    let the fetch land inside the LAN). The step is bounded by
//!    `min(cfg.timeout, `[`DNS_TIMEOUT_CAP`]`)`: a resolver that never answers
//!    must not hold the axum task (and its blocking-pool thread) for ever, which
//!    would take `/healthz` down with the rest of the process.
//! 5. **Pinning** — the resolved addresses travel with the [`ValidatedTarget`],
//!    so the fetch never resolves the name again.
//!
//! # Error mapping
//!
//! Only two codes come out of this module, and the split is documented in the
//! public contract (`{"error":"<code>"}`):
//!
//! | situation | code | status |
//! | --- | --- | --- |
//! | empty `url` parameter | [`RelayError::MissingUrl`] | 400 |
//! | not an absolute URL, longer than [`MAX_TARGET_LEN`], carries a fragment, embeds credentials, or names a port other than 443 | [`RelayError::InvalidUrl`] | 400 |
//! | scheme is not `https`, an IP literal, a name outside the allowlist, a name that does not resolve, no address, or a non-global address | [`RelayError::ForbiddenHost`] | 403 |
//! | the resolver does not answer within `min(cfg.timeout, 5 s)` | [`RelayError::Timeout`] | 504 |
//!
//! The rationale: `invalid_url` means "this string is not a calendar URL we can
//! work with" — a fragment, userinfo or a non-standard port makes it a URL of
//! another kind, not one this relay would ever be asked to fetch, so it is a
//! shape error. `forbidden_host` means "this string is a calendar URL, but this
//! relay refuses to fetch it": everything the scheme check, the allowlist and
//! the IP deny list decide is a policy refusal on the target itself, not a
//! syntax error.

use std::future::Future;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::LazyLock;
use std::time::Duration;

use ipnet::{Ipv4Net, Ipv6Net};
use url::{Host, Url};

use crate::config::Config;
use crate::error::RelayError;

/// Only scheme the relay fetches. The PWA's calendars live on `https`.
pub const HTTPS_SCHEME: &str = "https";
/// Implicit port of `https`; any explicit other port is refused.
pub const HTTPS_PORT: u16 = 443;
/// Hard limit on an accepted target URL, in bytes.
///
/// Mirrors the `/ics` `url` parameter limit of the contract (2048 bytes) and is
/// re-checked here so the guard stays safe whatever the caller enforces.
pub const MAX_TARGET_LEN: usize = 2048;
/// Hard cap on the resolution step, whatever `CRENO_TIMEOUT_SECS` is.
///
/// A stuck resolver (dead upstream DNS, black-holed query) would otherwise hold
/// the axum task for ever: `/healthz` stops answering, the Docker healthcheck
/// flips the container to `unhealthy`, and a blocking-pool thread is never
/// returned. Resolution therefore gets `min(cfg.timeout, DNS_TIMEOUT_CAP)` and
/// a blow-through maps to `timeout` (504), exactly like a slow fetch.
pub const DNS_TIMEOUT_CAP: Duration = Duration::from_secs(5);

/// Deadline granted to the resolution step.
fn dns_budget(cfg: &Config) -> Duration {
    cfg.timeout.min(DNS_TIMEOUT_CAP)
}

/// A request target that passed every check.
///
/// `url` is the normalised absolute URL, `host` the DNS name that matched the
/// allowlist, and `ips` the addresses the connection must be pinned to.
///
/// `Debug` is hand-written: the URL carries the calendar token in its query
/// string, so a future `?target` in a log line must never print it.
#[derive(Clone, PartialEq, Eq)]
pub struct ValidatedTarget {
    /// Normalised absolute URL (`https` only, no credentials, no fragment,
    /// implicit port, lowercase host without a trailing dot).
    pub url: Url,
    /// DNS name that satisfied `cfg.allowed_hosts`, lowercase, no trailing dot.
    pub host: String,
    /// Resolved addresses, sorted and de-duplicated; the upstream client must
    /// connect to one of these and never resolve `host` again.
    pub ips: Vec<IpAddr>,
}

/// Debug rendering deliberately excludes the URL: it carries the private token
/// (`https://api.skello.io/ics/<id>?token=…`) and the relay must not be one
/// `tracing::debug!(?target)` away from logging it. Only the allowlisted host
/// and the pinned addresses are printable.
impl std::fmt::Debug for ValidatedTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatedTarget")
            .field("host", &self.host)
            .field("ips", &self.ips)
            .finish_non_exhaustive()
    }
}

/// Validates the raw `url` query parameter against `cfg`.
///
/// Resolution uses the system resolver; tests use
/// [`validate_with_resolver`] so they never touch the network.
///
/// ```no_run
/// # use creno_relay::{config::Config, guard::validate_target};
/// # async fn run() -> Result<(), creno_relay::error::RelayError> {
/// let cfg = Config::default();
/// let target = validate_target("https://api.skello.io/ics/abc/calendar.ics", &cfg).await?;
/// assert_eq!(target.host, "api.skello.io");
/// // `target.ips` is what the fetch has to connect to.
/// # let _ = target.ips;
/// # Ok(())
/// # }
/// ```
pub async fn validate_target(raw: &str, cfg: &Config) -> Result<ValidatedTarget, RelayError> {
    validate_with_resolver(raw, cfg, resolve_host).await
}

/// Validates `raw`, resolving through the injected `resolve` function.
///
/// Tests inject synthetic answers, which keeps the suite hermetic and lets it
/// exercise addresses that would be painful to obtain from real DNS (loopback,
/// `169.254.169.254`, mixed public/private round-robins…). `resolve` receives
/// the normalised host and the port (`443`) and returns the candidate
/// addresses; any error is treated as "cannot prove it is public".
pub(crate) async fn validate_with_resolver<F, Fut>(
    raw: &str,
    cfg: &Config,
    resolve: F,
) -> Result<ValidatedTarget, RelayError>
where
    F: FnOnce(String, u16) -> Fut,
    Fut: Future<Output = io::Result<Vec<IpAddr>>>,
{
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(RelayError::MissingUrl);
    }
    if raw.len() > MAX_TARGET_LEN {
        return Err(RelayError::InvalidUrl);
    }

    let mut url = Url::parse(raw).map_err(|_| RelayError::InvalidUrl)?;
    check_shape(&url)?;
    let domain = domain_host(&url)?;
    let host = normalise_host(&mut url, &domain)?;
    if !host_is_allowed(&host, &cfg.allowed_hosts) {
        return Err(RelayError::ForbiddenHost);
    }

    let port = url
        .port_or_known_default()
        .ok_or(RelayError::ForbiddenHost)?;
    // Justification for `ForbiddenHost` rather than `UpstreamError`: a name that
    // does not resolve cannot be *proven* public, so the gate refuses it exactly
    // like a private address, and the caller gets one uniform negative answer
    // (403) instead of an oracle telling "blocked" apart from "nonexistent".
    // `upstream_error` stays for failures of the fetch itself.
    //
    // The wait is bounded: a resolver that never answers is a `timeout` (504),
    // not an unbounded task.
    let mut ips = match tokio::time::timeout(dns_budget(cfg), resolve(host.clone(), port)).await {
        // Any resolution error is treated as "cannot prove it is public".
        Ok(Ok(ips)) => ips,
        Ok(Err(_)) => return Err(RelayError::ForbiddenHost),
        Err(_elapsed) => return Err(RelayError::Timeout),
    };

    if ips.is_empty() {
        return Err(RelayError::ForbiddenHost);
    }
    // One private answer is enough to refuse: DNS may round-robin.
    if ips.iter().any(|ip| !is_global(*ip)) {
        return Err(RelayError::ForbiddenHost);
    }

    ips.sort_unstable();
    ips.dedup();

    Ok(ValidatedTarget { url, host, ips })
}

/// Resolves through the system resolver (`/etc/hosts`, then DNS).
async fn resolve_host(host: String, port: u16) -> io::Result<Vec<IpAddr>> {
    let addrs = tokio::net::lookup_host((host.as_str(), port)).await?;
    Ok(addrs.map(|addr| addr.ip()).collect())
}

/// Refuses everything that is not a plain `https://host[:443]/…` reference.
fn check_shape(url: &Url) -> Result<(), RelayError> {
    if url.scheme() != HTTPS_SCHEME {
        // Contract: "scheme/host not allowlisted" is a `forbidden_host`.
        return Err(RelayError::ForbiddenHost);
    }
    // `https:///ics` parses with an empty host: addressable by nobody, so it is
    // an invalid URL rather than a policy refusal.
    if url.host_str().is_none_or(str::is_empty) {
        return Err(RelayError::InvalidUrl);
    }
    // `https://api.skello.io@evil.test/` and `https://user:pw@host/` are how
    // credential-smuggling and allowlist confusion start; the relay never needs
    // them (the calendar token lives in the query string). A URL carrying
    // credentials is a URL of another kind, so this is a shape error
    // (`invalid_url`), not a host-policy refusal.
    if !url.username().is_empty() || url.password().is_some() {
        return Err(RelayError::InvalidUrl);
    }
    // A fragment is never sent to a server and never carries meaning for a
    // calendar fetch: a URL with one is malformed for our purposes (and an
    // empty fragment, `…#`, counts too).
    if url.fragment().is_some() {
        return Err(RelayError::InvalidUrl);
    }
    match url.port() {
        // No explicit port, or the implicit 443 that the URL parser already
        // normalised away.
        None | Some(HTTPS_PORT) => Ok(()),
        // There is no configuration for alternative ports: only 443. A URL on
        // another port is not one the PWA can produce, so it is malformed for
        // our purposes (`invalid_url`), not a host-policy refusal.
        Some(_) => Err(RelayError::InvalidUrl),
    }
}

/// Extracts a DNS name, refusing every IP literal.
///
/// The `url` crate follows the WHATWG URL standard, so `2130706433`,
/// `0x7f.0.0.1`, `0177.0.0.1` and `127.1` all arrive here already normalised to
/// [`Host::Ipv4`], and `[::ffff:127.0.0.1]` to [`Host::Ipv6`]. Refusing the
/// variants wholesale is therefore enough to close every spelling.
fn domain_host(url: &Url) -> Result<String, RelayError> {
    match url.host() {
        Some(Host::Domain(domain)) => Ok(domain.to_ascii_lowercase()),
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => Err(RelayError::ForbiddenHost),
        // Already refused by `check_shape`; kept for totality.
        None => Err(RelayError::InvalidUrl),
    }
}

/// Lowercases the host and drops a trailing dot (FQDN spelling).
///
/// Choice documented: `https://api.skello.io./…` names the same host as
/// `https://api.skello.io/…`, so the dots are dropped *before* the allowlist
/// comparison and the normalised URL is rewritten, rather than letting a
/// harmless trailing dot be a bypass or a false refusal.
fn normalise_host(url: &mut Url, domain: &str) -> Result<String, RelayError> {
    let normalised = domain.trim_end_matches('.').to_ascii_lowercase();
    if normalised.is_empty() || normalised.split('.').any(str::is_empty) {
        return Err(RelayError::ForbiddenHost);
    }
    if normalised != domain {
        url.set_host(Some(&normalised))
            .map_err(|_| RelayError::ForbiddenHost)?;
    }
    Ok(normalised)
}

/// Allowlist match on label boundaries, case-insensitively.
///
/// `api.skello.io` accepts `api.skello.io` and `eu.api.skello.io`, and refuses
/// `skello.io.evil.test` and `evil-api.skello.io`: the suffix must be preceded
/// by a dot, not merely by any character. An empty allowlist accepts nothing.
fn host_is_allowed(host: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|suffix| {
        let suffix = suffix.trim_start_matches('.').trim_end_matches('.');
        !suffix.is_empty() && matches_suffix(host, suffix)
    })
}

/// `host` equals `suffix` or ends with `.` + `suffix`.
fn matches_suffix(host: &str, suffix: &str) -> bool {
    if host.len() < suffix.len() {
        return false;
    }
    let (prefix, tail) = host.split_at(host.len() - suffix.len());
    tail.eq_ignore_ascii_case(suffix) && (prefix.is_empty() || prefix.ends_with('.'))
}

/// IPv4 ranges the relay never fetches: everything that is not routable on the
/// public Internet, plus the documentation and benchmark blocks (they exist so
/// that examples and tests never point at a real host).
const DENIED_V4_CIDRS: &[&str] = &[
    "0.0.0.0/8",       // "this network", unspecified address
    "10.0.0.0/8",      // RFC 1918
    "100.64.0.0/10",   // CGNAT (RFC 6598)
    "127.0.0.0/8",     // loopback
    "169.254.0.0/16",  // link-local, cloud metadata
    "172.16.0.0/12",   // RFC 1918
    "192.0.0.0/24",    // IETF protocol assignments
    "192.0.2.0/24",    // TEST-NET-1
    "192.88.99.0/24",  // deprecated 6to4 relay anycast
    "192.168.0.0/16",  // RFC 1918
    "198.18.0.0/15",   // benchmarking
    "198.51.100.0/24", // TEST-NET-2
    "203.0.113.0/24",  // TEST-NET-3
    "224.0.0.0/4",     // multicast
    "240.0.0.0/4",     // reserved, includes 255.255.255.255
];

/// IPv6 ranges the relay never fetches.
///
/// Only global unicast (`2000::/3`) is reachable at all (see [`is_global_v6`]);
/// this list names the well-known exceptions *inside* that block (6to4, Teredo,
/// documentation…) and repeats the non-global blocks so the policy reads the
/// same way here as in [`DENIED_V4_CIDRS`].
const DENIED_V6_CIDRS: &[&str] = &[
    "::/128",         // unspecified
    "::1/128",        // loopback
    "::/96",          // IPv4-compatible (deprecated)
    "::ffff:0:0/96",  // IPv4-mapped
    "64:ff9b::/96",   // NAT64 well-known prefix
    "64:ff9b:1::/48", // NAT64 local-use prefix
    "100::/64",       // discard-only
    "2001::/23",      // IETF assignments: Teredo, benchmarking, ORCHID…
    "2001:db8::/32",  // documentation
    "2002::/16",      // 6to4 (embeds an IPv4 address)
    "3fff::/20",      // documentation (RFC 9637)
    "fc00::/7",       // unique local
    "fe80::/10",      // link-local
    "ff00::/8",       // multicast
];

/// Parsed once, on first use.
///
/// The CIDR strings are compile-time constants: a typo must fail loudly (the
/// unit tests initialise both lists, so it fails in CI) rather than silently
/// drop a range from a security policy.
static DENIED_V4: LazyLock<Vec<Ipv4Net>> = LazyLock::new(|| {
    DENIED_V4_CIDRS
        .iter()
        .map(|cidr| cidr.parse().expect("DENIED_V4_CIDRS holds constant CIDRs"))
        .collect()
});

/// Parsed once, on first use; see [`DENIED_V4`].
static DENIED_V6: LazyLock<Vec<Ipv6Net>> = LazyLock::new(|| {
    DENIED_V6_CIDRS
        .iter()
        .map(|cidr| cidr.parse().expect("DENIED_V6_CIDRS holds constant CIDRs"))
        .collect()
});

/// Whether `ip` may be fetched: public, routable, and nothing else.
fn is_global(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_global_v4(ip),
        IpAddr::V6(ip) => is_global_v6(ip),
    }
}

/// IPv4 addresses outside every [`DENIED_V4`] block.
fn is_global_v4(ip: Ipv4Addr) -> bool {
    !DENIED_V4.iter().any(|net| net.contains(&ip))
}

/// IPv6 addresses inside global unicast and outside every [`DENIED_V6`] block.
///
/// The IPv4-mapped form (`::ffff:10.0.0.1`, which `getaddrinfo` hands back for
/// IPv4 on dual-stack hosts) and the older IPv4-compatible form are unwrapped
/// first, so they are judged by the IPv4 rules — otherwise a mapped private
/// address would sail through on its IPv6 spelling.
fn is_global_v6(ip: Ipv6Addr) -> bool {
    if let Some(ip) = ip.to_ipv4() {
        return is_global_v4(ip);
    }
    // Global unicast is `2000::/3`: the three high bits are `001`. Everything
    // else (loopback, ULA, link-local, multicast, site-local, Teredo's parents,
    // future ranges) is unreachable by design.
    if ip.segments()[0] & 0xe000 != 0x2000 {
        return false;
    }
    !DENIED_V6.iter().any(|net| net.contains(&ip))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::net::Ipv4Addr;

    const PUBLIC_V4: &str = "93.184.216.34";
    const PUBLIC_V6: &str = "2606:2800:220:1:248:1893:25c8:1946";

    fn cfg(hosts: &[&str]) -> Config {
        Config {
            allowed_hosts: hosts.iter().map(|host| (*host).to_string()).collect(),
            ..Config::default()
        }
    }

    fn ips(list: &[&str]) -> Vec<IpAddr> {
        list.iter()
            .map(|raw| raw.parse().expect("test address parses"))
            .collect()
    }

    /// Validates `raw` against `cfg` with a resolver that always answers `list`.
    async fn check(raw: &str, cfg: &Config, list: &[&str]) -> Result<ValidatedTarget, RelayError> {
        let answered = ips(list);
        validate_with_resolver(raw, cfg, move |_host, _port| async move { Ok(answered) }).await
    }

    /// Validates `raw` and asserts a `ForbiddenHost` refusal.
    async fn forbidden(raw: &str, cfg: &Config) {
        assert_eq!(
            check(raw, cfg, &[PUBLIC_V4]).await,
            Err(RelayError::ForbiddenHost),
            "{raw} must be refused"
        );
    }

    #[tokio::test]
    async fn happy_path_returns_normalised_url_host_and_pinned_ips() {
        let target = check(
            "https://API.Skello.IO./ics/abc/calendar.ics?token=s3cr3t",
            &cfg(&["api.skello.io"]),
            &[PUBLIC_V4, PUBLIC_V6],
        )
        .await
        .expect("public, allowlisted target");

        assert_eq!(target.host, "api.skello.io");
        assert_eq!(
            target.url.as_str(),
            "https://api.skello.io/ics/abc/calendar.ics?token=s3cr3t",
            "host lowercased, trailing dot dropped, query preserved"
        );
        assert_eq!(target.ips, ips(&[PUBLIC_V4, PUBLIC_V6]), "sorted, deduped");

        // The parser drops the implicit port, and 443 stays implied.
        assert_eq!(target.url.port(), None);
        let with_port = check(
            "https://api.skello.io:443/ics",
            &cfg(&["api.skello.io"]),
            &[PUBLIC_V4],
        )
        .await
        .expect("explicit 443 is the same thing");
        assert_eq!(with_port.url.port(), None);
    }

    #[tokio::test]
    async fn resolver_receives_the_normalised_host_and_the_https_port() {
        let seen: RefCell<Option<(String, u16)>> = RefCell::new(None);
        let target = validate_with_resolver(
            "https://Sub.API.skello.io./ics",
            &cfg(&["api.skello.io"]),
            |host, port| {
                *seen.borrow_mut() = Some((host, port));
                async { Ok(ips(&[PUBLIC_V4])) }
            },
        )
        .await
        .expect("valid target");

        assert_eq!(target.host, "sub.api.skello.io");
        assert_eq!(
            seen.into_inner(),
            Some(("sub.api.skello.io".to_string(), HTTPS_PORT))
        );
    }

    #[tokio::test]
    async fn non_https_schemes_are_forbidden() {
        for raw in [
            "http://api.skello.io/ics",
            "ftp://api.skello.io/ics",
            "file:///etc/passwd",
            "gopher://api.skello.io:70/_x",
            "data:text/calendar,BEGIN:VCALENDAR",
            "javascript:alert(1)",
        ] {
            forbidden(raw, &cfg(&["api.skello.io"])).await;
        }
    }

    #[tokio::test]
    async fn url_shape_errors_map_to_the_documented_codes() {
        let cfg = cfg(&["api.skello.io"]);

        assert_eq!(
            check("", &cfg, &[PUBLIC_V4]).await,
            Err(RelayError::MissingUrl)
        );
        assert_eq!(
            check("   ", &cfg, &[PUBLIC_V4]).await,
            Err(RelayError::MissingUrl)
        );

        for raw in [
            "not a url",
            "//api.skello.io/ics",
            "api.skello.io/ics",
            "https://",
            "https://?url=x",
        ] {
            assert_eq!(
                check(raw, &cfg, &[PUBLIC_V4]).await,
                Err(RelayError::InvalidUrl),
                "{raw} must be invalid"
            );
        }

        // Not invalid but forbidden: for a special scheme the WHATWG parser
        // ignores the extra slashes, so `https:///ics` is the host `ics`.
        assert_eq!(
            check("https:///ics", &cfg, &[PUBLIC_V4]).await,
            Err(RelayError::ForbiddenHost)
        );

        // 2049 bytes, one above the contract limit.
        let long = format!("https://api.skello.io/{}", "a".repeat(MAX_TARGET_LEN));
        assert_eq!(
            check(&long, &cfg, &[PUBLIC_V4]).await,
            Err(RelayError::InvalidUrl)
        );
        assert_eq!(MAX_TARGET_LEN, 2048);
    }

    #[tokio::test]
    async fn fragments_are_invalid() {
        let cfg = cfg(&["api.skello.io"]);
        for raw in [
            "https://api.skello.io/ics#token",
            "https://api.skello.io/ics?#",
            "https://api.skello.io/#",
        ] {
            assert_eq!(
                check(raw, &cfg, &[PUBLIC_V4]).await,
                Err(RelayError::InvalidUrl),
                "{raw} carries a fragment"
            );
        }
    }

    #[tokio::test]
    async fn userinfo_is_invalid() {
        let cfg = cfg(&["api.skello.io", "evil.test"]);
        for raw in [
            "https://user:password@api.skello.io/ics",
            "https://user@api.skello.io/ics",
            // The classic confusion: host is `evil.test`, allowlisted here on
            // purpose to prove credentials alone are refused.
            "https://api.skello.io@evil.test/ics",
        ] {
            assert_eq!(
                check(raw, &cfg, &[PUBLIC_V4]).await,
                Err(RelayError::InvalidUrl),
                "{raw} carries credentials"
            );
        }
    }

    #[tokio::test]
    async fn only_the_https_port_is_accepted() {
        let cfg = cfg(&["api.skello.io"]);
        for raw in [
            "https://api.skello.io:8443/ics",
            "https://api.skello.io:80/ics",
            "https://api.skello.io:0/ics",
            "https://api.skello.io:65535/ics",
        ] {
            assert_eq!(
                check(raw, &cfg, &[PUBLIC_V4]).await,
                Err(RelayError::InvalidUrl),
                "{raw} names a port other than 443"
            );
        }
        assert!(
            check("https://api.skello.io:443/ics", &cfg, &[PUBLIC_V4])
                .await
                .is_ok(),
            "explicit 443 is the implicit port"
        );
    }

    #[tokio::test]
    async fn ip_literals_are_forbidden_in_every_spelling() {
        let cfg = cfg(&["api.skello.io"]);
        let calls = Cell::new(0);
        for raw in [
            "https://127.0.0.1/ics",
            "https://[::1]/ics",
            "https://[::ffff:127.0.0.1]/ics",
            "https://2130706433/ics",
            "https://0x7f000001/ics",
            "https://0177.0.0.1/ics",
            "https://127.1/ics",
            "https://169.254.169.254/latest/meta-data/",
            "https://[::ffff:169.254.169.254]/ics",
            "https://0.0.0.0/ics",
            "https://[fe80::1]/ics",
            "https://[fd00::1]/ics",
            "https://93.184.216.34/ics",
        ] {
            let result = validate_with_resolver(raw, &cfg, |_host: String, _port| {
                calls.set(calls.get() + 1);
                async { Ok(ips(&[PUBLIC_V4])) }
            })
            .await;
            assert_eq!(result, Err(RelayError::ForbiddenHost), "{raw}");
        }
        assert_eq!(
            calls.get(),
            0,
            "IP literals must be refused before any resolution"
        );
    }

    #[tokio::test]
    async fn allowlist_matches_on_label_boundaries_only() {
        let cfg = cfg(&["api.skello.io"]);
        for raw in [
            "https://api.skello.io/ics",
            "https://API.SKELLO.IO/ics",
            "https://api.skello.io./ics",
            "https://eu.api.skello.io/ics",
        ] {
            assert!(
                check(raw, &cfg, &[PUBLIC_V4]).await.is_ok(),
                "{raw} must be allowed"
            );
        }

        for raw in [
            "https://skello.io/ics",
            "https://skello.io.evil.test/ics",
            "https://evil-api.skello.io/ics",
            "https://api.skello.io.evil.test/ics",
            "https://other.test/ics",
            "https://api.skello.iot/ics",
            "https://apiskello.io/ics",
        ] {
            assert_eq!(
                check(raw, &cfg, &[PUBLIC_V4]).await,
                Err(RelayError::ForbiddenHost),
                "{raw} must not match api.skello.io"
            );
        }
    }

    #[tokio::test]
    async fn empty_allowlist_forbids_everything() {
        let cfg = cfg(&[]);
        for raw in [
            "https://api.skello.io/ics",
            "https://example.com/ics",
            "https://93.184.216.34/ics",
        ] {
            forbidden(raw, &cfg).await;
        }
    }

    #[tokio::test]
    async fn any_non_global_answer_poisons_the_name() {
        let cfg = cfg(&["api.skello.io"]);
        for list in [
            &["127.0.0.1"][..],
            &["10.0.0.1"],
            &["172.16.0.1"],
            &["192.168.1.10"],
            &["169.254.169.254"],
            &["100.64.0.1"],
            &["0.0.0.0"],
            &["255.255.255.255"],
            &["224.0.0.1"],
            &["192.0.2.1"],
            &["198.51.100.7"],
            &["203.0.113.9"],
            &["::1"],
            &["::"],
            &["fe80::1"],
            &["fd12:3456::1"],
            &["ff02::1"],
            &["2001:db8::1"],
            &["::ffff:10.0.0.1"],
            &["64:ff9b::a00:1"],
            &["2002:a00:1::1"],
            &["2001::1"],
            // One public address does not redeem a private one.
            &[PUBLIC_V4, "10.0.0.1"],
            &[PUBLIC_V6, "127.0.0.1"],
            &[PUBLIC_V4, "169.254.169.254"],
        ] {
            assert_eq!(
                check("https://api.skello.io/ics", &cfg, list).await,
                Err(RelayError::ForbiddenHost),
                "{list:?} must be refused"
            );
        }
    }

    #[tokio::test]
    async fn public_answers_are_accepted() {
        let cfg = cfg(&["api.skello.io"]);
        for list in [
            &[PUBLIC_V4][..],
            &[PUBLIC_V6],
            &[PUBLIC_V4, PUBLIC_V6],
            &["1.1.1.1"],
            &["9.9.9.9", PUBLIC_V6],
        ] {
            let target = check("https://api.skello.io/ics", &cfg, list)
                .await
                .unwrap_or_else(|error| panic!("{list:?} must be accepted, got {error}"));
            assert!(!target.ips.is_empty(), "{list:?}");
        }
    }

    #[tokio::test]
    async fn resolution_failures_and_empty_answers_fail_closed() {
        let cfg = cfg(&["api.skello.io"]);

        let dns_error =
            validate_with_resolver("https://api.skello.io/ics", &cfg, |_host, _port| async {
                Err(io::Error::other("no such host"))
            })
            .await;
        assert_eq!(dns_error, Err(RelayError::ForbiddenHost));

        let empty = check("https://api.skello.io/ics", &cfg, &[]).await;
        assert_eq!(empty, Err(RelayError::ForbiddenHost));

        // A duplicate answer is collapsed, not rejected.
        let duplicated = check("https://api.skello.io/ics", &cfg, &[PUBLIC_V4, PUBLIC_V4])
            .await
            .expect("duplicates are fine");
        assert_eq!(duplicated.ips, ips(&[PUBLIC_V4]));
    }

    #[test]
    fn debug_never_prints_the_token_bearing_url() {
        const TOKEN: &str = "s3cr3t-token-6f1a2f83";
        let target = ValidatedTarget {
            url: Url::parse(&format!(
                "https://api.skello.io/ics/abc/calendar.ics?token={TOKEN}"
            ))
            .expect("test URL"),
            host: "api.skello.io".to_string(),
            ips: ips(&[PUBLIC_V4]),
        };

        let rendered = format!("{target:?}");
        assert!(rendered.contains("api.skello.io"), "{rendered}");
        assert!(rendered.contains(PUBLIC_V4), "{rendered}");
        assert!(
            !rendered.contains(TOKEN),
            "token leaked in Debug: {rendered}"
        );
        assert!(
            !rendered.contains("token"),
            "query leaked in Debug: {rendered}"
        );
        assert!(
            !rendered.contains("calendar.ics"),
            "path leaked in Debug: {rendered}"
        );
        assert!(!rendered.contains("https://"), "URL leaked: {rendered}");
        // `ips` is still a plain list, so a debug line stays diagnosable.
        assert!(
            rendered.starts_with("ValidatedTarget { host: \"api.skello.io\""),
            "{rendered}"
        );
        assert!(rendered.ends_with(".. }"), "{rendered}");
    }

    #[tokio::test]
    async fn a_hanging_resolver_is_a_timeout_not_an_endless_wait() {
        let mut cfg = cfg(&["api.skello.io"]);
        // A tiny budget keeps the test fast; production floors `CRENO_TIMEOUT_SECS`
        // at 1 s, and the 5 s cap is asserted below.
        cfg.timeout = Duration::from_millis(20);

        let started = std::time::Instant::now();
        let result = validate_with_resolver(
            "https://api.skello.io/ics",
            &cfg,
            |_host: String, _port| async {
                // A resolver that never answers, like a black-holed DNS query.
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(ips(&[PUBLIC_V4]))
            },
        )
        .await;

        assert_eq!(result, Err(RelayError::Timeout));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the guard must give up on the resolver, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn the_resolution_budget_is_the_smaller_of_the_request_timeout_and_the_cap() {
        // Default `CRENO_TIMEOUT_SECS=12` is above the cap, so DNS never gets
        // more than five seconds.
        assert_eq!(dns_budget(&Config::default()), DNS_TIMEOUT_CAP);
        assert_eq!(DNS_TIMEOUT_CAP, Duration::from_secs(5));

        // A tighter request timeout tightens the resolution budget with it.
        let tight = Config {
            timeout: Duration::from_secs(2),
            ..Config::default()
        };
        assert_eq!(dns_budget(&tight), Duration::from_secs(2));

        // The maximum accepted `CRENO_TIMEOUT_SECS` cannot widen it either.
        let wide = Config {
            timeout: Duration::from_secs(120),
            ..Config::default()
        };
        assert_eq!(dns_budget(&wide), DNS_TIMEOUT_CAP);
    }

    #[test]
    fn deny_lists_are_parsed_and_complete() {
        // Touching the lazies here means a mistyped CIDR fails in CI (the
        // `expect` panics) instead of at the first request in production.
        assert_eq!(DENIED_V4.len(), DENIED_V4_CIDRS.len());
        assert_eq!(DENIED_V6.len(), DENIED_V6_CIDRS.len());
        assert_eq!(DENIED_V4.len(), 15);
        assert_eq!(DENIED_V6.len(), 14);
        // Every network survives a parse/print round trip: the table is what it
        // says it is, and nothing was silently dropped.
        for (cidr, net) in DENIED_V4_CIDRS.iter().zip(DENIED_V4.iter()) {
            assert_eq!(cidr.parse::<Ipv4Net>().as_ref(), Ok(net), "{cidr}");
            assert_eq!(net.to_string().parse::<Ipv4Net>().as_ref(), Ok(net));
        }
        for (cidr, net) in DENIED_V6_CIDRS.iter().zip(DENIED_V6.iter()) {
            assert_eq!(cidr.parse::<Ipv6Net>().as_ref(), Ok(net), "{cidr}");
            assert_eq!(net.to_string().parse::<Ipv6Net>().as_ref(), Ok(net));
        }
    }

    #[test]
    fn global_address_classification() {
        for raw in [
            PUBLIC_V4,
            "1.1.1.1",
            "8.8.8.8",
            "34.243.56.7",
            "192.0.1.1",
            PUBLIC_V6,
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "2a00:1450:4007:80c::200e",
        ] {
            let ip: IpAddr = raw.parse().expect("test address");
            assert!(is_global(ip), "{raw} must be global");
        }

        for raw in [
            "0.0.0.0",
            "0.0.0.1",
            "10.1.2.3",
            "100.64.0.1",
            "100.127.255.254",
            "127.0.0.1",
            "127.255.255.254",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.254",
            "192.0.0.8",
            "192.0.2.1",
            "192.88.99.1",
            "192.168.0.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "::10.0.0.1",
            "64:ff9b::1.2.3.4",
            "100::1",
            "2001::1",
            "2001:2::1",
            "2001:db8::1",
            "2002:7f00:1::1",
            "3fff::1",
            "fc00::1",
            "fd12:3456:789a::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
        ] {
            let ip: IpAddr = raw.parse().expect("test address");
            assert!(!is_global(ip), "{raw} must be refused");
        }
    }

    #[test]
    fn boundary_arithmetic_of_the_deny_lists() {
        // Just outside each edge, the answer flips: proves the prefixes are
        // applied on whole networks, not on string prefixes.
        // 100.64.0.0/10 ends at 100.127.255.255.
        assert!(!is_global("100.127.255.255".parse().expect("ip")));
        assert!(is_global("100.128.0.0".parse().expect("ip")));
        assert!(is_global("100.63.255.255".parse().expect("ip")));
        // 172.16.0.0/12 ends at 172.31.255.255.
        assert!(!is_global("172.31.255.255".parse().expect("ip")));
        assert!(is_global("172.32.0.0".parse().expect("ip")));
        assert!(is_global("172.15.255.255".parse().expect("ip")));
        // 169.254.0.0/16 does not swallow 169.253/169.255.
        assert!(!is_global("169.254.255.255".parse().expect("ip")));
        assert!(is_global("169.253.255.255".parse().expect("ip")));
        assert!(is_global("169.255.0.0".parse().expect("ip")));
        // 198.18.0.0/15 covers 198.18 and 198.19 only.
        assert!(!is_global("198.19.255.255".parse().expect("ip")));
        assert!(is_global("198.20.0.0".parse().expect("ip")));
        // 2000::/3 is the global unicast block: 3fff::/20 is the last denied
        // exception inside it.
        assert!(!is_global("3fff:fff::1".parse().expect("ip")));
        assert!(is_global("3fff:1000::1".parse().expect("ip")));
        assert!(!is_global("4000::1".parse().expect("ip")));
        // IPv4-mapped answers are judged as IPv4, at both ends of the table.
        assert!(is_global("::ffff:93.184.216.34".parse().expect("ip")));
        assert!(!is_global("::ffff:169.254.169.254".parse().expect("ip")));
        assert!(is_global(IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4))));
    }

    #[test]
    fn suffix_matching_is_label_aware_and_case_insensitive() {
        for host in [
            "api.skello.io",
            "API.SKELLO.IO",
            "eu.api.skello.io",
            "a.b.api.skello.io",
        ] {
            assert!(matches_suffix(host, "api.skello.io"), "{host}");
        }
        for host in [
            "apiskello.io",
            "api.skello.iot",
            "xapi.skello.io",
            "skello.io",
            "io",
            "",
        ] {
            assert!(!matches_suffix(host, "api.skello.io"), "{host}");
        }
        assert!(matches_suffix("api.skello.io", "io"));

        // Allowlist entries are compared defensively: a leading or trailing dot
        // in the configuration cannot change the semantics, a blank entry never
        // matches.
        let allowed = vec![
            ".api.skello.io.".to_string(),
            "  ".to_string(),
            String::new(),
        ];
        assert!(host_is_allowed("api.skello.io", &allowed));
        assert!(!host_is_allowed("skello.io", &allowed));
        assert!(!host_is_allowed("api.skello.io", &[]));
    }

    #[test]
    fn error_codes_map_to_the_contract() {
        assert_eq!(RelayError::MissingUrl.status().as_u16(), 400);
        assert_eq!(RelayError::InvalidUrl.status().as_u16(), 400);
        assert_eq!(RelayError::ForbiddenHost.status().as_u16(), 403);
        assert_eq!(RelayError::MissingUrl.code(), "missing_url");
        assert_eq!(RelayError::InvalidUrl.code(), "invalid_url");
        assert_eq!(RelayError::ForbiddenHost.code(), "forbidden_host");
    }
}
