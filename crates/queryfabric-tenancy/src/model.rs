use queryfabric_access::GroupId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Kind of account operating on the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountKind {
    /// An interactive human user.
    Human,
    /// A machine identity (automation, integrations).
    Service,
}

/// A platform account.
///
/// `email` is a plain field; uniqueness is the host database's concern.
/// `verified` is whatever the host's verification flow means (e.g. academic
/// verification) — here it is just the bit the `Registered` access tier
/// checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub active: bool,
    pub verified: bool,
    pub kind: AccountKind,
}

impl Account {
    /// The [`queryfabric_contract::Subject`] this account acts as.
    #[must_use]
    pub fn subject(&self) -> queryfabric_contract::Subject {
        queryfabric_contract::Subject {
            id: self.id,
            registered: self.verified,
            attributes: Default::default(),
        }
    }
}

/// A named set of resources owned by one account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    /// Owning account.
    pub owner: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// An authorization group: members gain restricted-tier access to resources
/// the group is authorized for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    /// Administrating account.
    pub admin: Uuid,
    /// Free-text affiliation, when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub institution: Option<String>,
    /// Member accounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<Uuid>,
}
