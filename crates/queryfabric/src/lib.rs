//! Stable public facade for QueryFabric.
//!
//! ```rust
//! use queryfabric::{
//!     bind_and_validate_query, ClickHouseAdapter, ColumnSchema, GenericSqlDialect, MemoryCatalog,
//!     QueryCompiler, QueryParameters, RelationKind, RelationSchema, DataType,
//! };
//!
//! let dialect = GenericSqlDialect;
//! let compiler = QueryCompiler::default();
//! let parsed = compiler.parse(&dialect, "SELECT record_id FROM records LIMIT 5").unwrap();
//!
//! let mut catalog = MemoryCatalog::default();
//! catalog.register_relation(RelationSchema {
//!     namespace: None,
//!     name: "records".into(),
//!     aliases: Vec::new(),
//!     kind: RelationKind::Table,
//!     columns: vec![ColumnSchema {
//!         name: "record_id".into(),
//!         data_type: DataType::Uuid,
//!         nullable: false,
//!         metadata: Default::default(),
//!     }],
//!     metadata: Default::default(),
//! });
//!
//! let bound = bind_and_validate_query(&parsed, &catalog, &QueryParameters::default()).unwrap();
//! let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog).unwrap();
//! let sql = artifact.as_sql().unwrap();
//! assert!(sql.text.contains("FROM records"));
//! assert!(sql.text.contains("record_id"));
//! ```

use serde::{Deserialize, Serialize};

mod inspect;

pub use self::inspect::{
    ParsedQuerySummary, build_query_parameters, inspect_parameters, inspect_query,
    parameter_value_from_json,
};
pub use queryfabric_adapter_clickhouse::ClickHouseAdapter;
pub use queryfabric_adapter_postgres::PostgresAdapter;
pub use queryfabric_catalog::{
    BackendAdapter, BackendAnalysis, BackendExecutionLimits, BackendFeature,
    BackendFunctionMapping, CapabilitySet, Catalog, CatalogDocument, ColumnSchema,
    CostEstimateError, DefaultQueryCostModel, EmitArtifact, EstimatedCost, EstimatedCostClass,
    FunctionKind, FunctionRegistry, FunctionSignature, FunctionVolatility, MemoryCatalog,
    OpaqueArtifact, PlanCostEstimator, PlanFeatures, QueryCostEstimate, QueryCostInput,
    QueryCostModel, QueryTimeoutClass, RelationKind, RelationSchema, RelationStatistics,
    ResultDeliveryDescriptor, ResultDeliveryFormat, ResultDeliveryMode, SqlArtifact,
    TypeCoercionRule, infer_result_schema, inspect_plan,
};
pub use queryfabric_dialect_sql::{GenericSqlDialect, parse_sql_query};
pub use queryfabric_dialect_syql::{SyqlDialect, parse_syql};
#[doc(hidden)]
pub use queryfabric_ir::{
    BackendClause, BinaryOperator, BoundColumnRef, BoundExpr, BoundExprKind, BoundFunctionCall,
    BoundProjectionExpr, BoundProjectionItem, BoundQueryPlan, BoundRelation, BoundRelationBinding,
    BoundSelect, BoundSetExpr, BoundTableWithJoins, JoinKind, LiteralValue, NameRef, SyntaxCte,
    SyntaxExpr, SyntaxExprKind, SyntaxFunctionCall, SyntaxJoin, SyntaxNode, SyntaxOrderByExpr,
    SyntaxProjectionExpr, SyntaxProjectionItem, SyntaxQuery, SyntaxRelation, SyntaxSelect,
    SyntaxSetExpr, SyntaxTableWithJoins, SyntaxWhenThen, UnaryOperator, WindowSpec,
};
pub use queryfabric_ir::{
    BoundQuery, CapabilityRequirement, CapabilityRequirements, CatalogSnapshotId, DataType,
    DiagnosticSeverity, Dialect, DialectMetadata, FieldMetadata, FunctionRef, ParameterBinding,
    ParameterRef, ParameterSchema, ParameterSummary, ParameterValue, ParsedQuery,
    ProvenanceReceipt, QueryDiagnostic, QueryFabricError, QueryParameters, QuerySourceSpan, Result,
    ResultField, ResultSchema,
};
pub use queryfabric_opt::{IdentityPass, OptimizationPass, OptimizationPipeline, RewriteAdvisory};
pub use queryfabric_runtime::{
    DriverError, ExecutionRuntime, ExecutionRuntimeMode, InteractiveRuntime,
    IsolatedExecutionDriver, IsolatedJobSpec, ObjectStoreFormat, RecordBatchStream,
    ResourceRequest, RuntimeError, StorageAccessMode,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilityManifest {
    pub backend: String,
    pub schema_version: u32,
    pub capabilities: CapabilitySet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility_family: Option<String>,
}

/// Stable facade over parse/bind/normalize/analyze/emit.
#[derive(Default)]
pub struct QueryCompiler {
    optimization_pipeline: OptimizationPipeline,
}

impl QueryCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_optimization_pass(mut self, pass: impl OptimizationPass + 'static) -> Self {
        self.optimization_pipeline =
            std::mem::take(&mut self.optimization_pipeline).with_pass(pass);
        self
    }

    pub fn parse(&self, dialect: &dyn Dialect, input: &str) -> Result<ParsedQuery> {
        dialect.parse(input)
    }

    pub fn bind_and_validate(
        &self,
        parsed: &ParsedQuery,
        catalog: &dyn Catalog,
        parameters: &QueryParameters,
    ) -> Result<BoundQuery> {
        let compiler_version = env!("CARGO_PKG_VERSION");
        let bound = match queryfabric_catalog::bind_and_validate(parsed, catalog, parameters) {
            Ok(bound) => bound,
            Err(error) => return Err(error.with_compiler_version(compiler_version)),
        };
        let normalized = self.optimization_pipeline.normalize(bound, catalog)?;
        let provenance = normalized
            .provenance()
            .clone()
            .with_compiler_version(compiler_version);
        Ok(normalized.with_provenance(provenance))
    }

    pub fn analyze(
        &self,
        query: &BoundQuery,
        adapter: &dyn BackendAdapter,
        catalog: &dyn Catalog,
    ) -> BackendAnalysis {
        adapter.analyze(query, catalog)
    }

    pub fn emit(
        &self,
        query: &BoundQuery,
        adapter: &dyn BackendAdapter,
        catalog: &dyn Catalog,
    ) -> Result<EmitArtifact> {
        adapter.emit(query, catalog)
    }
}

