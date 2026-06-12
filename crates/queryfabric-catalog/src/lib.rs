mod bind;
mod builtins;
mod features;
mod model;
mod render;
mod stats_source;

pub use bind::{bind_and_validate, infer_result_schema, unsupported};
pub use features::{PlanFeatures, inspect_plan};
pub use model::{
    BackendAdapter, BackendAnalysis, BackendExecutionLimits, BackendFeature,
    BackendFunctionMapping, CapabilitySet, Catalog, CatalogDocument, ColumnSchema,
    CostEstimateError, DefaultQueryCostModel, EmitArtifact, EstimatedCost, EstimatedCostClass,
    FunctionKind, FunctionRegistry, FunctionSignature, FunctionVolatility, MemoryCatalog,
    OpaqueArtifact, PlanCostEstimator, QueryCostEstimate, QueryCostInput, QueryCostModel,
    QueryTimeoutClass, RelationKind, RelationSchema, RelationStatistics, ResultDeliveryDescriptor,
    ResultDeliveryFormat, ResultDeliveryMode, SqlArtifact, TypeCoercionRule,
};
pub use render::{SqlBackend, analyze_backend_support, emit_sql_artifact};
pub use stats_source::relation_statistics_from_source;
