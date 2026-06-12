use async_trait::async_trait;
use queryfabric_contract::{AccessDecision, AccessOutcome, AccessPolicy, ResourceRef, Subject};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identity of an authorization group.
///
/// Owned here (not in `queryfabric-tenancy`) so that `queryfabric-access`
/// never depends on a concrete tenancy implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupId(pub Uuid);

/// Host-implemented lookup of ownership, group, and agreement facts.
///
/// `queryfabric-tenancy` ships an in-memory implementation; production hosts
/// back this with their own identity store.
#[async_trait]
pub trait OwnershipSource: Send + Sync {
    /// The subject owning `resource`, if any.
    async fn owner(&self, resource: ResourceRef) -> Option<Subject>;

    /// Groups the subject belongs to.
    async fn groups_for(&self, subject: &Subject) -> Vec<GroupId>;

    /// Whether the subject belongs to `group`.
    async fn member_of(&self, subject: &Subject, group: GroupId) -> bool {
        self.groups_for(subject).await.contains(&group)
    }

    /// Groups authorized to access `resource`.
    async fn resource_groups(&self, resource: ResourceRef) -> Vec<GroupId>;

    /// Whether the subject holds an accepted, unexpired data-use agreement
    /// for `resource`.
    async fn has_accepted_agreement(&self, subject: &Subject, resource: ResourceRef) -> bool;
}

/// Pre-fetched ownership facts for one `(subject, resource)` pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OwnershipSnapshot {
    /// The subject owns the resource.
    pub is_owner: bool,
    /// The subject belongs to a group authorized for the resource.
    pub in_authorized_group: bool,
    /// The subject holds an accepted data-use agreement for the resource.
    pub has_accepted_agreement: bool,
}

impl OwnershipSnapshot {
    /// A snapshot granting nothing — the deny-by-default baseline.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            is_owner: false,
            in_authorized_group: false,
            has_accepted_agreement: false,
        }
    }
}

/// Pure three-tier access decision. Deny by default.
///
/// - `Open` — any subject.
/// - `Registered` — requires [`Subject::registered`].
/// - `Restricted` — requires ownership, authorized-group membership, or an
///   accepted agreement; anything else (including policies this crate does
///   not know) is denied.
#[must_use]
pub fn evaluate_with_snapshot(
    subject: &Subject,
    policy: &AccessPolicy,
    ownership: &OwnershipSnapshot,
) -> AccessOutcome {
    match policy {
        AccessPolicy::Open => AccessOutcome::Allow,
        AccessPolicy::Registered => {
            if subject.registered {
                AccessOutcome::Allow
            } else {
                AccessOutcome::Deny {
                    reason: "resource requires a registered account".to_owned(),
                }
            }
        }
        AccessPolicy::Restricted { .. } => {
            if ownership.is_owner
                || ownership.in_authorized_group
                || ownership.has_accepted_agreement
            {
                AccessOutcome::Allow
            } else {
                AccessOutcome::Deny {
                    reason:
                        "restricted resource: no ownership, group membership, or accepted agreement"
                            .to_owned(),
                }
            }
        }
        // `AccessPolicy` is #[non_exhaustive]: deny tiers this crate predates.
        other => AccessOutcome::Deny {
            reason: format!("unknown access policy: {other:?}"),
        },
    }
}

/// Fetch the ownership facts [`evaluate_with_snapshot`] needs for the
/// restricted tier.
pub async fn snapshot_for(
    subject: &Subject,
    resource: ResourceRef,
    ownership: &dyn OwnershipSource,
) -> OwnershipSnapshot {
    let is_owner = ownership
        .owner(resource)
        .await
        .is_some_and(|owner| owner.id == subject.id);
    if is_owner {
        return OwnershipSnapshot {
            is_owner: true,
            in_authorized_group: false,
            has_accepted_agreement: false,
        };
    }
    let subject_groups = ownership.groups_for(subject).await;
    let in_authorized_group = if subject_groups.is_empty() {
        false
    } else {
        ownership
            .resource_groups(resource)
            .await
            .iter()
            .any(|group| subject_groups.contains(group))
    };
    if in_authorized_group {
        return OwnershipSnapshot {
            is_owner: false,
            in_authorized_group: true,
            has_accepted_agreement: false,
        };
    }
    OwnershipSnapshot {
        is_owner: false,
        in_authorized_group: false,
        has_accepted_agreement: ownership.has_accepted_agreement(subject, resource).await,
    }
}

/// Evaluate a subject against a resource's access tier, looking up ownership
/// facts only when the restricted tier requires them.
pub async fn evaluate_access(
    subject: &Subject,
    resource: ResourceRef,
    policy: &AccessPolicy,
    ownership: &dyn OwnershipSource,
) -> AccessOutcome {
    let snapshot = match policy {
        AccessPolicy::Restricted { .. } => snapshot_for(subject, resource, ownership).await,
        _ => OwnershipSnapshot::none(),
    };
    evaluate_with_snapshot(subject, policy, &snapshot)
}

/// [`AccessDecision`] implementation over a pre-fetched [`OwnershipSnapshot`].
///
/// Lets synchronous call sites (and the Phase 01 contract trait) reuse the
/// same pure decision core as [`evaluate_access`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotAccessDecision {
    /// Ownership facts for the `(subject, resource)` pair under evaluation.
    pub ownership: OwnershipSnapshot,
}

impl AccessDecision for SnapshotAccessDecision {
    fn evaluate(&self, subject: &Subject, policy: &AccessPolicy) -> AccessOutcome {
        evaluate_with_snapshot(subject, policy, &self.ownership)
    }
}
