use queryfabric_contract::AccessPolicy;
use serde::{Deserialize, Serialize};

use crate::license::DataLicense;

/// A data-use restriction attached to a resource.
///
/// All fields are opaque to QueryFabric; `kind` is expected to be a GA4GH
/// DUO-style code (e.g. `DUO:0000007`) but any host vocabulary works.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataUseRestriction {
    /// Machine-readable restriction kind (e.g. a GA4GH DUO code).
    pub kind: String,
    /// Human-readable summary of the restriction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Where the restriction is defined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

/// The complete access posture of a resource: tier, license, and restriction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePolicy {
    /// Access tier evaluated by [`evaluate_access`](crate::evaluate_access).
    pub policy: AccessPolicy,
    /// Open data license, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<DataLicense>,
    /// Data-use restriction, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<DataUseRestriction>,
}

impl ResourcePolicy {
    /// An open, unrestricted policy with no declared license.
    #[must_use]
    pub const fn open() -> Self {
        Self {
            policy: AccessPolicy::Open,
            license: None,
            restriction: None,
        }
    }
}
