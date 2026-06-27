use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use chrono::Utc;
use queryfabric_contract::Health;
use piying::Actor;
use piying::actor::ActorRef;
use piying::error::Infallible;
use piying::message::{Context, Message};
use tracing::info;

use crate::host::FederationHost;
use crate::messages::{
    CatalogRequest, CatalogResponse, ClusterIdentity, FlightEndpointReply, GetFlightEndpoint,
    HealthPing, HealthPong,
};
use crate::schema::{SchemaSync, SchemaSyncReply, apply_schema_sync};

/// Cluster-node-side actor skeleton.
///
/// Runs on each cluster node and answers the hub's control-plane messages:
/// `HealthPing`, `SchemaSync`, `CatalogRequest`, and `GetFlightEndpoint`.
/// All domain behaviour (storage checks, DDL application, catalog
/// enumeration) is delegated to the [`FederationHost`].
///
/// The skeleton is a local actor; remote (libp2p DHT) registration is the
/// host's responsibility because `RemoteActor` derivation requires a
/// concrete type.
pub struct ClusterNodeActor<H: FederationHost> {
    /// Identity this node presented at registration.
    pub identity: ClusterIdentity,
    host: Arc<H>,
    flight_endpoint: Option<String>,
    flight_tls: bool,
    schema_version: AtomicI32,
    started_at: Instant,
}

/// Arguments for spawning a [`ClusterNodeActor`].
pub struct ClusterNodeArgs<H> {
    /// Identity this node presents to the hub.
    pub identity: ClusterIdentity,
    /// Host implementation providing domain behaviour.
    pub host: Arc<H>,
    /// Internal Flight endpoint (`host:port`) for data-plane delegation.
    pub flight_endpoint: Option<String>,
    /// Whether the Flight endpoint expects TLS.
    pub flight_tls: bool,
    /// Schema version the node starts at.
    pub schema_version: i32,
}

impl<H: FederationHost> Actor for ClusterNodeActor<H> {
    type Args = ClusterNodeArgs<H>;
    type Error = Infallible;

    async fn on_start(args: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        info!(
            cluster = %args.identity.name,
            schema_version = args.schema_version,
            "ClusterNodeActor started"
        );
        Ok(Self {
            identity: args.identity,
            host: args.host,
            flight_endpoint: args.flight_endpoint,
            flight_tls: args.flight_tls,
            schema_version: AtomicI32::new(args.schema_version),
            started_at: Instant::now(),
        })
    }
}

impl<H: FederationHost> Message<HealthPing> for ClusterNodeActor<H> {
    type Reply = HealthPong;

    async fn handle(
        &mut self,
        _msg: HealthPing,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let storage_ok = self.host.storage_ok().await;
        let status = if storage_ok {
            Health::Healthy
        } else {
            Health::Degraded
        };
        let resource_count = if storage_ok {
            self.host.resource_count().await
        } else {
            0
        };

        HealthPong {
            status,
            schema_version: self.schema_version.load(Ordering::Relaxed),
            resource_count,
            uptime_secs: self.started_at.elapsed().as_secs(),
            storage_ok,
            host_revision: self.host.host_revision(),
        }
    }
}

impl<H: FederationHost> Message<SchemaSync> for ClusterNodeActor<H> {
    type Reply = SchemaSyncReply;

    async fn handle(
        &mut self,
        msg: SchemaSync,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        apply_schema_sync(self.host.as_ref(), &self.schema_version, &msg).await
    }
}

impl<H: FederationHost> Message<CatalogRequest> for ClusterNodeActor<H> {
    type Reply = CatalogResponse<H::CatalogEntry>;

    async fn handle(
        &mut self,
        msg: CatalogRequest,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        CatalogResponse {
            resources: self.host.catalog(msg.since).await,
            as_of: Utc::now(),
        }
    }
}

impl<H: FederationHost> Message<GetFlightEndpoint> for ClusterNodeActor<H> {
    type Reply = FlightEndpointReply;

    async fn handle(
        &mut self,
        _msg: GetFlightEndpoint,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        FlightEndpointReply {
            endpoint: self.flight_endpoint.clone().unwrap_or_default(),
            tls: self.flight_tls,
        }
    }
}
