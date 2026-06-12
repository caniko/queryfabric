//! Shared HTTP client construction with optional bearer-token auth.

#![warn(missing_docs)]

use miette::Result;
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

/// Build a pre-configured `reqwest::Client` with the given user agent.
///
/// # Errors
/// Returns an error when `reqwest` fails to build the client.
pub fn client(user_agent: &str) -> Result<Client> {
    Client::builder()
        .user_agent(user_agent.to_owned())
        .build()
        .map_err(|e| miette::miette!("build reqwest client for user-agent {user_agent:?}: {e}"))
}

/// Build a `reqwest::Client` with a Bearer token in the default headers.
///
/// # Errors
/// Returns an error when the token cannot be encoded into an HTTP header or
/// when `reqwest` fails to build the client.
pub fn auth_client(user_agent: &str, token: &str) -> Result<Client> {
    let mut headers = HeaderMap::new();
    let val = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|e| {
            miette::miette!(
                "build Authorization header from bearer token: {e}; remove control characters from the token"
            )
        })?;
    headers.insert(AUTHORIZATION, val);

    Client::builder()
        .user_agent(user_agent.to_owned())
        .default_headers(headers)
        .build()
        .map_err(|e| {
            miette::miette!("build authenticated reqwest client for user-agent {user_agent:?}: {e}")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_builds() {
        assert!(client("test-cli").is_ok());
    }

    #[test]
    fn auth_client_builds_with_valid_token() {
        assert!(auth_client("test-cli", "test-token-123").is_ok());
    }

    #[test]
    fn auth_client_rejects_invalid_header_chars() {
        assert!(auth_client("test-cli", "token\nwith\nnewlines").is_err());
    }

    #[test]
    fn auth_client_works_with_empty_token() {
        assert!(auth_client("test-cli", "").is_ok());
    }
}
