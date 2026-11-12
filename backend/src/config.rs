//! Environment-driven configuration.
//!
//! Every value is read once at startup and validated eagerly: the process
//! refuses to start on an invalid configuration instead of failing later on a
//! request. Error messages name the offending variable and the expected shape,
//! never the value itself (values may contain a private host list).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use thiserror::Error;

use url::Url;

/// Environment variable holding the listen address.
pub const ENV_BIND: &str = "CRENO_BIND";
/// Environment variable holding the comma-separated host suffix allowlist.
pub const ENV_ALLOWED_HOSTS: &str = "CRENO_ALLOWED_HOSTS";
/// Environment variable holding the maximum upstream body size in bytes.
pub const ENV_MAX_BYTES: &str = "CRENO_MAX_BYTES";
/// Environment variable holding the upstream request timeout in seconds.
pub const ENV_TIMEOUT_SECS: &str = "CRENO_TIMEOUT_SECS";
/// Environment variable holding the per-IP request budget, per minute.
pub const ENV_RATE_LIMIT: &str = "CRENO_RATE_LIMIT";
/// Environment variable allowing `moz-extension://` origins.
pub const ENV_ALLOW_EXTENSION: &str = "CRENO_ALLOW_EXTENSION";
/// Environment variable allowing the `null` origin (`file://` documents).
pub const ENV_ALLOW_FILE_ORIGIN: &str = "CRENO_ALLOW_FILE_ORIGIN";
/// Environment variable holding additional exact origins.
pub const ENV_ALLOWED_ORIGINS: &str = "CRENO_ALLOWED_ORIGINS";

/// Default listen address: loopback only, so an unconfigured relay is never
/// exposed to the network by accident.
pub const DEFAULT_BIND: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
/// Default host suffix allowlist (the Skello ICS host).
pub const DEFAULT_ALLOWED_HOSTS: &str = "api.skello.io";
/// Default maximum upstream body size: 2 MiB.
pub const DEFAULT_MAX_BYTES: usize = 2 * 1024 * 1024;
/// Default upstream timeout: 12 seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 12;
/// Default per-IP budget: 30 requests per minute.
pub const DEFAULT_RATE_LIMIT: u32 = 30;
/// Burst size of the per-IP token bucket (fixed by the contract).
pub const RATE_BURST: u32 = 10;

/// Smallest accepted body cap; below that a calendar fetch is pointless.
pub const MIN_MAX_BYTES: usize = 1024;
/// Largest accepted body cap, guards the process against absurd values.
pub const MAX_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Smallest accepted upstream timeout.
pub const MIN_TIMEOUT_SECS: u64 = 1;
/// Largest accepted upstream timeout.
pub const MAX_TIMEOUT_SECS: u64 = 120;
/// Largest accepted rate limit, per minute.
pub const MAX_RATE_LIMIT: u32 = 100_000;

/// Fully validated runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Address the HTTP server binds to.
    pub bind: SocketAddr,
    /// Allowlisted host suffixes, lowercase, without a leading dot.
    pub allowed_hosts: Vec<String>,
    /// Maximum number of bytes accepted from the upstream calendar.
    pub max_bytes: usize,
    /// Upstream request timeout.
    pub timeout: Duration,
    /// Per-IP request budget, per minute.
    pub rate_limit_per_minute: u32,
    /// Per-IP token bucket capacity.
    pub rate_burst: u32,
    /// Whether `moz-extension://` origins are granted CORS access.
    pub allow_extension: bool,
    /// Whether the `null` origin (`file://` documents) is granted CORS access.
    ///
    /// Off by default: `Origin: null` is what *any* sandboxed iframe of *any*
    /// site sends, so granting it would let a random page drain the relay's
    /// token-bearing fetch. It only makes sense for the PWA opened straight from
    /// `index.html` on `file://`.
    pub allow_file_origin: bool,
    /// Additional exact origins granted CORS access.
    pub extra_origins: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND,
            allowed_hosts: vec![DEFAULT_ALLOWED_HOSTS.to_string()],
            max_bytes: DEFAULT_MAX_BYTES,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            rate_limit_per_minute: DEFAULT_RATE_LIMIT,
            rate_burst: RATE_BURST,
            allow_extension: false,
            allow_file_origin: false,
            extra_origins: Vec::new(),
        }
    }
}

