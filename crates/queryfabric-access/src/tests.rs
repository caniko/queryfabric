use std::collections::{BTreeMap, HashMap, HashSet};

use async_trait::async_trait;
use queryfabric_contract::{AccessDecision, AccessOutcome, AccessPolicy, ResourceRef, Subject};
use queryfabric_provenance::{
    HistoryFilter, ProvenanceStore, RecordedActivity, VecProvenanceStore,
};
use uuid::Uuid;

use crate::{
    DataLicense, DataRights, DataUseRestriction, GroupId, OwnershipSnapshot, OwnershipSource,
    ResourcePolicy, SnapshotAccessDecision, evaluate_access, evaluate_with_snapshot,
};

fn resource(n: u128) -> ResourceRef {
    ResourceRef::new(Uuid::from_u128(0xB), Uuid::from_u128(n))
}

fn subject(n: u128, registered: bool) -> Subject {
    Subject {
        id: Uuid::from_u128(n),
        registered,
        attributes: BTreeMap::new(),
    }
}

fn restricted() -> AccessPolicy {
    AccessPolicy::Restricted {
        data_use_restrictions: vec!["DUO:0000007".to_owned()],
    }
}

/// In-memory mock so the matrix runs without queryfabric-tenancy.
#[derive(Default)]
struct MockOwnership {
    owners: HashMap<ResourceRef, Subject>,
    subject_groups: HashMap<Uuid, Vec<GroupId>>,
    resource_groups: HashMap<ResourceRef, Vec<GroupId>>,
    agreements: HashSet<(Uuid, ResourceRef)>,
}

#[async_trait]
impl OwnershipSource for MockOwnership {
    async fn owner(&self, resource: ResourceRef) -> Option<Subject> {
        self.owners.get(&resource).cloned()
    }

    async fn groups_for(&self, subject: &Subject) -> Vec<GroupId> {
        self.subject_groups
            .get(&subject.id)
            .cloned()
            .unwrap_or_default()
    }

    async fn resource_groups(&self, resource: ResourceRef) -> Vec<GroupId> {
        self.resource_groups
            .get(&resource)
            .cloned()
            .unwrap_or_default()
    }

    async fn has_accepted_agreement(&self, subject: &Subject, resource: ResourceRef) -> bool {
        self.agreements.contains(&(subject.id, resource))
    }
}

#[test]
fn tier_matrix_with_snapshots() {
    let registered = subject(1, true);
    let unregistered = subject(2, false);
    let none = OwnershipSnapshot::none();

    // Open: everyone, registered or not.
    for who in [&registered, &unregistered] {
        assert!(evaluate_with_snapshot(who, &AccessPolicy::Open, &none).is_allowed());
    }

    // Registered: only registered subjects.
    assert!(evaluate_with_snapshot(&registered, &AccessPolicy::Registered, &none).is_allowed());
    assert!(!evaluate_with_snapshot(&unregistered, &AccessPolicy::Registered, &none).is_allowed());

    // Restricted: each single grant suffices...
    for grant in [
        OwnershipSnapshot {
            is_owner: true,
            ..OwnershipSnapshot::none()
        },
        OwnershipSnapshot {
            in_authorized_group: true,
            ..OwnershipSnapshot::none()
        },
        OwnershipSnapshot {
            has_accepted_agreement: true,
            ..OwnershipSnapshot::none()
        },
    ] {
        assert!(evaluate_with_snapshot(&registered, &restricted(), &grant).is_allowed());
    }

    // ...and with no grant the default is deny, even for registered subjects.
    let denied = evaluate_with_snapshot(&registered, &restricted(), &none);
    match denied {
        AccessOutcome::Deny { reason } => assert!(reason.contains("restricted")),
        AccessOutcome::Allow => panic!("restricted resource without grants must deny"),
    }
}

#[test]
fn snapshot_decision_implements_contract_trait() {
    let decision = SnapshotAccessDecision {
        ownership: OwnershipSnapshot {
            is_owner: true,
            ..OwnershipSnapshot::none()
        },
    };
    assert!(
        decision
            .evaluate(&subject(1, false), &restricted())
            .is_allowed()
    );
}

