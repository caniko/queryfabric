//! Generic two-node in-memory federation flow:
//! register → announce → health sweep → route. No domain type appears and
//! no networking is involved.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use papaya::HashMap as PapayaMap;
use queryfabric_cluster::{
    CheckAllClusters, CircuitConfig, DhtNaming, GetHealth, HealthMonitorActor, HealthMonitorArgs,
    HubRegistryState,
};
use queryfabric_contract::{Health, NodeId, ResourceRef};
use queryfabric_federation::{
    ClusterIdentity, ClusterNodeActor, ClusterNodeArgs, ClusterRefs, ClusterRegistration,
    FederationHost, HubActor, HubActorArgs, InMemoryTransport, RegisterCluster, ResourceAction,
    ResourceAnnouncement, ResourceLocalityIndex, SchemaMigration, SyncAllSchemas, TransportProbe,
    get_healthy_flight_endpoint, resolve_locality,
};
use thespis::actor::Spawn;
use uuid::Uuid;

const NAMING: DhtNaming = DhtNaming::new("fabric-cluster", "fabric-hub");

/// Fully generic test host: registers clusters with fresh node ids, serves
/// one migration, records applied DDL, and counts announcements.
#[derive(Default)]
struct TestHost {
    applied_ddl: Mutex<Vec<String>>,
    announcements: AtomicU64,
    resources: AtomicU64,
}

#[async_trait]
impl FederationHost for TestHost {
    type CatalogEntry = serde_json::Value;

    async fn register_cluster(
        &self,
        identity: &ClusterIdentity,
        federation_password: &str,
    ) -> Result<ClusterRegistration, String> {
        if federation_password != "open-sesame" {
            return Err("bad federation password".to_owned());
        }
        Ok(ClusterRegistration {
            cluster_id: NodeId::from(Uuid::now_v7()),
            api_key: format!("key-{}", identity.name),
            message: "welcome".to_owned(),
        })
    }

    async fn on_announce(&self, _announcement: &ResourceAnnouncement<Self::CatalogEntry>) {
        self.announcements.fetch_add(1, Ordering::Relaxed);
    }

    fn schema_version(&self) -> i32 {
        1
    }

    fn schema_migrations(&self, from_version: i32) -> Vec<SchemaMigration> {
        if from_version >= 1 {
            return Vec::new();
        }
        vec![SchemaMigration {
            version: 1,
            name: "init".to_owned(),
            sql: "CREATE TABLE resources (id UUID)".to_owned(),
        }]
    }

    async fn resource_count(&self) -> u64 {
        self.resources.load(Ordering::Relaxed)
    }

    async fn apply_ddl(&self, migration: &SchemaMigration) -> Result<(), String> {
        self.applied_ddl
            .lock()
            .expect("ddl lock")
            .push(migration.name.clone());
        Ok(())
    }
}

fn identity(name: &str) -> ClusterIdentity {
    ClusterIdentity {
        name: name.to_owned(),
        endpoint: format!("https://{name}.example.org"),
        port: 443,
        ca_certificate_pem: None,
        description: None,
        institution: None,
        contact_email: None,
    }
}

fn resource() -> ResourceRef {
    ResourceRef::new(Uuid::now_v7(), Uuid::now_v7())
}

