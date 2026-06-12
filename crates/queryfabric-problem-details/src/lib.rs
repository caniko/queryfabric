//! RFC 7807 Problem Details types for HTTP APIs.
//!
//! Provides [`ProblemDetails`], the canonical `application/problem+json`
//! body, plus the [`REQUEST_ID_HEADER`] constant for request correlation.

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// Header used to correlate UI and API requests in logs.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Content type used by RFC 7807 API error responses.
pub const PROBLEM_JSON_CONTENT_TYPE: &str = "application/problem+json";

/// RFC 7807 Problem Details response body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProblemDetails {
    /// Problem type URI, serialized as the RFC 7807 `type` field.
    #[serde(rename = "type")]
    pub type_url: String,
    /// Short, human-readable summary of the problem.
    pub title: String,
    /// Detailed explanation specific to this occurrence of the problem.
    pub detail: String,
    /// HTTP status code associated with the problem.
    pub status: u16,
}

impl ProblemDetails {
    /// Build a problem-details body using the standard `about:blank` type URI.
    #[must_use]
    pub fn new(title: impl Into<String>, detail: impl Into<String>, status: u16) -> Self {
        Self {
            type_url: "about:blank".to_owned(),
            title: title.into(),
            detail: detail.into(),
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_details_serializes_rfc7807_type_field() {
        let body = ProblemDetails::new("Bad Request", "bad input", 400);
        let value = serde_json::to_value(body).unwrap();
        assert_eq!(value["type"], "about:blank");
        assert_eq!(value["title"], "Bad Request");
        assert_eq!(value["detail"], "bad input");
        assert_eq!(value["status"], 400);
    }
}
