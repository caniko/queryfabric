use std::sync::Arc;

use async_trait::async_trait;
use queryfabric_cluster::{DhtNaming, HealthMonitorActor, HubRegistryState, ResetCircuitBreaker};
use queryfabric_contract::{ClusterProbe, NodeId, ResourceRef};
use thespis::Actor;
use thespis::actor::ActorRef;
use thespis::error::Infallible;
use thespis::message::{Context, Message};
use tracing::{info, warn};

use crate::host::FederationHost;
use crate::locality::{ClusterRemoteHandle, ResourceLocation};
use crate::messages::{
    RegisterCluster, RegisterClusterReply, ResourceAction, ResourceAnnouncement, SyncAllSchemas,
};
use crate::schema::SchemaSync;
use crate::transport::FederationTransport;

/// Hook for resetting a node's circuit breaker when it (re-)registers, so
/// recovered nodes are not stuck in the Open state.
#[async_trait]
pub trait CircuitResetHook: Send + Sync + 'static {
    /// Reset the circuit breaker for `node`.
    async fn reset_circuit(&self, node: NodeId);
}

#[async_trait]
impl<P> CircuitResetHook for ActorRef<HealthMonitorActor<NodeId, P>>
where
    P: ClusterProbe<NodeId, ClusterRemoteHandle>,
{
    async fn reset_circuit(&self, node: NodeId) {
        if let Err(e) = self.tell(ResetCircuitBreaker(node)).send().await {
            warn!(error = %e, node = ?node, "Failed to reset circuit breaker");
        }
    }
}

/// Hub-side actor skeleton.
///
/// Handles cluster registration, resource announcements, and schema-sync
/// broadcasts. Registration validation/persistence and announcement side
/// effects are delegated to the [`FederationHost`]; locality bookkeeping
/// happens here so routing state stays consistent regardless of host
/// behaviour.
pub struct HubActor<H: FederationHost, T: FederationTransport> {
    host: Arc<H>,
    registry: HubRegistryState<ResourceRef, NodeId>,
    transport: Arc<T>,
    naming: DhtNaming,
    circuit_reset: Option<Box<dyn CircuitResetHook>>,
}

/// Arguments for spawning a [`HubActor`].
pub struct HubActorArgs<H, T> {
    /// Host implementation providing domain behaviour.
    pub host: Arc<H>,
    /// Shared cluster/resource registry state.
    pub registry: HubRegistryState<ResourceRef, NodeId>,
    /// Transport used for schema-sync broadcasts.
    pub transport: Arc<T>,
    /// DHT naming scheme used to derive per-cluster actor names.
    pub naming: DhtNaming,
    /// Optional circuit-breaker reset hook (usually the health monitor's
    /// actor ref).
    pub circuit_reset: Option<Box<dyn CircuitResetHook>>,
}

impl<H: FederationHost, T: FederationTransport> Actor for HubActor<H, T> {
    type Args = HubActorArgs<H, T>;
    type Error = Infallible;

    async fn on_start(args: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        info!("HubActor started");
        Ok(Self {
            host: args.host,
            registry: args.registry,
            transport: args.transport,
            naming: args.naming,
            circuit_reset: args.circuit_reset,
        })
    }
}

impl<H: FederationHost, T: FederationTransport> Message<RegisterCluster> for HubActor<H, T> {
    type Reply = RegisterClusterReply;

    async fn handle(
        &mut self,
        msg: RegisterCluster,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .host
            .register_cluster(&msg.identity, &msg.federation_password)
            .await
        {
            Ok(registration) => {
                // Store the remote handle so the health monitor can find
                // this cluster. The Flight endpoint is populated on the
                // first health sweep.
                self.registry.upsert_cluster(ClusterRemoteHandle {
                    cluster_id: registration.cluster_id,
                    cluster_name: msg.identity.name.clone(),
                    dht_name: self.naming.cluster_name(&msg.identity.name),
                    flight_endpoint: None,
                    flight_tls: false,
                });

                if let Some(ref hook) = self.circuit_reset {
                    hook.reset_circuit(registration.cluster_id).await;
                }

                info!(
                    cluster = %msg.identity.name,
                    id = ?registration.cluster_id,
                    "Cluster registered"
                );

                RegisterClusterReply {
                    cluster_id: registration.cluster_id,
                    api_key: registration.api_key,
                    schema_version: self.host.schema_version(),
                    schema_ddl: self.host.schema_migrations(0),
                    accepted: true,
                    message: registration.message,
                }
            }
            Err(e) => {
                warn!(error = %e, cluster = %msg.identity.name, "Cluster registration failed");
                RegisterClusterReply {
                    cluster_id: NodeId::from(uuid::Uuid::nil()),
                    api_key: String::new(),
                    schema_version: 0,
                    schema_ddl: Vec::new(),
                    accepted: false,
                    message: format!("Registration failed: {e}"),
                }
            }
        }
    }
}

impl<H: FederationHost, T: FederationTransport> Message<ResourceAnnouncement<H::CatalogEntry>>
    for HubActor<H, T>
{
    type Reply = ();

    async fn handle(
        &mut self,
        msg: ResourceAnnouncement<H::CatalogEntry>,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        info!(
            cluster_id = ?msg.cluster_id,
            resource_id = ?msg.resource_id,
            action = ?msg.action,
            "Received resource announcement"
        );

        match msg.action {
            ResourceAction::Added | ResourceAction::Updated => {
                self.registry.upsert_resource(
                    msg.resource_id,
                    ResourceLocation {
                        cluster_id: msg.cluster_id,
                        tables: msg.facets.clone(),
                    },
                );
            }
            ResourceAction::Removed => {
                self.registry.remove_resource(&msg.resource_id);
            }
        }

        // Host side effects (persistence, search indexing) are best-effort.
        self.host.on_announce(&msg).await;
    }
}

impl<H: FederationHost, T: FederationTransport> Message<SyncAllSchemas> for HubActor<H, T> {
    type Reply = Vec<(String, bool)>;

    async fn handle(
        &mut self,
        _msg: SyncAllSchemas,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let migrations = self.host.schema_migrations(0);
        let target_version = self.host.schema_version();
        let mut results = Vec::new();

        let entries: Vec<_> = {
            let guard = self.registry.cluster_refs().guard();
            self.registry
                .cluster_refs()
                .iter(&guard)
                .map(|(_, handle)| handle.clone())
                .collect()
        };

        for handle in entries {
            let sync = SchemaSync {
                target_version,
                migrations: migrations.clone(),
            };
            match self.transport.schema_sync(&handle, sync).await {
                Ok(reply) => results.push((handle.cluster_name.clone(), reply.success)),
                Err(e) => {
                    warn!(
                        cluster = %handle.cluster_name,
                        error = %e,
                        "Schema sync failed"
                    );
                    results.push((handle.cluster_name.clone(), false));
                }
            }
        }

        results
    }
}
