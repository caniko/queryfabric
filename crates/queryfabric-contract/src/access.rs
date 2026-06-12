use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An actor requesting access to a resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subject {
    pub id: Uuid,
    /// Whether the subject has completed the host's registration flow.
    pub registered: bool,
    /// Opaque attributes (e.g. GA4GH passport visas) that the host's
    /// [`AccessDecision`] implementation understands.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

/// Access tier attached to a resource.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum AccessPolicy {
    Open,
    Registered,
    Restricted {
        /// GA4GH DUO data-use restriction codes, opaque to QueryFabric.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        data_use_restrictions: Vec<String>,
    },
}

/// Outcome of evaluating a [`Subject`] against an [`AccessPolicy`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AccessOutcome {
    Allow,
    Deny { reason: String },
}

impl AccessOutcome {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

/// Host-implemented access policy evaluation.
///
/// Implemented in Phase 05 (`queryfabric-access` ships a default evaluator;
/// hosts may supply their own).
pub trait AccessDecision: Send + Sync {
    fn evaluate(&self, subject: &Subject, policy: &AccessPolicy) -> AccessOutcome;
}