pub fn bind_and_validate(
    parsed: &ParsedQuery,
    catalog: &dyn Catalog,
    parameters: &QueryParameters,
) -> Result<BoundQuery> {
    QueryCompiler::default().bind_and_validate(parsed, catalog, parameters)
}

pub fn bind_and_validate_query(
    parsed: &ParsedQuery,
    catalog: &dyn Catalog,
    parameters: &QueryParameters,
) -> Result<BoundQuery> {
    bind_and_validate(parsed, catalog, parameters)
}

/// Build the standard portable test catalog used across QueryFabric tests.
///
/// The catalog contains `records` and `links` with the canonical portable
/// schema expected by the integration tests, fuzz target, and Python binding.
pub fn portable_catalog(snapshot_id: impl Into<String>) -> MemoryCatalog {
    let mut catalog = MemoryCatalog::default();
    catalog.set_snapshot_id(snapshot_id);
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "records".into(),
        aliases: vec!["r".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "score".into(),
                data_type: DataType::Float64,
                nullable: true,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "links".into(),
        aliases: vec!["l".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "source_record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "target_record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "weight".into(),
                data_type: DataType::Float64,
                nullable: false,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });
    catalog
}

pub fn builtin_capability_manifest() -> Vec<BackendCapabilityManifest> {
    vec![
        BackendCapabilityManifest {
            backend: "clickhouse".into(),
            schema_version: 1,
            capabilities: ClickHouseAdapter.capabilities(),
            compatibility_family: Some("sql".into()),
        },
        BackendCapabilityManifest {
            backend: "postgres".into(),
            schema_version: 1,
            capabilities: PostgresAdapter.capabilities(),
            compatibility_family: Some("sql".into()),
        },
    ]
}
