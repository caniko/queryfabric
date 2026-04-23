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
//! let parsed = compiler.parse(&dialect, "SELECT neuron_id FROM neurons LIMIT 5").unwrap();
//!
//! let mut catalog = MemoryCatalog::default();
//! catalog.register_relation(RelationSchema {
//!     namespace: None,
//!     name: "neurons".into(),
//!     aliases: Vec::new(),
//!     kind: RelationKind::Table,
//!     columns: vec![ColumnSchema {
//!         name: "neuron_id".into(),
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
//! assert!(sql.text.contains("FROM neurons"));
//! assert!(sql.text.contains("neuron_id"));
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
    BackendAdapter, BackendAnalysis, BackendFeature, BackendFunctionMapping, CapabilitySet,
    Catalog, CatalogDocument, ColumnSchema, EmitArtifact, EstimatedCostClass, FunctionKind,
    FunctionRegistry, FunctionSignature, FunctionVolatility, MemoryCatalog, OpaqueArtifact,
    PlanFeatures, RelationKind, RelationSchema, SqlArtifact, TypeCoercionRule, infer_result_schema,
    inspect_plan,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilityManifest {
    pub backend: String,
    pub capabilities: CapabilitySet,
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
/// The catalog contains `neurons` and `synapses` with the canonical portable
/// schema expected by the integration tests, fuzz target, and Python binding.
pub fn portable_catalog(snapshot_id: impl Into<String>) -> MemoryCatalog {
    let mut catalog = MemoryCatalog::default();
    catalog.set_snapshot_id(snapshot_id);
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "neurons".into(),
        aliases: vec!["n".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "neuron_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "cable_length".into(),
                data_type: DataType::Float64,
                nullable: true,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "synapses".into(),
        aliases: vec!["s".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "source_neuron_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "target_neuron_id".into(),
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
            capabilities: ClickHouseAdapter.capabilities(),
        },
        BackendCapabilityManifest {
            backend: "postgres".into(),
            capabilities: PostgresAdapter.capabilities(),
        },
    ]
}