#[tokio::test]
async fn federation_two_node() {
    // Shared hub state.
    let cluster_refs: ClusterRefs = Arc::new(PapayaMap::new());
    let locality: ResourceLocalityIndex = Arc::new(PapayaMap::new());
    let registry = HubRegistryState::new(Arc::clone(&cluster_refs), Arc::clone(&locality));

    // Two in-process nodes behind the in-memory transport.
    let transport = Arc::new(InMemoryTransport::new());
    let host_a = Arc::new(TestHost::default());
    let host_b = Arc::new(TestHost::default());
    host_b.resources.store(2, Ordering::Relaxed);

    for (name, host, endpoint) in [
        ("node-a", Arc::clone(&host_a), "node-a:50052"),
        ("node-b", Arc::clone(&host_b), "node-b:50052"),
    ] {
        let node = ClusterNodeActor::spawn(ClusterNodeArgs {
            identity: identity(name),
            host,
            flight_endpoint: Some(endpoint.to_owned()),
            flight_tls: false,
            schema_version: 0,
        });
        transport.register(NAMING.cluster_name(name), node);
    }

    // Hub actor with its own host.
    let hub_host = Arc::new(TestHost::default());
    let hub = HubActor::spawn(HubActorArgs {
        host: Arc::clone(&hub_host),
        registry,
        transport: Arc::clone(&transport),
        naming: NAMING,
        circuit_reset: None,
    });

    // 1. Register both nodes.
    let mut node_ids = Vec::new();
    for name in ["node-a", "node-b"] {
        let reply = hub
            .ask(RegisterCluster {
                identity: identity(name),
                federation_password: "open-sesame".to_owned(),
            })
            .send()
            .await
            .expect("registration reply");
        assert!(reply.accepted, "registration accepted for {name}");
        assert_eq!(reply.schema_version, 1);
        assert_eq!(reply.schema_ddl.len(), 1);
        node_ids.push(reply.cluster_id);
    }
    let (node_a, node_b) = (node_ids[0], node_ids[1]);
    assert_ne!(node_a, node_b);

    // A bad password is rejected.
    let rejected = hub
        .ask(RegisterCluster {
            identity: identity("node-c"),
            federation_password: "wrong".to_owned(),
        })
        .send()
        .await
        .expect("rejection reply");
    assert!(!rejected.accepted);

    // 2. Announce two resources living on node B.
    let (r1, r2) = (resource(), resource());
    for r in [r1, r2] {
        hub.ask(ResourceAnnouncement::<serde_json::Value> {
            cluster_id: node_b,
            resource_id: r,
            action: ResourceAction::Added,
            facets: vec!["primary".to_owned()],
            catalog_entry: None,
        })
        .send()
        .await
        .expect("announcement handled");
    }
    assert_eq!(hub_host.announcements.load(Ordering::Relaxed), 2);

    // 3. Health sweep over the same transport (also discovers Flight
    //    endpoints). Long interval: we drive the sweep explicitly.
    let probe = TransportProbe::new(Arc::clone(&transport), Arc::clone(&cluster_refs));
    let (monitor, health_cache) = HealthMonitorActor::spawn_with_args(HealthMonitorArgs {
        cluster_refs: Arc::clone(&cluster_refs),
        check_interval: Duration::from_secs(3600),
        health_cache: Arc::new(PapayaMap::new()),
        probe_timeout: Duration::from_secs(5),
        circuit_config: CircuitConfig::default(),
        probe,
    })
    .await;

    monitor
        .tell(CheckAllClusters)
        .send()
        .await
        .expect("sweep triggered");
    // Same mailbox ⇒ the ask below observes the completed sweep.
    let health = monitor
        .ask(GetHealth(node_b))
        .send()
        .await
        .expect("health reply");
    assert_eq!(health, Some(Health::Healthy));

    // 4. Routing: a query touching r1, r2, and an unknown resource
    //    partitions into node-B remote group + hub-local remainder.
    let unknown = resource();
    let decision = resolve_locality(&locality, &[r1, r2, unknown]);
    assert_eq!(decision.local_ids, vec![unknown]);
    assert_eq!(decision.remote.len(), 1);
    assert_eq!(decision.remote[0].cluster_id, node_b);
    let mut routed = decision.remote[0].resource_ids.clone();
    routed.sort();
    let mut expected = vec![r1, r2];
    expected.sort();
    assert_eq!(routed, expected);

    // The sweep discovered node B's Flight endpoint, and B is delegatable.
    assert_eq!(
        get_healthy_flight_endpoint(&cluster_refs, &health_cache, node_b),
        Some(("node-b:50052".to_owned(), false))
    );

    // 5. Schema-sync broadcast applies the opaque DDL on both nodes.
    let results = hub
        .ask(SyncAllSchemas)
        .send()
        .await
        .expect("schema sync results");
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(_, ok)| *ok));
    for host in [&host_a, &host_b] {
        assert_eq!(*host.applied_ddl.lock().expect("ddl lock"), vec!["init"]);
    }
}
