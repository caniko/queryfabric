use std::sync::Mutex;

use async_trait::async_trait;
use queryfabric_contract::ResourceRef;
use uuid::Uuid;

use crate::entry::{ProvenanceEntry, ProvenanceHistory};

/// Error surface for provenance storage backends.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProvenanceError {
    /// The backing store failed to persist or read entries.
    #[error("provenance storage failed: {0}")]
    Storage(String),
    /// An entry or payload could not be (de)serialized.
    #[error("provenance serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Optional constraints on a history query. All fields are conjunctive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryFilter {
    /// Only entries whose activity tag matches ([`RecordedActivity::tag`]).
    ///
    /// [`RecordedActivity::tag`]: crate::RecordedActivity::tag
    pub activity_tag: Option<String>,
    /// Only entries performed by this actor (subject id).
    pub actor: Option<Uuid>,
    /// Only entries at or after this Unix-millisecond timestamp.
    pub from_unix_ms: Option<i64>,
    /// Only entries strictly before this Unix-millisecond timestamp.
    pub until_unix_ms: Option<i64>,
}

impl HistoryFilter {
    fn matches(&self, entry: &ProvenanceEntry) -> bool {
        let tag_ok = self
            .activity_tag
            .as_deref()
            .is_none_or(|tag| entry.activity.tag() == tag);
        let actor_ok = self
            .actor
            .is_none_or(|actor| entry.actor.as_ref().map(|subject| subject.id) == Some(actor));
        let from_ok = self
            .from_unix_ms
            .is_none_or(|from| entry.occurred_at_unix_ms >= from);
        let until_ok = self
            .until_unix_ms
            .is_none_or(|until| entry.occurred_at_unix_ms < until);
        tag_ok && actor_ok && from_ok && until_ok
    }
}

/// Append-only provenance storage.
///
/// Implementations must preserve insertion order for equal timestamps and
/// never mutate or remove appended entries.
#[async_trait]
pub trait ProvenanceStore: Send + Sync {
    /// Append one entry to the log.
    async fn append(&self, entry: ProvenanceEntry) -> Result<(), ProvenanceError>;

    /// Ordered history for `resource`, restricted by `filter`.
    async fn history(
        &self,
        resource: ResourceRef,
        filter: &HistoryFilter,
    ) -> Result<ProvenanceHistory, ProvenanceError>;
}

/// In-memory reference [`ProvenanceStore`].
///
/// Suitable for tests and the demonstrator host; not durable.
#[derive(Debug, Default)]
pub struct VecProvenanceStore {
    entries: Mutex<Vec<ProvenanceEntry>>,
}

impl VecProvenanceStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ProvenanceStore for VecProvenanceStore {
    async fn append(&self, entry: ProvenanceEntry) -> Result<(), ProvenanceError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| ProvenanceError::Storage("provenance store mutex poisoned".to_owned()))?;
        entries.push(entry);
        Ok(())
    }

    async fn history(
        &self,
        resource: ResourceRef,
        filter: &HistoryFilter,
    ) -> Result<ProvenanceHistory, ProvenanceError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| ProvenanceError::Storage("provenance store mutex poisoned".to_owned()))?;
        let mut matched: Vec<ProvenanceEntry> = entries
            .iter()
            .filter(|entry| entry.resource == resource && filter.matches(entry))
            .cloned()
            .collect();
        // Stable sort keeps insertion order for equal timestamps.
        matched.sort_by_key(|entry| entry.occurred_at_unix_ms);
        Ok(ProvenanceHistory {
            resource,
            entries: matched,
        })
    }
}