#[tokio::test]
async fn restricted_paths_through_ownership_source() {
    let res = resource(1);
    let owner = subject(10, true);
    let group_member = subject(11, true);
    let agreement_holder = subject(12, true);
    let stranger = subject(13, true);
    let group = GroupId(Uuid::from_u128(0x61));

    let mut ownership = MockOwnership::default();
    ownership.owners.insert(res, owner.clone());
    ownership
        .subject_groups
        .insert(group_member.id, vec![group]);
    ownership.resource_groups.insert(res, vec![group]);
    ownership.agreements.insert((agreement_holder.id, res));

    let policy = restricted();
    assert!(
        evaluate_access(&owner, res, &policy, &ownership)
            .await
            .is_allowed()
    );
    assert!(
        evaluate_access(&group_member, res, &policy, &ownership)
            .await
            .is_allowed()
    );
    assert!(
        evaluate_access(&agreement_holder, res, &policy, &ownership)
            .await
            .is_allowed()
    );
    assert!(
        !evaluate_access(&stranger, res, &policy, &ownership)
            .await
            .is_allowed()
    );

    // Open and Registered tiers never consult the ownership source.
    assert!(
        evaluate_access(&stranger, res, &AccessPolicy::Open, &ownership)
            .await
            .is_allowed()
    );
}

#[tokio::test]
async fn gdpr_operations_emit_the_right_activities() {
    let store = VecProvenanceStore::new();
    let rights = DataRights::new(&store);
    let res = resource(2);
    let actor = subject(1, true);

    let deletion = rights
        .soft_delete(res, Some(actor.clone()), "user requested erasure", 1_000)
        .await
        .expect("soft delete");
    assert_eq!(deletion.reason, "user requested erasure");
    assert_eq!(deletion.deleted_at_unix_ms, 1_000);

    let receipt = rights
        .rectify(res, Some(actor.clone()), "label", 2_000)
        .await
        .expect("rectify");
    assert_eq!(receipt.field, "label");

    rights
        .restore(res, Some(actor.clone()), 3_000)
        .await
        .expect("restore");

    for (tag, expected) in [("deleted", 1), ("modified", 1), ("restored", 1)] {
        let history = store
            .history(
                res,
                &HistoryFilter {
                    activity_tag: Some(tag.to_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("history");
        assert_eq!(history.entries.len(), expected, "tag {tag}");
    }

    // The deleted entry carries the reason through the core activity.
    let deleted = store
        .history(
            res,
            &HistoryFilter {
                activity_tag: Some("deleted".to_owned()),
                ..Default::default()
            },
        )
        .await
        .expect("history");
    match &deleted.entries[0].activity {
        RecordedActivity::Core { activity } => {
            assert_eq!(activity.tag(), "deleted");
        }
        RecordedActivity::Domain { .. } => panic!("soft delete must be a core activity"),
    }
}

#[tokio::test]
async fn access_export_returns_structured_record_and_records_access() {
    let store = VecProvenanceStore::new();
    let rights = DataRights::new(&store);
    let res = resource(3);
    let actor = subject(1, true);

    rights
        .soft_delete(res, Some(actor.clone()), "cleanup", 100)
        .await
        .expect("seed history");
    rights
        .restore(res, Some(actor.clone()), 200)
        .await
        .expect("seed history");

    let policy = ResourcePolicy {
        policy: AccessPolicy::Open,
        license: Some(DataLicense::CcBy),
        restriction: Some(DataUseRestriction {
            kind: "DUO:0000042".to_owned(),
            summary: Some("general research use".to_owned()),
            source_url: None,
        }),
    };

    let record = rights
        .access_export(res, &actor, policy.clone(), 300)
        .await
        .expect("access export");
    assert_eq!(record.resource, res);
    assert_eq!(record.subject_id, actor.id);
    assert_eq!(record.policy, policy);
    assert_eq!(record.history.entries.len(), 2);
    assert_eq!(record.exported_at_unix_ms, 300);

    // The export itself shows up in the audit trail as an access.
    let accessed = store
        .history(
            res,
            &HistoryFilter {
                activity_tag: Some("accessed".to_owned()),
                ..Default::default()
            },
        )
        .await
        .expect("history");
    assert_eq!(accessed.entries.len(), 1);

    // Round-trip: the record is a serializable structure for sub-03.
    let json = serde_json::to_string(&record).expect("serialize record");
    let back: crate::AccessExportRecord = serde_json::from_str(&json).expect("deserialize record");
    assert_eq!(back, record);
}

#[test]
fn license_metadata_is_spdx_shaped() {
    assert_eq!(DataLicense::Cc0.spdx_id(), "CC0-1.0");
    assert_eq!(DataLicense::OdcOdbl.spdx_id(), "ODbL-1.0");
    assert_eq!(
        DataLicense::CcBy.rights_uri(),
        "https://creativecommons.org/licenses/by/4.0/"
    );
    assert_eq!(
        DataLicense::Pddl.display_name(),
        "Open Data Commons Public Domain Dedication and License v1.0"
    );
}
