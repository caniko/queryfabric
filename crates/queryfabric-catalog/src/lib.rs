mod bind;
mod builtins;
mod features;
mod model;
mod render;

pub use bind::{bind_and_validate, infer_result_schema, unsupported};
pub use features::{PlanFeatures, inspect_plan};
pub use model::{
    BackendAdapter, BackendAnalysis, BackendFeature, BackendFunctionMapping, CapabilitySet,
    Catalog, CatalogDocument, ColumnSchema, EmitArtifact, EstimatedCostClass, FunctionKind,
    FunctionRegistry, FunctionSignature, FunctionVolatility, MemoryCatalog, OpaqueArtifact,
    RelationKind, RelationSchema, SqlArtifact, TypeCoercionRule,
};
pub use render::{SqlBackend, analyze_backend_support, emit_sql_artifact};
