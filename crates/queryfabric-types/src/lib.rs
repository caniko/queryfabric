use std::fmt;
use thiserror::Error;

// =============================================================================
// ValidationError
// =============================================================================

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid email: {0}")]
    InvalidEmail(String),
    #[error("Invalid country code: {0}")]
    InvalidCountryCode(String),
    #[error("Invalid DOI: {0}")]
    InvalidDoi(String),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Value must not be empty")]
    EmptyValue,
}

// =============================================================================
// Macro helpers
// =============================================================================

macro_rules! impl_newtype_common {
    ($name:ident) => {
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
            pub fn into_inner(self) -> String {
                self.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }
        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }
        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
        impl TryFrom<String> for $name {
            type Error = ValidationError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::new(s)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0
            }
        }
    };
}

// =============================================================================
// Email
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Email(String);

impl Email {
    pub fn new(email: impl Into<String>) -> Result<Self, ValidationError> {
        let s = email.into();
        if s.len() >= 3 && s.contains('@') && !s.starts_with('@') && !s.ends_with('@') {
            Ok(Self(s))
        } else {
            Err(ValidationError::InvalidEmail(s))
        }
    }

    pub fn new_unchecked(email: impl Into<String>) -> Self {
        Self(email.into())
    }
}
impl_newtype_common!(Email);

// =============================================================================
// CountryCode
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CountryCode(String);

impl CountryCode {
    pub fn new(code: impl Into<String>) -> Result<Self, ValidationError> {
        let s = code.into();
        if s.len() == 2 && s.chars().all(|c| c.is_ascii_uppercase()) {
            Ok(Self(s))
        } else {
            Err(ValidationError::InvalidCountryCode(s))
        }
    }

    pub fn new_unchecked(code: impl Into<String>) -> Self {
        Self(code.into())
    }
}
impl_newtype_common!(CountryCode);

// =============================================================================
// Doi
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Doi(String);

impl Doi {
    pub fn new(doi: impl Into<String>) -> Result<Self, ValidationError> {
        let s = doi.into();
        if s.starts_with("10.") && s.len() > 4 {
            Ok(Self(s))
        } else {
            Err(ValidationError::InvalidDoi(s))
        }
    }

    pub fn new_unchecked(doi: impl Into<String>) -> Self {
        Self(doi.into())
    }
}
impl_newtype_common!(Doi);

// =============================================================================
// ClusterName — non-empty string
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ClusterName(String);

impl ClusterName {
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let s = name.into();
        if s.is_empty() {
            Err(ValidationError::EmptyValue)
        } else {
            Ok(Self(s))
        }
    }
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}
impl_newtype_common!(ClusterName);

// =============================================================================
// DatabaseName — non-empty string
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DatabaseName(String);

impl DatabaseName {
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let s = name.into();
        if s.is_empty() {
            Err(ValidationError::EmptyValue)
        } else {
            Ok(Self(s))
        }
    }
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}
impl_newtype_common!(DatabaseName);

// =============================================================================
// UserType
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum UserType {
    #[default]
    Human,
    Service,
}

impl UserType {
    pub fn is_service(&self) -> bool {
        matches!(self, Self::Service)
    }
}

// =============================================================================
// OAuthProviderName
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OAuthProviderName {
    Github,
    Google,
    GitLab,
    Orcid,
    #[serde(rename = "cilogon")]
    CiLogon,
}

impl OAuthProviderName {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "github" => Some(Self::Github),
            "google" => Some(Self::Google),
            "gitlab" => Some(Self::GitLab),
            "orcid" => Some(Self::Orcid),
            "cilogon" => Some(Self::CiLogon),
            _ => None,
        }
    }
}

impl std::str::FromStr for OAuthProviderName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| format!("Unknown OAuth provider: {s}"))
    }
}

impl fmt::Display for OAuthProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Github => f.write_str("github"),
            Self::Google => f.write_str("google"),
            Self::GitLab => f.write_str("gitlab"),
            Self::Orcid => f.write_str("orcid"),
            Self::CiLogon => f.write_str("cilogon"),
        }
    }
}

// =============================================================================
// BenchmarkType
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BenchmarkType {
    Upload,
    Download,
}

impl fmt::Display for BenchmarkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Upload => f.write_str("upload"),
            Self::Download => f.write_str("download"),
        }
    }
}

// =============================================================================
// DatasetLabel — non-empty string
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DatasetLabel(String);

impl DatasetLabel {
    pub fn new(label: impl Into<String>) -> Result<Self, ValidationError> {
        let s = label.into();
        if s.is_empty() {
            Err(ValidationError::EmptyValue)
        } else {
            Ok(Self(s))
        }
    }
    pub fn new_unchecked(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}
impl_newtype_common!(DatasetLabel);

// =============================================================================
// CollectionName — non-empty string
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CollectionName(String);

impl CollectionName {
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let s = name.into();
        if s.is_empty() {
            Err(ValidationError::EmptyValue)
        } else {
            Ok(Self(s))
        }
    }
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}
impl_newtype_common!(CollectionName);
