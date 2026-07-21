use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Validated email address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "String", into = "String")]
pub struct Email(String);

/// Error returned when constructing an [`Email`] from invalid input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EmailParseError {
    /// The provided email string failed basic validation.
    #[error("invalid email address: {value}")]
    Invalid {
        /// Original invalid email string.
        value: String,
    },
}

impl Email {
    /// Create a new `Email`, validating basic format (contains `@`, min length 3).
    ///
    /// # Errors
    /// Returns [`EmailParseError::Invalid`] when the string does not pass the
    /// crate's lightweight validation.
    pub fn new(email: impl Into<String>) -> Result<Self, EmailParseError> {
        let s = email.into();
        if s.len() >= 3 && s.contains('@') && !s.starts_with('@') && !s.ends_with('@') {
            Ok(Self(s))
        } else {
            Err(EmailParseError::Invalid { value: s })
        }
    }

    /// Create an `Email` without validation.
    #[must_use]
    pub fn new_unchecked(email: impl Into<String>) -> Self {
        Self(email.into())
    }

    /// Access the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner `String`.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Email {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Email {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Email {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl TryFrom<String> for Email {
    type Error = EmailParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<Email> for String {
    fn from(value: Email) -> String {
        value.0
    }
}

/// Type of user account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserType {
    /// Regular user attached to a person.
    #[default]
    Human,
    /// Automated service account (ETL, CI, etc.).
    Service,
}

/// Error returned when parsing a user type fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UserTypeParseError {
    /// The provided user-type string was not recognized.
    #[error("invalid user type: {value} (expected 'human' or 'service')")]
    Invalid {
        /// Original invalid user-type string.
        value: String,
    },
}

impl UserType {
    /// Return the stable snake_case representation used in tokens.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Service => "service",
        }
    }

    /// Return `true` when this user type represents an automated service account.
    #[must_use]
    pub fn is_service(self) -> bool {
        matches!(self, Self::Service)
    }
}

impl fmt::Display for UserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for UserType {
    type Err = UserTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" => Ok(Self::Human),
            "service" => Ok(Self::Service),
            _ => Err(UserTypeParseError::Invalid {
                value: s.to_owned(),
            }),
        }
    }
}

/// Errors that can occur during PASETO token validation.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The configured secret is shorter than the required 32 bytes.
    #[error("Secret key must be at least 32 bytes")]
    SecretTooShort,

    /// Token parsing or building failed.
    #[error("Token parsing failed: {0}")]
    TokenParse(String),

    /// The token was missing its `sub` claim.
    #[error("Missing 'sub' claim")]
    MissingSub,

    /// The token was missing its `email` claim.
    #[error("Missing 'email' claim")]
    MissingEmail,

    /// The token's email claim was invalid.
    #[error("Invalid email in token: {0}")]
    InvalidEmail(String),

    /// The token's `sub` claim was not a valid UUID.
    #[error("Invalid UUID in 'sub' claim: {0}")]
    InvalidUuid(#[from] uuid::Error),

    /// The token carried an unexpected scope.
    #[error("Invalid scope: expected '{expected}', got '{actual}'")]
    InvalidScope {
        /// Expected scope value.
        expected: String,
        /// Actual scope value found in the token.
        actual: String,
    },

    /// A required delegation-token claim was missing.
    #[error("Missing '{0}' claim in delegation token")]
    MissingDelegationClaim(String),

    /// A delegation-token claim failed validation.
    #[error("Invalid delegation claim '{field}': {reason}")]
    InvalidDelegationClaim {
        /// Claim name.
        field: String,
        /// Human-readable validation failure.
        reason: String,
    },
}

impl From<crate::AuthTokenError> for AuthError {
    fn from(e: crate::AuthTokenError) -> Self {
        match e {
            crate::AuthTokenError::SecretTooShort => Self::SecretTooShort,
            crate::AuthTokenError::TokenParse(s) | crate::AuthTokenError::TokenBuild(s) => {
                Self::TokenParse(s)
            }
        }
    }
}

