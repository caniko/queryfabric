use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque identity of a queryable resource.
///
/// The host decides what a namespace and a resource are; QueryFabric only
/// requires that the pair is globally unique. Distinct from [`NodeId`] so the
/// two cannot be mixed even though both are UUID-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceRef {
    /// Grouping scope the resource lives in (e.g. a tenant or collection).
    pub namespace: Uuid,
    /// Identity of the resource within its namespace.
    pub id: Uuid,
}

impl ResourceRef {
    pub fn new(namespace: Uuid, id: Uuid) -> Self {
        Self { namespace, id }
    }
}

/// Identity of a node participating in a federation cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl From<Uuid> for NodeId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}
