//! Behaviour of the SSRF guard, through its public API only.
//!
//! Every assertion here is hermetic: the cases fail before resolution (shape,
//! host form, allowlist — the guard resolves only after those checks), except
//! `localhost`, whose answer comes from the local host file and is refused
//! either way. No test in this file reaches DNS or the network, and none needs
//! a real calendar host.
//!
//! The resolver seam (`validate_with_resolver`) is `pub(crate)`, so the
//! synthetic-answer matrix — loopback, RFC 1918, `169.254.169.254`, CGNAT,
//! documentation blocks, mixed public/private round-robins — lives in the unit
//! tests of `src/guard.rs`, next to the code it drives.

use creno_relay::config::Config;
use creno_relay::error::RelayError;
use creno_relay::guard::{HTTPS_PORT, MAX_TARGET_LEN, validate_target};

use RelayError::{ForbiddenHost, InvalidUrl, MissingUrl};

fn cfg(hosts: &[&str]) -> Config {
    Config {
        allowed_hosts: hosts.iter().map(|host| (*host).to_string()).collect(),
        ..Config::default()
    }
}

/// One row of the guard's decision table: input, expected code, reason.
struct Case {
    url: &'static str,
    expected: RelayError,
    why: &'static str,
}

/// The default allowlist (`api.skello.io`) is used for every row.
fn cases() -> Vec<Case> {
    vec![
        Case {
            url: "",
            expected: MissingUrl,
            why: "empty parameter",
        },
        Case {
            url: "not a url",
            expected: InvalidUrl,
            why: "not absolute",
        },
        Case {
            url: "api.skello.io/ics",
            expected: InvalidUrl,
            why: "no scheme",
        },
        Case {
            url: "http://api.skello.io/ics",
            expected: ForbiddenHost,
            why: "scheme must be https",
        },
        Case {
            url: "ftp://api.skello.io/ics",
            expected: ForbiddenHost,
            why: "scheme must be https",
        },
        Case {
            url: "file:///etc/passwd",
            expected: ForbiddenHost,
            why: "scheme must be https",
        },
        Case {
            url: "gopher://api.skello.io:70/_x",
            expected: ForbiddenHost,
            why: "scheme must be https",
        },
        Case {
            url: "data:text/calendar,BEGIN:VCALENDAR",
            expected: ForbiddenHost,
            why: "scheme must be https",
        },
        Case {
            url: "https://user:pw@api.skello.io/ics",
            expected: InvalidUrl,
            why: "userinfo",
        },
        Case {
            url: "https://api.skello.io@evil.test/ics",
            expected: InvalidUrl,
            why: "userinfo hides the real host",
        },
        Case {
            url: "https://api.skello.io/ics#token",
            expected: InvalidUrl,
            why: "fragment",
        },
        Case {
            url: "https://api.skello.io:8443/ics",
            expected: InvalidUrl,
            why: "non-443 port",
        },
        Case {
            url: "https://api.skello.io:80/ics",
            expected: InvalidUrl,
            why: "non-443 port",
        },
        Case {
            url: "https://127.0.0.1/ics",
            expected: ForbiddenHost,
            why: "IPv4 literal",
        },
        Case {
            url: "https://[::1]/ics",
            expected: ForbiddenHost,
            why: "IPv6 literal",
        },
        Case {
            url: "https://[::ffff:127.0.0.1]/ics",
            expected: ForbiddenHost,
            why: "IPv4-mapped IPv6 literal",
        },
        Case {
            url: "https://2130706433/ics",
            expected: ForbiddenHost,
            why: "decimal IPv4 literal",
        },
        Case {
            url: "https://0x7f000001/ics",
            expected: ForbiddenHost,
            why: "hexadecimal IPv4 literal",
        },
        Case {
            url: "https://169.254.169.254/latest/meta-data/",
            expected: ForbiddenHost,
            why: "cloud metadata address",
        },
        Case {
            url: "https://skello.io/ics",
            expected: ForbiddenHost,
            why: "parent domain is not allowlisted",
        },
        Case {
            url: "https://skello.io.evil.com/ics",
            expected: ForbiddenHost,
            why: "allowlist is a label suffix, not a substring",
        },
        Case {
            url: "https://evil-api.skello.io/ics",
            expected: ForbiddenHost,
            why: "label boundary, not a bare prefix",
        },
        Case {
            url: "https://apiskello.io/ics",
            expected: ForbiddenHost,
            why: "label boundary, not a concatenation",
        },
        Case {
            url: "https://localhost/ics",
            expected: ForbiddenHost,
            why: "resolves to loopback (fails closed even if it does not resolve)",
        },
    ]
}

#[tokio::test]
async fn decision_table_is_the_documented_one() {
    let config = cfg(&["api.skello.io"]);
    for case in cases() {
        let error = validate_target(case.url, &config)
            .await
            .expect_err("every row of the table is refused");
        assert_eq!(
            error,
            case.expected,
            "{:?} should be {} ({})",
            case.url,
            case.expected.code(),
            case.why
        );
    }
    assert!(cases().len() >= 20, "the table must stay broad");
}

#[tokio::test]
async fn an_empty_allowlist_forbids_every_target() {
    let config = cfg(&[]);
    for url in [
        "https://api.skello.io/ics",
        "https://example.com/ics",
        "https://93.184.216.34/ics",
        "https://localhost/ics",
    ] {
        assert_eq!(
            validate_target(url, &config).await.err(),
            Some(ForbiddenHost),
            "{url} with an empty allowlist"
        );
    }
}

#[tokio::test]
async fn overlong_targets_are_invalid_at_the_contract_limit() {
    let config = cfg(&["api.skello.io"]);
    assert_eq!(MAX_TARGET_LEN, 2048);

    // Long enough to be refused before any parsing work.
    let overlong = format!("https://api.skello.io/{}", "a".repeat(MAX_TARGET_LEN));
    assert!(overlong.len() > MAX_TARGET_LEN);
    assert_eq!(
        validate_target(&overlong, &config).await.err(),
        Some(InvalidUrl)
    );

    // A long but in-limit target is not a shape error: it can only be refused
    // as a host (here by the allowlist), never as `invalid_url`.
    let in_limit = |suffix: &str| {
        let mut url = format!("https://evil.example/ics/{suffix}");
        while url.len() < MAX_TARGET_LEN - 1 {
            url.push('a');
        }
        url
    };
    let url = in_limit("");
    assert!(url.len() <= MAX_TARGET_LEN);
    assert_eq!(
        validate_target(&url, &config).await.err(),
        Some(ForbiddenHost)
    );
}

#[tokio::test]
async fn codes_and_statuses_are_the_contract_ones() {
    assert_eq!(HTTPS_PORT, 443);
    assert_eq!(MissingUrl.code(), "missing_url");
    assert_eq!(MissingUrl.status().as_u16(), 400);
    assert_eq!(InvalidUrl.code(), "invalid_url");
    assert_eq!(InvalidUrl.status().as_u16(), 400);
    assert_eq!(ForbiddenHost.code(), "forbidden_host");
    assert_eq!(ForbiddenHost.status().as_u16(), 403);
}
