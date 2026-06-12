use queryfabric_access::{GroupId, OwnershipSource, evaluate_access};
use queryfabric_contract::{AccessPolicy, ResourceRef};
use uuid::Uuid;

use crate::{Account, AccountKind, Collection, Group, InMemoryOwnership};

fn account(n: u128, verified: bool) -> Account {
    Account {
        id: Uuid::from_u128(n),
        email: format!("user{n}@example.org"),
        active: true,
        verified,
        kind: AccountKind::Human,
    }
}

fn resource(n: u128) -> ResourceRef {
    ResourceRef::new(Uuid::from_u128(0xD), Uuid::from_u128(n))
}

fn restricted() -> AccessPolicy {
    AccessPolicy::Restricted {
        data_use_restrictions: vec![],
    }
}

fn populated() -> (InMemoryOwnership, Account, Account, Account, Account) {
    let registry = InMemoryOwnership::new();
    let owner = account(1, true);
    let member = account(2, true);
    let agreement_holder = account(3, true);
    let stranger = account(4, true);
    for acc in [&owner, &member, &agreement_holder, &stranger] {
        registry.add_account(acc.clone());
    }

    let group = Group {
        id: GroupId(Uuid::from_u128(0x10)),
        name: "lab".to_owned(),
        admin: owner.id,
        institution: Some("Example Institute".to_owned()),
        members: vec![member.id],
    };
    registry.add_group(group.clone());

    let res = resource(1);
    registry.set_owner(res, owner.id);
    registry.authorize_group(res, group.id);
    registry.accept_agreement(agreement_holder.id, res);

    (registry, owner, member, agreement_holder, stranger)
}

#[tokio::test]
async fn ownership_lookups_answer_from_registered_state() {
    let (registry, owner, member, agreement_holder, stranger) = populated();
    let res = resource(1);

    let resolved_owner = registry.owner(res).await.expect("owner registered");
    assert_eq!(resolved_owner.id, owner.id);
    assert!(resolved_owner.registered);

    assert_eq!(
        registry.groups_for(&member.subject()).await,
        vec![GroupId(Uuid::from_u128(0x10))]
    );
    assert!(registry.groups_for(&stranger.subject()).await.is_empty());

    assert!(
        registry
            .member_of(&member.subject(), GroupId(Uuid::from_u128(0x10)))
            .await
    );
    assert!(
        registry
            .has_accepted_agreement(&agreement_holder.subject(), res)
            .await
    );
    assert!(registry.owner(resource(99)).await.is_none());
}

#[tokio::test]
async fn in_memory_ownership_satisfies_the_restricted_tier_matrix() {
    let (registry, owner, member, agreement_holder, stranger) = populated();
    let res = resource(1);
    let policy = restricted();

    // Owner, group member, and agreement holder are each allowed.
    for granted in [&owner, &member, &agreement_holder] {
        assert!(
            evaluate_access(&granted.subject(), res, &policy, &registry)
                .await
                .is_allowed(),
            "account {} should be granted",
            granted.email
        );
    }

    // A verified stranger is still denied: deny-by-default.
    assert!(
        !evaluate_access(&stranger.subject(), res, &policy, &registry)
            .await
            .is_allowed()
    );

    // The registered tier keys on Account::verified.
    let unverified = account(5, false);
    assert!(
        !evaluate_access(
            &unverified.subject(),
            res,
            &AccessPolicy::Registered,
            &registry
        )
        .await
        .is_allowed()
    );
    assert!(
        evaluate_access(
            &stranger.subject(),
            res,
            &AccessPolicy::Registered,
            &registry
        )
        .await
        .is_allowed()
    );
}

#[test]
fn model_serde_round_trips() {
    let collection = Collection {
        id: Uuid::from_u128(1),
        name: "benchmark inputs".to_owned(),
        owner: Uuid::from_u128(2),
        notes: None,
    };
    let json = serde_json::to_string(&collection).expect("serialize");
    let back: Collection = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, collection);

    let kind_json = serde_json::to_string(&AccountKind::Service).expect("serialize");
    assert_eq!(kind_json, "\"service\"");
}
