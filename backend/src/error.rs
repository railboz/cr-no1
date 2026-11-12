//! Relay error type.
//!
//! One variant per wire code of the contract. The response body is always
//! `{"error":"<code>"}`: upstream details, request URLs and query strings are
//! never echoed back to the client.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Content type used by every error response.
pub const ERROR_CONTENT_TYPE: &str = "application/json; charset=utf-8";

/// Everything that can go wrong while serving `/ics` (plus the framework-level
/// 405). Each variant maps to exactly one machine-readable code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RelayError {
    /// The `url` query parameter is absent or empty.
    #[error("missing_url")]
    MissingUrl,
    /// The `url` parameter is not a usable absolute URL, or exceeds 2048 bytes.
    #[error("invalid_url")]
    InvalidUrl,
    /// The target scheme/host is not allowlisted (or resolves to a private IP).
    #[error("forbidden_host")]
    ForbiddenHost,
    /// The upstream fetch failed.
    #[error("upstream_error")]
    UpstreamError,
    /// The upstream body exceeded `CRENO_MAX_BYTES`.
    #[error("upstream_too_large")]
    UpstreamTooLarge,
    /// The upstream fetch exceeded `CRENO_TIMEOUT_SECS`.
    #[error("timeout")]
    Timeout,
    /// The per-IP budget is exhausted; `retry_after_secs` feeds `Retry-After`.
    #[error("rate_limited")]
    RateLimited {
        /// Seconds to wait before retrying (always at least 1).
        retry_after_secs: u64,
    },
    /// The route exists but not for this method.
    #[error("method_not_allowed")]
    MethodNotAllowed,
}

impl RelayError {
    /// Machine-readable code sent in the JSON body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingUrl => "missing_url",
            Self::InvalidUrl => "invalid_url",
            Self::ForbiddenHost => "forbidden_host",
            Self::UpstreamError => "upstream_error",
            Self::UpstreamTooLarge => "upstream_too_large",
            Self::Timeout => "timeout",
            Self::RateLimited { .. } => "rate_limited",
            Self::MethodNotAllowed => "method_not_allowed",
        }
    }

    /// HTTP status sent with the code.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::MissingUrl | Self::InvalidUrl => StatusCode::BAD_REQUEST,
            Self::ForbiddenHost => StatusCode::FORBIDDEN,
            Self::UpstreamError => StatusCode::BAD_GATEWAY,
            Self::UpstreamTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        }
    }

    /// Builds the wire response: status, JSON code, `Retry-After` when relevant.
    /// Error responses are never cached.
    pub fn into_response_parts(self) -> Response {
        let status = self.status();
        let body = format!("{{\"error\":\"{}\"}}", self.code());

        let mut response = (
            status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(ERROR_CONTENT_TYPE),
            )],
            body,
        )
            .into_response();

        let headers = response.headers_mut();
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );

        if let Self::RateLimited { retry_after_secs } = &self {
            let secs = (*retry_after_secs).max(1).to_string();
            if let Ok(value) = HeaderValue::from_str(&secs) {
                headers.insert(header::RETRY_AFTER, value);
            }
        }

        if matches!(self, Self::MethodNotAllowed) {
            headers.insert(header::ALLOW, HeaderValue::from_static("GET, OPTIONS"));
        }

        response
    }
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        self.into_response_parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn body_of(error: RelayError) -> (StatusCode, axum::http::HeaderMap, String) {
        let response = error.into_response();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body collects")
            .to_bytes();
        (
            status,
            headers,
            String::from_utf8_lossy(&bytes).into_owned(),
        )
    }

    #[test]
    fn codes_and_statuses_match_the_contract() {
        let cases = [
            (RelayError::MissingUrl, 400, "missing_url"),
            (RelayError::InvalidUrl, 400, "invalid_url"),
            (RelayError::ForbiddenHost, 403, "forbidden_host"),
            (RelayError::UpstreamError, 502, "upstream_error"),
            (RelayError::UpstreamTooLarge, 413, "upstream_too_large"),
            (RelayError::Timeout, 504, "timeout"),
            (
                RelayError::RateLimited {
                    retry_after_secs: 7,
                },
                429,
                "rate_limited",
            ),
            (RelayError::MethodNotAllowed, 405, "method_not_allowed"),
        ];
        for (error, status, code) in cases {
            assert_eq!(error.status().as_u16(), status, "{code}");
            assert_eq!(error.code(), code);
        }
    }

    #[tokio::test]
    async fn every_error_is_a_json_code() {
        let (status, headers, body) = body_of(RelayError::MissingUrl).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some(ERROR_CONTENT_TYPE)
        );
        assert_eq!(body, "{\"error\":\"missing_url\"}");
        assert_eq!(
            headers
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );
        assert!(headers.get(header::RETRY_AFTER).is_none());
    }

    #[tokio::test]
    async fn rate_limited_carries_retry_after() {
        let (status, headers, body) = body_of(RelayError::RateLimited {
            retry_after_secs: 42,
        })
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            headers
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok()),
            Some("42")
        );
        assert_eq!(body, "{\"error\":\"rate_limited\"}");
    }

    #[tokio::test]
    async fn method_not_allowed_advertises_allowed_methods() {
        let (status, headers, body) = body_of(RelayError::MethodNotAllowed).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            headers.get(header::ALLOW).and_then(|v| v.to_str().ok()),
            Some("GET, OPTIONS")
        );
        assert_eq!(body, "{\"error\":\"method_not_allowed\"}");
    }
}
