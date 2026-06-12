use queryfabric_contract::{Activity, DomainActivity, ResourceRef, Subject};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An activity as recorded in the log: either a universal core verb or an
/// opaque host-domain extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum RecordedActivity {
    /// A universal activity from the contract vocabulary.
    Core {
        #[serde(flatten)]
        activity: Activity,
    },
    /// A host-domain activity carried opaquely.
    Domain {
        /// Stable machine-readable activity kind
        /// ([`DomainActivity::activity_kind`]).
        kind: String,
        /// Serialized host payload; never interpreted by this crate.
        payload: serde_json::Value,
    },
}

impl RecordedActivity {
    /// Wrap a host [`DomainActivity`] as an opaque domain entry.
    pub fn from_domain<A: DomainActivity>(activity: &A) -> Result<Self, serde_json::Error> {
        Ok(Self::Domain {
            kind: activity.activity_kind().to_owned(),
            payload: serde_json::to_value(activity)?,
        })
    }

    /// Stable low-cardinality tag for indexed storage and filtering.
    ///
    /// Core entries reuse [`Activity::tag`]; domain entries use their own
    /// `kind`.
    #[must_use]
    pub fn tag(&self) -> &str {
        match self {
            Self::Core { activity } => activity.tag(),
            Self::Domain { kind, .. } => kind,
        }
    }
}

impl From<Activity> for RecordedActivity {
    fn from(activity: Activity) -> Self {
        Self::Core { activity }
    }
}

/// A single provenance log entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceEntry {
    /// Unique ID for this record.
    pub id: Uuid,
    /// The resource this event applies to.
    pub resource: ResourceRef,
    /// The subject who performed the action (`None` for system actions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Subject>,
    /// What happened.
    pub activity: RecordedActivity,
    /// Free-text description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When it happened, as Unix milliseconds (caller-supplied).
    pub occurred_at_unix_ms: i64,
}

/// Ordered provenance history for one resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceHistory {
    /// Resource whose history is being returned.
    pub resource: ResourceRef,
    /// Entries ordered by `occurred_at_unix_ms`, then insertion order.
    pub entries: Vec<ProvenanceEntry>,
}