/// Authenticated user extracted from a PASETO token.
///
/// Shared between the Axum API and Arrow Flight services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    /// User id from the token `sub` claim.
    pub id: Uuid,
    /// Validated email address from the token.
    pub email: Email,
    /// Whether the account is active.
    pub is_active: bool,
    /// Whether the account is a superuser.
    pub is_superuser: bool,
    /// Whether the account's email is verified.
    pub is_verified: bool,
    /// Account type.
    #[serde(default)]
    pub user_type: UserType,
    /// Arbitrary authorization roles carried by the token.
    #[serde(default)]
    pub roles: Vec<String>,
}

impl AuthUser {
    /// Whether this is a service account (ETL, CI, etc.).
    #[must_use]
    pub fn is_service(&self) -> bool {
        self.user_type.is_service()
    }

    /// Whether the token carries the named role.
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|candidate| candidate == role)
    }
}

/// Validate a PASETO v4.local token and extract user claims.
///
/// # Errors
/// Returns [`AuthError`] when the token cannot be parsed or does not contain
/// the claims needed to construct an [`AuthUser`].
pub fn validate_paseto_token(token: &str, secret: &str) -> Result<AuthUser, AuthError> {
    let claims = crate::parse_paseto_v4_local(token, secret)?;

    let id_str = claims["sub"].as_str().ok_or_else(|| {
        tracing::warn!("PASETO token missing 'sub' claim");
        AuthError::MissingSub
    })?;
    let id = Uuid::parse_str(id_str).map_err(|e| {
        tracing::warn!("PASETO token has invalid UUID in 'sub' claim");
        AuthError::InvalidUuid(e)
    })?;

    let email_str = claims["email"].as_str().ok_or_else(|| {
        tracing::warn!("PASETO token missing 'email' claim");
        AuthError::MissingEmail
    })?;
    let email = Email::new_unchecked(email_str);

    let is_active = claims["is_active"]
        .as_str()
        .map(|s| s == "true")
        .or_else(|| claims["is_active"].as_bool())
        .unwrap_or(true);
    let is_superuser = claims["is_superuser"]
        .as_str()
        .map(|s| s == "true")
        .or_else(|| claims["is_superuser"].as_bool())
        .unwrap_or(false);
    let is_verified = claims["is_verified"]
        .as_str()
        .map(|s| s == "true")
        .or_else(|| claims["is_verified"].as_bool())
        .unwrap_or(false);
    let user_type = claims["user_type"]
        .as_str()
        .and_then(|s| s.parse::<UserType>().ok())
        .unwrap_or_default();
    let roles = roles_from_claim(&claims["roles"]);

    tracing::debug!(%user_type, role_count = roles.len(), "PASETO token validated");

    Ok(AuthUser {
        id,
        email,
        is_active,
        is_superuser,
        is_verified,
        user_type,
        roles,
    })
}

fn roles_from_claim(value: &serde_json::Value) -> Vec<String> {
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect();
    }
    value
        .as_str()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Scoped delegation tokens for inter-node data access
// ---------------------------------------------------------------------------

/// TTL for delegation tokens (seconds).
const DELEGATION_TTL_SECS: i64 = 30;

/// Claims extracted from a validated delegation token.
#[derive(Debug, Clone)]
pub struct DelegationClaims {
    /// User ID from the `sub` claim.
    pub sub: Uuid,
    /// User email.
    pub email: Email,
    /// Dataset IDs the delegation authorizes access to.
    pub dataset_ids: Vec<Uuid>,
    /// QueryFabric table discriminant the delegation authorizes.
    pub table_id: i32,
}

