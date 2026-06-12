//! PASETO v4.local token primitives.
//!
//! Provides the lower layer of an auth stack: extract a bearer token from an
//! `Authorization` header, validate the secret length, parse PASETO claims
//! into a JSON value, and build a v4.local symmetric key from a secret.

#![warn(missing_docs)]

use thiserror::Error;

pub use rusty_paseto;

mod typed;

pub use typed::{
    AuthError, AuthUser, DelegationClaims, Email, EmailParseError, UserType, UserTypeParseError,
    mint_delegation_token, validate_delegation_token, validate_paseto_token,
};

/// Errors that can occur during PASETO token validation.
#[derive(Debug, Error)]
pub enum AuthTokenError {
    /// The configured secret is shorter than the required 32 bytes.
    #[error("secret key must be at least 32 bytes")]
    SecretTooShort,

    /// Token parsing failed.
    #[error("token parsing failed: {0}")]
    TokenParse(String),

    /// Token construction failed.
    #[error("token build failed: {0}")]
    TokenBuild(String),
}

/// Extract a bearer token from an `Authorization` header value.
///
/// Accepts both `Bearer <token>` and legacy `Token <token>` (case-insensitive).
#[must_use]
pub fn extract_bearer_token(header: &str) -> Option<&str> {
    if header.len() >= 7 && header[..7].eq_ignore_ascii_case("bearer ") {
        Some(&header[7..])
    } else if header.len() >= 6 && header[..6].eq_ignore_ascii_case("token ") {
        Some(&header[6..])
    } else {
        None
    }
}

/// Validate that a PASETO secret is at least 32 bytes.
///
/// # Errors
/// Returns [`AuthTokenError::SecretTooShort`] when `secret` has fewer than
/// 32 bytes.
pub fn validate_paseto_secret(secret: &str) -> Result<(), AuthTokenError> {
    if secret.len() < 32 {
        return Err(AuthTokenError::SecretTooShort);
    }
    Ok(())
}

/// Build a v4.local symmetric key from the first 32 bytes of `secret`.
///
/// Used by both [`parse_paseto_v4_local`] and any caller that needs to
/// construct their own `PasetoBuilder`/`PasetoParser`.
///
/// # Errors
/// Returns [`AuthTokenError::SecretTooShort`] when `secret` has fewer than
/// 32 bytes.
pub fn v4_local_key(
    secret: &str,
) -> Result<
    rusty_paseto::prelude::PasetoSymmetricKey<
        rusty_paseto::prelude::V4,
        rusty_paseto::prelude::Local,
    >,
    AuthTokenError,
> {
    use rusty_paseto::prelude::*;

    let bytes = secret
        .as_bytes()
        .first_chunk::<32>()
        .ok_or(AuthTokenError::SecretTooShort)?;
    Ok(PasetoSymmetricKey::<V4, Local>::from(Key::from(bytes)))
}

/// Parse a PASETO v4.local token and return the raw claims as a JSON value.
///
/// Callers build their typed user/auth records from the returned `Value`.
///
/// # Errors
/// Returns [`AuthTokenError::SecretTooShort`] for short secrets and
/// [`AuthTokenError::TokenParse`] when the token cannot be parsed.
pub fn parse_paseto_v4_local(
    token: &str,
    secret: &str,
) -> Result<serde_json::Value, AuthTokenError> {
    use rusty_paseto::prelude::*;

    let key = v4_local_key(secret)?;

    PasetoParser::<V4, Local>::default()
        .parse(token, &key)
        .map_err(|e| {
            tracing::warn!("PASETO token parse failed: {e}");
            AuthTokenError::TokenParse(e.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_extraction() {
        assert_eq!(extract_bearer_token("Bearer abc"), Some("abc"));
        assert_eq!(extract_bearer_token("bearer abc"), Some("abc"));
        assert_eq!(extract_bearer_token("BEARER abc"), Some("abc"));
        assert_eq!(extract_bearer_token("Token xyz"), Some("xyz"));
        assert_eq!(extract_bearer_token("token xyz"), Some("xyz"));
        assert_eq!(extract_bearer_token("Basic abc"), None);
        assert_eq!(extract_bearer_token(""), None);
    }

    #[test]
    fn secret_length_validation() {
        assert!(validate_paseto_secret("").is_err());
        assert!(validate_paseto_secret("short").is_err());
        assert!(validate_paseto_secret(&"a".repeat(31)).is_err());
        assert!(validate_paseto_secret(&"a".repeat(32)).is_ok());
        assert!(validate_paseto_secret(&"a".repeat(64)).is_ok());
    }
}
