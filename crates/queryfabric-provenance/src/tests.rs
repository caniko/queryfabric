use queryfabric_contract::{Activity, DomainActivity, NodeId, ResourceRef, Subject};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    HistoryFilter, ProvenanceEntry, ProvenanceHistory, ProvenanceStore, RecordedActivity,
    VecProvenanceStore,
};

fn resource(n: u128) -> ResourceRef {
    ResourceRef::new(Uuid::from_u128(0xA), Uuid::from_u128(n))
}

fn subject(n: u128) -> Subject {
    Subject {
        id: Uuid::from_u128(n),
        registered: true,
        attributes: Default::default(),
    }
}

fn entry(
    id: u128,
    res: ResourceRef,
    actor: Option<Subject>,
    activity: RecordedActivity,
    at: i64,
) -> ProvenanceEntry {
    ProvenanceEntry {
        id: Uuid::from_u128(id),
        resource: res,
        actor,
        activity,
        description: None,
        occurred_at_unix_ms: at,
    }
}

/// A host-domain activity this crate knows nothing about.
#[derive(Serialize)]
struct HostJobFinished {
    job: Uuid,
    items: u64,
}

impl DomainActivity for HostJobFinished {
    fn activity_kind(&self) -> &str {
        "host_job_finished"
    }
}

#[tokio::test]
async fn append_and_ordered_history() {
    let store = VecProvenanceStore::new();
    let res = resource(1);
    let actor = subject(7);

    let domain = RecordedActivity::from_domain(&HostJobFinished {
        job: Uuid::from_u128(99),
        items: 42,
    })
    .expect("domain activity serializes");

    // Append out of timestamp order to prove the store orders history.
    for (id, activity, at) in [
        (2, Activity::Accessed { rows: 10 }.into(), 200),
        (1, Activity::Created.into(), 100),
        (3, domain.clone(), 300),
        (
            4,
            Activity::FederationFlow {
                nodes: vec![NodeId(Uuid::from_u128(5))],
                latencies_ms: vec![12],
            }
            .into(),
            400,
        ),
    ] {
        store
            .append(entry(id, res, Some(actor.clone()), activity, at))
            .await
            .expect("append");
    }

    let history = store
        .history(res, &HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(history.resource, res);
    let order: Vec<i64> = history
        .entries
        .iter()
        .map(|e| e.occurred_at_unix_ms)
        .collect();
    assert_eq!(order, vec![100, 200, 300, 400]);
}

#[tokio::test]
async fn history_is_scoped_to_resource() {
    let store = VecProvenanceStore::new();
    store
        .append(entry(1, resource(1), None, Activity::Created.into(), 1))
        .await
        .expect("append");
    store
        .append(entry(2, resource(2), None, Activity::Created.into(), 2))
        .await
        .expect("append");

    let history = store
        .history(resource(1), &HistoryFilter::default())
        .await
        .expect("history");
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].id, Uuid::from_u128(1));
}

#[tokio::test]
async fn filter_by_tag_actor_and_time_range() {
    let store = VecProvenanceStore::new();
    let res = resource(1);
    let alice = subject(1);
    let bob = subject(2);

    store
        .append(entry(
            1,
            res,
            Some(alice.clone()),
            Activity::Created.into(),
            100,
        ))
        .await
        .expect("append");
    store
        .append(entry(
            2,
            res,
            Some(bob.clone()),
            Activity::Accessed { rows: 5 }.into(),
            200,
        ))
        .await
        .expect("append");
    store
        .append(entry(
            3,
            res,
            Some(alice.clone()),
            Activity::Accessed { rows: 9 }.into(),
            300,
        ))
        .await
        .expect("append");
    // System event: no actor.
    store
        .append(entry(
            4,
            res,
            None,
            Activity::BackupAnchor {
                location: "s3://backups/nightly".to_owned(),
                content_hash: "abc".to_owned(),
            }
            .into(),
            400,
        ))
        .await
        .expect("append");

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
    assert_eq!(accessed.entries.len(), 2);

    let by_alice = store
        .history(
            res,
            &HistoryFilter {
                actor: Some(alice.id),
                ..Default::default()
            },
        )
        .await
        .expect("history");
    assert_eq!(by_alice.entries.len(), 2);

    let windowed = store
        .history(
            res,
            &HistoryFilter {
                from_unix_ms: Some(200),
                until_unix_ms: Some(400),
                ..Default::default()
            },
        )
        .await
        .expect("history");
    let ids: Vec<Uuid> = windowed.entries.iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![Uuid::from_u128(2), Uuid::from_u128(3)]);

    // Actor filter never matches actor-less system events.
    let by_bob = store
        .history(
            res,
            &HistoryFilter {
                actor: Some(bob.id),
                ..Default::default()
            },
        )
        .await
        .expect("history");
    assert_eq!(by_bob.entries.len(), 1);
}

#[test]
fn domain_payload_round_trips_opaquely() {
    let recorded = RecordedActivity::from_domain(&HostJobFinished {
        job: Uuid::from_u128(99),
        items: 42,
    })
    .expect("serialize");

    assert_eq!(recorded.tag(), "host_job_finished");

    let json = serde_json::to_string(&recorded).expect("serialize recorded");
    let back: RecordedActivity = serde_json::from_str(&json).expect("deserialize recorded");
    assert_eq!(back, recorded);

    match back {
        RecordedActivity::Domain { kind, payload } => {
            assert_eq!(kind, "host_job_finished");
            assert_eq!(payload["items"], 42);
        }
        RecordedActivity::Core { .. } => panic!("domain activity must stay opaque"),
    }
}

#[test]
fn core_activity_serde_carries_contract_tag() {
    let recorded: RecordedActivity = Activity::Deleted {
        reason: "user request".to_owned(),
    }
    .into();
    assert_eq!(recorded.tag(), "deleted");

    let value = serde_json::to_value(&recorded).expect("serialize");
    assert_eq!(value["scope"], "core");
    assert_eq!(value["activity"], "deleted");
    assert_eq!(value["reason"], "user request");

    let back: RecordedActivity = serde_json::from_value(value).expect("deserialize");
    assert_eq!(back, recorded);
}

#[test]
fn history_serde_round_trip() {
    let res = resource(1);
    let history = ProvenanceHistory {
        resource: res,
        entries: vec![entry(
            1,
            res,
            Some(subject(1)),
            Activity::Created.into(),
            100,
        )],
    };
    let json = serde_json::to_string(&history).expect("serialize");
    let back: ProvenanceHistory = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, history);
}