/// Mint a short-lived PASETO v4.local delegation token.
///
/// # Errors
/// Returns [`AuthError`] when the secret is invalid or the delegation claims
/// cannot be encoded into a token.
pub fn mint_delegation_token(
    user: &AuthUser,
    dataset_ids: &[Uuid],
    table_id: i32,
    secret: &str,
) -> Result<String, AuthError> {
    use rusty_paseto::prelude::*;

    let key = crate::v4_local_key(secret)?;

    let expiry = chrono::Utc::now() + chrono::Duration::seconds(DELEGATION_TTL_SECS);
    let expiry_str = expiry.to_rfc3339();
    let user_id_str = user.id.to_string();

    let dataset_ids_json = serde_json::to_string(
        &dataset_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
    )
    .map_err(|e| AuthError::InvalidDelegationClaim {
        field: "dataset_ids".to_owned(),
        reason: e.to_string(),
    })?;

    let table_id_str = table_id.to_string();

    let token = PasetoBuilder::<V4, Local>::default()
        .set_claim(SubjectClaim::from(user_id_str.as_str()))
        .set_claim(ExpirationClaim::try_from(expiry_str.as_str()).map_err(|e| {
            AuthError::TokenParse(format!(
                "build delegation expiry claim from RFC3339 timestamp {expiry_str:?}: {e}"
            ))
        })?)
        .set_claim(
            CustomClaim::try_from(("email", user.email.as_str())).map_err(|e| {
                AuthError::TokenParse(format!(
                    "build delegation claim 'email' from authenticated user {}: {e}",
                    user.email
                ))
            })?,
        )
        .set_claim(CustomClaim::try_from(("scope", "delegation")).map_err(|e| {
            AuthError::TokenParse(format!(
                "build delegation claim 'scope' with value \"delegation\": {e}"
            ))
        })?)
        .set_claim(
            CustomClaim::try_from(("dataset_ids", dataset_ids_json.as_str())).map_err(|e| {
                AuthError::TokenParse(format!(
                    "build delegation claim 'dataset_ids' for {} dataset ids: {e}",
                    dataset_ids.len()
                ))
            })?,
        )
        .set_claim(
            CustomClaim::try_from(("table_id", table_id_str.as_str())).map_err(|e| {
                AuthError::TokenParse(format!(
                    "build delegation claim 'table_id' from table discriminant {table_id}: {e}"
                ))
            })?,
        )
        .build(&key)
        .map_err(|e| {
            AuthError::TokenParse(format!(
                "build delegation PASETO token for user {} and {} dataset ids: {e}",
                user.id,
                dataset_ids.len()
            ))
        })?;

    tracing::debug!(
        dataset_count = dataset_ids.len(),
        table_id,
        "Delegation token minted"
    );

    Ok(token)
}

/// Validate a delegation token and extract its claims.
///
/// # Errors
/// Returns [`AuthError`] when the token cannot be parsed, has the wrong
/// scope, or contains invalid delegation claims.
pub fn validate_delegation_token(token: &str, secret: &str) -> Result<DelegationClaims, AuthError> {
    let claims = crate::parse_paseto_v4_local(token, secret)?;

    let scope = claims["scope"]
        .as_str()
        .ok_or_else(|| AuthError::MissingDelegationClaim("scope".to_owned()))?;
    if scope != "delegation" {
        return Err(AuthError::InvalidScope {
            expected: "delegation".to_owned(),
            actual: scope.to_owned(),
        });
    }

    let id_str = claims["sub"].as_str().ok_or(AuthError::MissingSub)?;
    let sub = Uuid::parse_str(id_str)?;

    let email_str = claims["email"].as_str().ok_or(AuthError::MissingEmail)?;
    let email = Email::new_unchecked(email_str);

    let dataset_ids_str = claims["dataset_ids"]
        .as_str()
        .ok_or_else(|| AuthError::MissingDelegationClaim("dataset_ids".to_owned()))?;
    let dataset_id_strings: Vec<String> =
        serde_json::from_str(dataset_ids_str).map_err(|e| AuthError::InvalidDelegationClaim {
            field: "dataset_ids".to_owned(),
            reason: e.to_string(),
        })?;
    let dataset_ids: Vec<Uuid> = dataset_id_strings
        .iter()
        .map(|s| Uuid::parse_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AuthError::InvalidDelegationClaim {
            field: "dataset_ids".to_owned(),
            reason: e.to_string(),
        })?;

    let table_id_str = claims["table_id"]
        .as_str()
        .ok_or_else(|| AuthError::MissingDelegationClaim("table_id".to_owned()))?;
    let table_id: i32 = table_id_str.parse().map_err(|e: std::num::ParseIntError| {
        AuthError::InvalidDelegationClaim {
            field: "table_id".to_owned(),
            reason: e.to_string(),
        }
    })?;

    tracing::debug!(
        dataset_count = dataset_ids.len(),
        table_id,
        "Delegation token validated"
    );

    Ok(DelegationClaims {
        sub,
        email,
        dataset_ids,
        table_id,
    })
}

#[cfg(test)]
mod tests;
