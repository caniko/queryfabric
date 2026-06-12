use async_trait::async_trait;
use chrono::{DateTime, Utc};
use queryfabric_contract::NodeId;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::messages::{ClusterIdentity, ResourceAnnouncement};
use crate::schema::SchemaMigration;

/// Successful registration result produced by the host.
#[derive(Debug, Clone)]
pub struct ClusterRegistration {
    /// Node id assigned to the cluster.
    pub cluster_id: NodeId,
    /// API key for subsequent control-plane calls.
    pub api_key: String,
    /// Human-readable status message.
    pub message: String,
}

/// The single seam through which a host injects domain behaviour into the
/// federation actors.
///
/// Hub-side hooks: [`register_cluster`](Self::register_cluster),
/// [`on_announce`](Self::on_announce),
/// [`schema_version`](Self::schema_version),
/// [`schema_migrations`](Self::schema_migrations).
/// Cluster-node-side hooks: [`storage_ok`](Self::storage_ok),
/// [`resource_count`](Self::resource_count), [`catalog`](Self::catalog),
/// [`apply_ddl`](Self::apply_ddl), [`host_revision`](Self::host_revision).
///
/// A host that only runs one side may leave the other side's methods at
/// their defaults (deny registration / no-op).
#[async_trait]
pub trait FederationHost: Send + Sync + 'static {
    /// Catalog entry type carried in announcements and catalog replies.
    type CatalogEntry: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    // -- Hub side ------------------------------------------------------

    /// Validate and persist a cluster registration.
    ///
    /// # Errors
    /// Returns a human-readable reason when the registration is rejected.
    async fn register_cluster(
        &self,
        _identity: &ClusterIdentity,
        _federation_password: &str,
    ) -> Result<ClusterRegistration, String> {
        Err("registration not supported by this host".to_owned())
    }

    /// Hook invoked after the locality index has been updated for an
    /// announcement (persistence, search indexing, ...). Best-effort.
    async fn on_announce(&self, _announcement: &ResourceAnnouncement<Self::CatalogEntry>) {}

    /// Current federation schema version.
    fn schema_version(&self) -> i32 {
        0
    }

    /// Migrations needed to go from `from_version` to
    /// [`schema_version`](Self::schema_version).
    fn schema_migrations(&self, _from_version: i32) -> Vec<SchemaMigration> {
        Vec::new()
    }

    // -- Cluster-node side ---------------------------------------------

    /// Whether the node's backing storage passes its own health checks.
    async fn storage_ok(&self) -> bool {
        true
    }

    /// Number of resources this node currently serves.
    async fn resource_count(&self) -> u64 {
        0
    }

    /// Local catalog entries, optionally restricted to changes since
    /// `since`.
    async fn catalog(&self, _since: Option<DateTime<Utc>>) -> Vec<Self::CatalogEntry> {
        Vec::new()
    }

    /// Apply one schema migration's opaque DDL body to local storage.
    ///
    /// # Errors
    /// Returns a human-readable reason when the migration fails.
    async fn apply_ddl(&self, _migration: &SchemaMigration) -> Result<(), String> {
        Ok(())
    }

    /// Host-defined auxiliary revision reported in health pongs (e.g. a
    /// domain vocabulary version).
    fn host_revision(&self) -> i64 {
        0
    }
}
