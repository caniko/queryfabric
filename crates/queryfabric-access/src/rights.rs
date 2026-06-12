use queryfabric_contract::{Activity, ResourceRef, Subject};
use queryfabric_provenance::{
    HistoryFilter, ProvenanceEntry, ProvenanceError, ProvenanceHistory, ProvenanceStore,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::policy::ResourcePolicy;

/// GDPR Article 15 record: everything a subject gets back from an access
/// request — the resource's policy posture plus its full audit trail.
///
/// Consumed by `queryfabric-portability` when assembling export bundles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessExportRecord {
    /// Resource the record describes.
    pub resource: ResourceRef,
    /// Subject the export was produced for.
    pub subject_id: Uuid,
    /// The resource's access tier, license, and restriction.
    pub policy: ResourcePolicy,
    /// Full ordered provenance history.
    pub history: ProvenanceHistory,
    /// When the export was produced (Unix milliseconds, caller-supplied).
    pub exported_at_unix_ms: i64,
}

/// Receipt for a GDPR Article 16 rectification.
///
/// The host applies the actual field change; this receipt proves the change
/// was recorded in provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectifyReceipt {
    /// Resource that was rectified.
    pub resource: ResourceRef,
    /// Name of the rectified field.
    pub field: String,
    /// When the rectification was recorded (Unix milliseconds).
    pub rectified_at_unix_ms: i64,
}

/// Result of a GDPR Article 17 erasure: soft-delete-with-reason semantics.
///
/// The host sets its own `deleted_at`/`deletion_reason` columns from this;
/// provenance survives so the erasure itself remains auditable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftDeletion {
    /// Resource that was soft-deleted.
    pub resource: ResourceRef,
    /// Why it was deleted.
    pub reason: String,
    /// When the deletion was recorded (Unix milliseconds).
    pub deleted_at_unix_ms: i64,
}

/// GDPR data-rights operations over a generic resource.
///
/// Each operation appends the corresponding [`Activity`] to the injected
/// [`ProvenanceStore`]. Timestamps are caller-supplied Unix milliseconds so
/// hosts control the clock.
pub struct DataRights<'a> {
    store: &'a dyn ProvenanceStore,
}

impl<'a> DataRights<'a> {
    /// Operate against `store`.
    #[must_use]
    pub fn new(store: &'a dyn ProvenanceStore) -> Self {
        Self { store }
    }

    async fn record(
        &self,
        resource: ResourceRef,
        actor: Option<Subject>,
        activity: Activity,
        now_unix_ms: i64,
    ) -> Result<(), ProvenanceError> {
        self.store
            .append(ProvenanceEntry {
                id: Uuid::now_v7(),
                resource,
                actor,
                activity: activity.into(),
                description: None,
                occurred_at_unix_ms: now_unix_ms,
            })
            .await
    }

    /// Article 15: produce the structured access-export record for a subject,
    /// recording the access itself.
    pub async fn access_export(
        &self,
        resource: ResourceRef,
        subject: &Subject,
        policy: ResourcePolicy,
        now_unix_ms: i64,
    ) -> Result<AccessExportRecord, ProvenanceError> {
        let history = self
            .store
            .history(resource, &HistoryFilter::default())
            .await?;
        self.record(
            resource,
            Some(subject.clone()),
            Activity::Accessed {
                rows: history.entries.len() as u64,
            },
            now_unix_ms,
        )
        .await?;
        Ok(AccessExportRecord {
            resource,
            subject_id: subject.id,
            policy,
            history,
            exported_at_unix_ms: now_unix_ms,
        })
    }

    /// Article 16: record the rectification of one field.
    ///
    /// The host performs the actual mutation; this only guarantees the audit
    /// trail.
    pub async fn rectify(
        &self,
        resource: ResourceRef,
        actor: Option<Subject>,
        field: &str,
        now_unix_ms: i64,
    ) -> Result<RectifyReceipt, ProvenanceError> {
        self.record(
            resource,
            actor,
            Activity::Modified {
                field: field.to_owned(),
            },
            now_unix_ms,
        )
        .await?;
        Ok(RectifyReceipt {
            resource,
            field: field.to_owned(),
            rectified_at_unix_ms: now_unix_ms,
        })
    }

    /// Article 17: record a soft deletion with its reason.
    pub async fn soft_delete(
        &self,
        resource: ResourceRef,
        actor: Option<Subject>,
        reason: &str,
        now_unix_ms: i64,
    ) -> Result<SoftDeletion, ProvenanceError> {
        self.record(
            resource,
            actor,
            Activity::Deleted {
                reason: reason.to_owned(),
            },
            now_unix_ms,
        )
        .await?;
        Ok(SoftDeletion {
            resource,
            reason: reason.to_owned(),
            deleted_at_unix_ms: now_unix_ms,
        })
    }

    /// Record the restoration of a previously soft-deleted resource.
    pub async fn restore(
        &self,
        resource: ResourceRef,
        actor: Option<Subject>,
        now_unix_ms: i64,
    ) -> Result<(), ProvenanceError> {
        self.record(resource, actor, Activity::Restored, now_unix_ms)
            .await
    }
}