/// Configuration rejection. Rendered at startup, then the process exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// `CRENO_BIND` is not a socket address.
    #[error("CRENO_BIND must be an IP:port socket address (default 127.0.0.1:8080)")]
    Bind,
    /// `CRENO_ALLOWED_HOSTS` is empty or contains an invalid suffix.
    #[error(
        "CRENO_ALLOWED_HOSTS must be a comma-separated list of bare domain suffixes \
         such as api.example.com (no scheme, no port, no wildcard)"
    )]
    AllowedHosts,
    /// `CRENO_MAX_BYTES` is out of range or not a number.
    #[error("CRENO_MAX_BYTES must be an integer between 1 KiB and 64 MiB")]
    MaxBytes,
    /// `CRENO_TIMEOUT_SECS` is out of range or not a number.
    #[error("CRENO_TIMEOUT_SECS must be an integer between 1 and 120")]
    Timeout,
    /// `CRENO_RATE_LIMIT` is out of range or not a number.
    #[error("CRENO_RATE_LIMIT must be an integer between 1 and 100000 (requests per minute)")]
    RateLimit,
    /// `CRENO_ALLOW_EXTENSION` is not a boolean.
    #[error("CRENO_ALLOW_EXTENSION must be 0 or 1")]
    AllowExtension,
    /// `CRENO_ALLOW_FILE_ORIGIN` is not a boolean.
    #[error("CRENO_ALLOW_FILE_ORIGIN must be 0 or 1")]
    AllowFileOrigin,
    /// `CRENO_ALLOWED_ORIGINS` contains a malformed origin.
    #[error(
        "CRENO_ALLOWED_ORIGINS must be a comma-separated list of exact http(s) origins \
         such as https://app.example.fr (no path, no wildcard)"
    )]
    AllowedOrigins,
}

impl Config {
    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Reads the configuration from an arbitrary lookup, so tests never have to
    /// mutate the process environment (which is racy across threads).
    pub fn from_lookup<F>(mut get: F) -> Result<Self, ConfigError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut read = |key: &str| -> Option<String> {
            get(key)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };

        let bind = match read(ENV_BIND) {
            Some(raw) => raw.parse::<SocketAddr>().map_err(|_| ConfigError::Bind)?,
            None => DEFAULT_BIND,
        };

        let allowed_hosts = match read(ENV_ALLOWED_HOSTS) {
            Some(raw) => parse_hosts(&raw)?,
            None => parse_hosts(DEFAULT_ALLOWED_HOSTS)?,
        };

        let max_bytes = match read(ENV_MAX_BYTES) {
            Some(raw) => {
                let value = parse_u64(&raw, ConfigError::MaxBytes)?;
                let value = usize::try_from(value).map_err(|_| ConfigError::MaxBytes)?;
                if !(MIN_MAX_BYTES..=MAX_MAX_BYTES).contains(&value) {
                    return Err(ConfigError::MaxBytes);
                }
                value
            }
            None => DEFAULT_MAX_BYTES,
        };

        let timeout = match read(ENV_TIMEOUT_SECS) {
            Some(raw) => {
                let secs = parse_u64(&raw, ConfigError::Timeout)?;
                if !(MIN_TIMEOUT_SECS..=MAX_TIMEOUT_SECS).contains(&secs) {
                    return Err(ConfigError::Timeout);
                }
                Duration::from_secs(secs)
            }
            None => Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        };

        let rate_limit_per_minute = match read(ENV_RATE_LIMIT) {
            Some(raw) => {
                let value = parse_u64(&raw, ConfigError::RateLimit)?;
                let value = u32::try_from(value).map_err(|_| ConfigError::RateLimit)?;
                if !(1..=MAX_RATE_LIMIT).contains(&value) {
                    return Err(ConfigError::RateLimit);
                }
                value
            }
            None => DEFAULT_RATE_LIMIT,
        };

        let allow_extension = match read(ENV_ALLOW_EXTENSION) {
            Some(raw) => parse_bool(&raw).ok_or(ConfigError::AllowExtension)?,
            None => false,
        };

