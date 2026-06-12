//! Domain-neutral scatter-gather planning for federated query execution.
//!
//! A federated query runs in two stages: a *scatter* statement executed on
//! every participating node, and a *gather* statement that merges the
//! collected partial results. Aggregates are decomposed into partial+merge
//! pairs (`SUM`→`SUM`/`SUM`, `COUNT`→`COUNT`/`SUM`, `AVG`→`SUM`+`COUNT` /
//! `SUM÷SUM`) so the merge stage combines node-local partials correctly.

mod aggregate;
mod plans;
mod relations;
#[cfg(test)]
mod tests;

use queryfabric_catalog::{BackendAdapter, Catalog, PlanFeatures, SqlArtifact};
use queryfabric_ir::{BoundQuery, QueryFabricError, ResultSchema};

pub use plans::{build_federated_aggregate_plan, build_federated_passthrough_plan};
pub use relations::{from_target, primary_relation_binding, validate_federation_shape};

/// Placeholder the executor substitutes with the gathered partial results.
pub const PARTIALS_PLACEHOLDER: &str = "{partials}";
/// Relation name bound to the partials placeholder in the merge stage.
pub const PARTIALS_RELATION: &str = "partials";

/// Errors raised while planning a federated (scatter-gather) execution.
#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    #[error("unsupported for federated execution: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Compile(#[from] QueryFabricError),
}

pub type Result<T> = std::result::Result<T, FederationError>;

/// Two-stage execution plan produced by the federation planner.
///
/// `scatter_sql` runs unchanged on every node; `gather_sql` merges the
/// node-local partials and contains [`PARTIALS_PLACEHOLDER`] where the
/// executor injects the gathered relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScatterGatherPlan {
    pub scatter_sql: String,
    pub gather_sql: String,
    /// Display name of the base relation the scatter stage reads from.
    pub from_target: String,
    pub result_schema: ResultSchema,
}

/// Gather statement for single-node passthrough: the remote node executes
/// the full query and the gather stage forwards its result unchanged.
pub fn passthrough_gather_sql() -> String {
    format!("SELECT * FROM ({PARTIALS_PLACEHOLDER})")
}

/// Validate the query shape and build the appropriate two-stage plan for
/// multi-node execution: aggregate decomposition when the plan aggregates,
/// order/limit-stripping passthrough otherwise.
pub fn build_scatter_gather_plan(
    adapter: &dyn BackendAdapter,
    catalog: &dyn Catalog,
    bound: &BoundQuery,
    features: &PlanFeatures,
    artifact: &SqlArtifact,
) -> Result<ScatterGatherPlan> {
    validate_federation_shape(features, bound)?;
    if features.has_aggregates {
        build_federated_aggregate_plan(adapter, catalog, bound, artifact)
    } else {
        build_federated_passthrough_plan(adapter, catalog, bound, artifact)
    }
}