        let allow_file_origin = match read(ENV_ALLOW_FILE_ORIGIN) {
            Some(raw) => parse_bool(&raw).ok_or(ConfigError::AllowFileOrigin)?,
            None => false,
        };

        let extra_origins = match read(ENV_ALLOWED_ORIGINS) {
            Some(raw) => parse_origins(&raw)?,
            None => Vec::new(),
        };

        Ok(Self {
            bind,
            allowed_hosts,
            max_bytes,
            timeout,
            rate_limit_per_minute,
            rate_burst: RATE_BURST,
            allow_extension,
            allow_file_origin,
            extra_origins,
        })
    }
}

fn parse_u64(raw: &str, error: ConfigError) -> Result<u64, ConfigError> {
    raw.parse::<u64>().map_err(|_| error)
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Parses a comma-separated list of bare domain suffixes.
///
/// Entries are lowercased, de-duplicated and stripped of a leading dot
/// (`.example.com` and `example.com` mean the same thing).
fn parse_hosts(raw: &str) -> Result<Vec<String>, ConfigError> {
    let mut hosts: Vec<String> = Vec::new();
    for entry in raw.split(',') {
        let host = entry.trim().trim_start_matches('.');
        if host.is_empty() {
            continue;
        }
        let host = host.to_ascii_lowercase();
        if !is_host_suffix(&host) {
            return Err(ConfigError::AllowedHosts);
        }
        if !hosts.iter().any(|known| known == &host) {
            hosts.push(host);
        }
    }
    if hosts.is_empty() {
        return Err(ConfigError::AllowedHosts);
    }
    Ok(hosts)
}

/// A host suffix is a bare DNS name: at least two lowercase labels, no scheme,
/// no port, no path, no wildcard, no leading dash, no empty label.
///
/// The two-label minimum is the whole point of the allowlist: a bare TLD such as
/// `io` would accept *every* public host under it, i.e. turn the relay into an
/// open proxy (and `com`, `fr`, … are one entry away from the same mistake). A
/// single-label name is never a calendar host.
fn is_host_suffix(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.bytes().any(|b| !b.is_ascii()) {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

/// Parses a comma-separated list of exact origins (`scheme://host[:port]`).
fn parse_origins(raw: &str) -> Result<Vec<String>, ConfigError> {
    let mut origins: Vec<String> = Vec::new();
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let parsed = Url::parse(entry).map_err(|_| ConfigError::AllowedOrigins)?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(ConfigError::AllowedOrigins);
        }
        if parsed.host_str().is_none() || parsed.cannot_be_a_base() {
            return Err(ConfigError::AllowedOrigins);
        }
        if parsed.username() != "" || parsed.password().is_some() {
            return Err(ConfigError::AllowedOrigins);
        }
        if parsed.query().is_some() || parsed.fragment().is_some() || parsed.path() != "/" {
            return Err(ConfigError::AllowedOrigins);
        }
        let origin = parsed.origin().ascii_serialization();
        if origin == "null" || origin == "*" {
            return Err(ConfigError::AllowedOrigins);
        }
        if !origins.iter().any(|known| known == &origin) {
            origins.push(origin);
        }
    }
    Ok(origins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        Config::from_lookup(|key| map.get(key).cloned())
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let config = cfg(&[]).expect("defaults must be valid");
        assert_eq!(config.bind.to_string(), "127.0.0.1:8080");
        assert_eq!(config.allowed_hosts, vec!["api.skello.io".to_string()]);
        assert_eq!(config.max_bytes, 2_097_152);
        assert_eq!(config.timeout, Duration::from_secs(12));
        assert_eq!(config.rate_limit_per_minute, 30);
        assert_eq!(config.rate_burst, 10);
        assert!(!config.allow_extension);
        assert!(
            !config.allow_file_origin,
            "`Origin: null` must be refused unless CRENO_ALLOW_FILE_ORIGIN=1"
        );
        assert!(config.extra_origins.is_empty());
    }

    #[test]
    fn default_matches_config_default() {
        assert_eq!(cfg(&[]).expect("defaults"), Config::default());
    }

    #[test]
    fn empty_values_fall_back_to_defaults() {
        let config = cfg(&[
            (ENV_BIND, "   "),
            (ENV_ALLOWED_HOSTS, ""),
            (ENV_MAX_BYTES, ""),
            (ENV_ALLOW_EXTENSION, ""),
        ])
        .expect("blank values are treated as unset");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn explicit_values_are_parsed() {
        let config = cfg(&[
            (ENV_BIND, "0.0.0.0:9000"),
            (ENV_ALLOWED_HOSTS, "example.com"),
            (ENV_MAX_BYTES, "4096"),
            (ENV_TIMEOUT_SECS, "3"),
            (ENV_RATE_LIMIT, "7"),
            (ENV_ALLOW_EXTENSION, "1"),
            (ENV_ALLOW_FILE_ORIGIN, "1"),
            (ENV_ALLOWED_ORIGINS, "https://app.example.fr"),
        ])
        .expect("valid configuration");
        assert_eq!(config.bind.to_string(), "0.0.0.0:9000");
        assert_eq!(config.allowed_hosts, vec!["example.com".to_string()]);
        assert_eq!(config.max_bytes, 4096);
        assert_eq!(config.timeout, Duration::from_secs(3));
        assert_eq!(config.rate_limit_per_minute, 7);
        assert!(config.allow_extension);
        assert!(config.allow_file_origin);
        assert_eq!(config.extra_origins, vec!["https://app.example.fr"]);
    }

    #[test]
    fn host_list_is_normalised() {
        let config = cfg(&[(
            ENV_ALLOWED_HOSTS,
            " API.Skello.IO , .example.com,,api.skello.io ",
        )])
        .expect("valid host list");
        assert_eq!(
            config.allowed_hosts,
            vec!["api.skello.io".to_string(), "example.com".to_string()]
        );
    }

    #[test]
    fn bad_host_lists_are_rejected() {
        for raw in [
            ",",
            " , ",
            "*",
            "*.example.com",
            "https://api.example.com",
            "api.example.com:443",
            "api.example.com/path",
            "exa mple.com",
            "-bad.example.com",
            "bad-.example.com",
            "a..b",
            // A bare TLD would allowlist every public host under it, i.e. turn
            // the relay into an open proxy.
            "io",
            "com",
            "fr",
            "localhost",
            "example.com:443",
            ".io",
        ] {
            assert_eq!(
                cfg(&[(ENV_ALLOWED_HOSTS, raw)]),
                Err(ConfigError::AllowedHosts),
                "host list {raw:?} must be rejected"
            );
        }
    }

    #[test]
    fn bad_bind_is_rejected() {
        for raw in [
            "8080",
            "localhost:8080",
            "127.0.0.1",
            "127.0.0.1:0:1",
            "::1",
        ] {
            assert_eq!(
                cfg(&[(ENV_BIND, raw)]),
                Err(ConfigError::Bind),
                "bind {raw:?}"
            );
        }
        assert_eq!(
            cfg(&[(ENV_BIND, "[::1]:8080")])
                .expect("ipv6 bind")
                .bind
                .to_string(),
            "[::1]:8080"
        );
    }

    #[test]
    fn numeric_ranges_are_enforced() {
        assert_eq!(cfg(&[(ENV_MAX_BYTES, "0")]), Err(ConfigError::MaxBytes));
        assert_eq!(cfg(&[(ENV_MAX_BYTES, "1023")]), Err(ConfigError::MaxBytes));
        assert_eq!(
            cfg(&[(ENV_MAX_BYTES, "67108865")]),
            Err(ConfigError::MaxBytes)
        );
        assert_eq!(cfg(&[(ENV_MAX_BYTES, "2MiB")]), Err(ConfigError::MaxBytes));
        assert_eq!(cfg(&[(ENV_TIMEOUT_SECS, "0")]), Err(ConfigError::Timeout));
        assert_eq!(cfg(&[(ENV_TIMEOUT_SECS, "121")]), Err(ConfigError::Timeout));
        assert_eq!(cfg(&[(ENV_TIMEOUT_SECS, "1.5")]), Err(ConfigError::Timeout));
        assert_eq!(cfg(&[(ENV_RATE_LIMIT, "0")]), Err(ConfigError::RateLimit));
        assert_eq!(
            cfg(&[(ENV_RATE_LIMIT, "100001")]),
            Err(ConfigError::RateLimit)
        );
    }

    #[test]
    fn two_label_entries_are_the_shortest_accepted_suffix() {
        // The boundary, stated once: one label is refused, two are accepted.
        assert_eq!(
            cfg(&[(ENV_ALLOWED_HOSTS, "io")]),
            Err(ConfigError::AllowedHosts)
        );
        let config = cfg(&[(ENV_ALLOWED_HOSTS, "skello.io")]).expect("two labels");
        assert_eq!(config.allowed_hosts, vec!["skello.io".to_string()]);
        // Deep names stay fine.
        let config = cfg(&[(ENV_ALLOWED_HOSTS, "eu.api.skello.io")]).expect("three labels");
        assert_eq!(config.allowed_hosts, vec!["eu.api.skello.io".to_string()]);
        // And the failure names the variable only, never the value.
        let message = ConfigError::AllowedHosts.to_string();
        assert!(message.contains(ENV_ALLOWED_HOSTS));
        assert!(!message.contains("io"));
    }

    #[test]
    fn file_origin_flag_is_opt_in() {
        for raw in ["1", "true", "yes", "on"] {
            assert!(
                cfg(&[(ENV_ALLOW_FILE_ORIGIN, raw)])
                    .expect("valid")
                    .allow_file_origin,
                "{raw} should enable the null origin"
            );
        }
        for raw in ["0", "false", "no", "off"] {
            assert!(
                !cfg(&[(ENV_ALLOW_FILE_ORIGIN, raw)])
                    .expect("valid")
                    .allow_file_origin,
                "{raw} should keep the null origin refused"
            );
        }
        assert_eq!(
            cfg(&[(ENV_ALLOW_FILE_ORIGIN, "maybe")]),
            Err(ConfigError::AllowFileOrigin)
        );
        assert_eq!(
            ConfigError::AllowFileOrigin.to_string(),
            "CRENO_ALLOW_FILE_ORIGIN must be 0 or 1"
        );
    }

    #[test]
    fn boolean_flag_accepts_common_spellings() {
        for raw in ["1", "true", "TRUE", "yes", "on"] {
            assert!(
                cfg(&[(ENV_ALLOW_EXTENSION, raw)])
                    .expect("valid")
                    .allow_extension
            );
        }
        for raw in ["0", "false", "no", "off"] {
            assert!(
                !cfg(&[(ENV_ALLOW_EXTENSION, raw)])
                    .expect("valid")
                    .allow_extension
            );
        }
        assert_eq!(
            cfg(&[(ENV_ALLOW_EXTENSION, "2")]),
            Err(ConfigError::AllowExtension)
        );
        assert_eq!(
            cfg(&[(ENV_ALLOW_EXTENSION, "maybe")]),
            Err(ConfigError::AllowExtension)
        );
    }

    #[test]
    fn extra_origins_are_normalised_and_validated() {
        let config = cfg(&[(
            ENV_ALLOWED_ORIGINS,
            " https://App.Example.fr:8443/ , http://127.0.0.1:1234,https://app.example.fr:8443",
        )])
        .expect("valid origins");
        assert_eq!(
            config.extra_origins,
            vec![
                "https://app.example.fr:8443".to_string(),
                "http://127.0.0.1:1234".to_string(),
            ]
        );

        for raw in [
            "*",
            "null",
            "ftp://x.fr",
            "https://x.fr/path",
            "https://x.fr?a=1",
            "x.fr",
        ] {
            assert_eq!(
                cfg(&[(ENV_ALLOWED_ORIGINS, raw)]),
                Err(ConfigError::AllowedOrigins),
                "origin {raw:?} must be rejected"
            );
        }
    }

    #[test]
    fn error_messages_never_contain_the_value() {
        let secret = "https://internal.example.com:8443/secret-token";
        let error = cfg(&[(ENV_ALLOWED_HOSTS, secret)]).expect_err("must be rejected");
        let message = error.to_string();
        assert!(!message.contains("secret-token"));
        assert!(!message.contains("8443"));
    }
}
