use std::collections::{BTreeMap, BTreeSet};

use queryfabric_ir::{
    BoundQuery, CatalogSnapshotId, DataType, DiagnosticSeverity, ParameterSchema,
    ProvenanceReceipt, QueryDiagnostic, Result, ResultField, ResultSchema,
};
use serde::{Deserialize, Serialize};

use crate::builtins::{builtin_function_signature, portable_builtin_functions};
use crate::features::PlanFeatures;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    Table,
    View,
    MaterializedView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    #[serde(
        default,
        skip_serializing_if = "queryfabric_ir::FieldMetadata::is_empty"
    )]
    pub metadata: queryfabric_ir::FieldMetadata,
}

impl ColumnSchema {
    pub fn to_result_field(&self) -> ResultField {
        ResultField {
            name: self.name.clone(),
            data_type: self.data_type.clone(),
            nullable: self.nullable,
            metadata: self.metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub kind: RelationKind,
    pub columns: Vec<ColumnSchema>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl RelationSchema {
    pub fn to_result_schema(&self) -> ResultSchema {
        ResultSchema::new(
            self.columns
                .iter()
                .map(ColumnSchema::to_result_field)
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogDocument {
    pub snapshot_id: CatalogSnapshotId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RelationSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<FunctionSignature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionKind {
    Scalar,
    Aggregate,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionVolatility {
    Immutable,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeCoercionRule {
    pub from: DataType,
    pub to: DataType,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub implicit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendFunctionMapping {
    pub backend: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
}

impl BackendFunctionMapping {
    pub fn as_function_ref(&self) -> queryfabric_ir::FunctionRef {
        queryfabric_ir::FunctionRef {
            namespace: self.namespace.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
    pub kind: FunctionKind,
    pub volatility: FunctionVolatility,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arg_types: Vec<DataType>,
    pub return_type: DataType,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub variadic: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coercions: Vec<TypeCoercionRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backend_mappings: Vec<BackendFunctionMapping>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl FunctionSignature {
    pub fn function_ref(&self) -> queryfabric_ir::FunctionRef {
        queryfabric_ir::FunctionRef {
            namespace: self.namespace.clone(),
            name: self.name.clone(),
        }
    }

    pub fn backend_mapping(&self, backend: &str) -> Option<queryfabric_ir::FunctionRef> {
        self.backend_mappings
            .iter()
            .find(|mapping| mapping.backend == backend)
            .map(BackendFunctionMapping::as_function_ref)
    }

    pub fn metadata_flag(&self, key: &str) -> bool {
        self.metadata.get(key).is_some_and(|value| value == "true")
    }
}

pub trait FunctionRegistry: Send + Sync {
    fn resolve_function(&self, namespace: Option<&str>, name: &str) -> Option<FunctionSignature>;
    fn functions(&self) -> Vec<FunctionSignature>;
}

pub trait Catalog: FunctionRegistry + Send + Sync {
    fn snapshot_id(&self) -> CatalogSnapshotId;
    fn resolve_relation(&self, namespace: Option<&str>, name: &str) -> Option<RelationSchema>;
    fn relations(&self) -> Vec<RelationSchema>;

    fn relation_statistics(
        &self,
        namespace: Option<&str>,
        name: &str,
    ) -> Option<RelationStatistics> {
        relation_statistics_from_schema(&self.resolve_relation(namespace, name)?)
    }
}

#[derive(Debug, Clone)]
pub struct MemoryCatalog {
    snapshot_id: CatalogSnapshotId,
    relations: BTreeMap<(Option<String>, String), RelationSchema>,
    functions: BTreeMap<(Option<String>, String), FunctionSignature>,
    statistics: BTreeMap<(Option<String>, String), RelationStatistics>,
}

impl Default for MemoryCatalog {
    fn default() -> Self {
        Self {
            snapshot_id: CatalogSnapshotId("memory-catalog".into()),
            relations: BTreeMap::new(),
            functions: BTreeMap::new(),
            statistics: BTreeMap::new(),
        }
    }
}

impl MemoryCatalog {
    pub fn set_snapshot_id(&mut self, snapshot_id: impl Into<String>) {
        self.snapshot_id = CatalogSnapshotId(snapshot_id.into());
    }

    pub fn to_document(&self) -> CatalogDocument {
        CatalogDocument {
            snapshot_id: self.snapshot_id(),
            relations: self.relations(),
            functions: self.functions(),
        }
    }

    pub fn from_document(document: CatalogDocument) -> Self {
        let mut catalog = Self {
            snapshot_id: document.snapshot_id,
            relations: BTreeMap::new(),
            functions: BTreeMap::new(),
            statistics: BTreeMap::new(),
        };

        for relation in document.relations {
            catalog.register_relation(relation);
        }

        for signature in document.functions {
            catalog.register_function(signature);
        }

        catalog
    }

    pub fn register_relation(&mut self, relation: RelationSchema) {
        self.relations.insert(
            (
                relation.namespace.clone(),
                relation.name.to_ascii_lowercase(),
            ),
            relation,
        );
    }

    /// Inject live statistics for a relation, overriding any estimate derived
    /// from schema metadata. This is the host-facing cost hook: refresh these
    /// whenever the backing store changes and the cost model picks them up on
    /// the next estimate.
    pub fn set_relation_statistics(
        &mut self,
        namespace: Option<&str>,
        name: &str,
        statistics: RelationStatistics,
    ) {
        let key = self
            .resolve_relation(namespace, name)
            .map(|relation| {
                (
                    relation.namespace.clone(),
                    relation.name.to_ascii_lowercase(),
                )
            })
            .unwrap_or_else(|| (namespace.map(str::to_owned), name.to_ascii_lowercase()));
        self.statistics.insert(key, statistics);
    }

    pub fn register_function(&mut self, signature: FunctionSignature) {
        self.functions.insert(
            (
                signature.namespace.clone(),
                signature.name.to_ascii_lowercase(),
            ),
            signature,
        );
    }
}

impl FunctionRegistry for MemoryCatalog {
    fn resolve_function(&self, namespace: Option<&str>, name: &str) -> Option<FunctionSignature> {
        self.functions
            .get(&(namespace.map(str::to_owned), name.to_ascii_lowercase()))
            .cloned()
            .or_else(|| builtin_function_signature(namespace, name))
    }

    fn functions(&self) -> Vec<FunctionSignature> {
        let mut values: BTreeMap<(Option<String>, String), FunctionSignature> =
            portable_builtin_functions()
                .into_iter()
                .map(|signature| {
                    (
                        (
                            signature.namespace.clone(),
                            signature.name.to_ascii_lowercase(),
                        ),
                        signature,
                    )
                })
                .collect();
        for (key, signature) in &self.functions {
            values.insert(key.clone(), signature.clone());
        }
        values.into_values().collect()
    }
}

impl Catalog for MemoryCatalog {
    fn snapshot_id(&self) -> CatalogSnapshotId {
        self.snapshot_id.clone()
    }

    fn resolve_relation(&self, namespace: Option<&str>, name: &str) -> Option<RelationSchema> {
        let key = (namespace.map(str::to_owned), name.to_ascii_lowercase());
        if let Some(relation) = self.relations.get(&key) {
            return Some(relation.clone());
        }

        self.relations
            .values()
            .find(|relation| {
                relation.name.eq_ignore_ascii_case(name)
                    || relation
                        .aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(name))
            })
            .cloned()
    }

    fn relations(&self) -> Vec<RelationSchema> {
        self.relations.values().cloned().collect()
    }

    fn relation_statistics(
        &self,
        namespace: Option<&str>,
        name: &str,
    ) -> Option<RelationStatistics> {
        let relation = self.resolve_relation(namespace, name)?;
        let key = (
            relation.namespace.clone(),
            relation.name.to_ascii_lowercase(),
        );
        if let Some(statistics) = self.statistics.get(&key) {
            return Some(statistics.clone());
        }
        relation_statistics_from_schema(&relation)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackendFeature, Catalog, CatalogDocument, ColumnSchema, DataType, FunctionKind,
        FunctionSignature, FunctionVolatility, MemoryCatalog, RelationKind, RelationSchema,
    };

    #[test]
    fn catalog_document_roundtrip_preserves_snapshot_relations_and_functions() {
        let mut catalog = MemoryCatalog::default();
        catalog.set_snapshot_id("catalog-doc-test");
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "neurons".into(),
            aliases: vec!["n".into()],
            kind: RelationKind::Table,
            columns: vec![ColumnSchema {
                name: "neuron_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            }],
            metadata: Default::default(),
        });
        catalog.register_function(FunctionSignature {
            namespace: Some("host".into()),
            name: "example_fn".into(),
            kind: FunctionKind::Scalar,
            volatility: FunctionVolatility::Immutable,
            arg_types: vec![DataType::Uuid],
            return_type: DataType::Utf8,
            variadic: false,
            coercions: Vec::new(),
            backend_mappings: Vec::new(),
            metadata: Default::default(),
        });

        let document = catalog.to_document();
        assert_eq!(document.snapshot_id.0, "catalog-doc-test");
        assert!(
            document
                .relations
                .iter()
                .any(|relation| relation.name == "neurons")
        );
        assert!(
            document
                .functions
                .iter()
                .any(|function| function.name == "example_fn")
        );

        let reloaded = MemoryCatalog::from_document(document.clone());
        let reloaded_document: CatalogDocument = reloaded.to_document();

        assert_eq!(reloaded.snapshot_id().0, "catalog-doc-test");
        assert_eq!(reloaded_document, document);
    }

    #[test]
    fn isolated_execution_feature_serializes_roundtrips() {
        let json = serde_json::to_string(&BackendFeature::IsolatedExecution).expect("serialize");
        assert_eq!(json, "\"IsolatedExecution\"");
        let decoded: BackendFeature = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, BackendFeature::IsolatedExecution);
    }

    #[test]
    fn uuid_arrow_workaround_feature_serializes_roundtrips() {
        let json =
            serde_json::to_string(&BackendFeature::UuidToStringInArrowOutput).expect("serialize");
        assert_eq!(json, "\"UuidToStringInArrowOutput\"");
        let decoded: BackendFeature = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, BackendFeature::UuidToStringInArrowOutput);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BackendFeature {
    CommonTableExpressions,
    DerivedTables,
    Joins,
    Windows,
    SetOperations,
    Aggregates,
    DistinctAggregates,
    ScalarSubqueries,
    InSubqueries,
    NamespacedFunctions,
    ApproximateAggregates,
    Explain,
    LimitOffset,
    IsolatedExecution,
    UuidToStringInArrowOutput,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimatedCost {
    pub memory_bytes: u64,
    pub rows_scanned: u64,
    pub partitions_touched: u32,
    pub wallclock_estimate_ms: u64,
}

pub trait PlanCostEstimator: Send + Sync {
    fn estimate(
        &self,
        plan: &BoundQuery,
        catalog: &dyn Catalog,
    ) -> std::result::Result<EstimatedCost, CostEstimateError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CostEstimateError {
    #[error("estimation unsupported for this backend")]
    Unsupported,
    #[error("missing catalog statistics: {0}")]
    MissingStatistics(String),
    #[error("estimation failed: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub features: BTreeSet<BackendFeature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<BackendExecutionLimits>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub result_formats: BTreeSet<ResultDeliveryFormat>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub async_export: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub federated_execution: bool,
}

impl CapabilitySet {
    pub fn from_features(features: impl IntoIterator<Item = BackendFeature>) -> Self {
        Self {
            features: features.into_iter().collect(),
            ..Self::default()
        }
    }

    pub fn supports(&self, feature: BackendFeature) -> bool {
        self.features.contains(&feature)
    }

    pub fn with_limits(mut self, limits: BackendExecutionLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    pub fn with_result_formats(
        mut self,
        formats: impl IntoIterator<Item = ResultDeliveryFormat>,
    ) -> Self {
        self.result_formats = formats.into_iter().collect();
        self
    }

    pub fn with_async_export(mut self, enabled: bool) -> Self {
        self.async_export = enabled;
        self
    }

    pub fn with_federated_execution(mut self, enabled: bool) -> Self {
        self.federated_execution = enabled;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendExecutionLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes_scanned: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_result_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_queries: Option<u32>,
    pub interactive_byte_limit: u64,
    pub batch_byte_limit: u64,
}

impl Default for BackendExecutionLimits {
    fn default() -> Self {
        Self {
            max_rows: None,
            max_bytes_scanned: None,
            max_result_bytes: None,
            max_concurrent_queries: None,
            interactive_byte_limit: 512 * 1024 * 1024,
            batch_byte_limit: 16 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResultDeliveryFormat {
    ArrowIpc,
    Parquet,
    Csv,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultDeliveryMode {
    InteractiveStream,
    PagedResult,
    AsyncMaterializedExport,
    RejectedOverBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultDeliveryDescriptor {
    pub mode: ResultDeliveryMode,
    pub format: ResultDeliveryFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_uri: Option<String>,
    pub row_count: u64,
    pub byte_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationStatistics {
    pub relation: String,
    pub estimated_rows: u64,
    pub average_row_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard_count: Option<u32>,
}

fn relation_statistics_from_schema(relation: &RelationSchema) -> Option<RelationStatistics> {
    let estimated_rows =
        metadata_u64(&relation.metadata, &["estimated_rows", "row_count", "rows"])?;
    Some(RelationStatistics {
        relation: relation.name.clone(),
        estimated_rows,
        average_row_bytes: metadata_u64(
            &relation.metadata,
            &["average_row_bytes", "avg_row_bytes", "row_bytes"],
        )
        .unwrap_or(64),
        shard_count: metadata_u32(&relation.metadata, &["shard_count", "shards"]),
    })
}

fn metadata_u64(metadata: &BTreeMap<String, String>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| value.parse().ok())
}

fn metadata_u32(metadata: &BTreeMap<String, String>, keys: &[&str]) -> Option<u32> {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| value.parse().ok())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryCostInput {
    pub plan_features: PlanFeatures,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RelationStatistics>,
    pub selected_columns: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_limit: Option<u64>,
    pub estimated_filter_selectivity_ppm: u32,
    pub default_row_bytes: u64,
    pub default_shards: u32,
    pub backend_capabilities: CapabilitySet,
}

impl Default for QueryCostInput {
    fn default() -> Self {
        Self {
            plan_features: PlanFeatures::default(),
            relations: Vec::new(),
            selected_columns: 16,
            row_limit: None,
            estimated_filter_selectivity_ppm: 1_000_000,
            default_row_bytes: 64,
            default_shards: 1,
            backend_capabilities: CapabilitySet::default(),
        }
    }
}

pub trait QueryCostModel: Send + Sync {
    fn estimate(&self, input: &QueryCostInput) -> QueryCostEstimate;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultQueryCostModel;

impl QueryCostModel for DefaultQueryCostModel {
    fn estimate(&self, input: &QueryCostInput) -> QueryCostEstimate {
        let relation_rows = input
            .relations
            .iter()
            .map(|relation| relation.estimated_rows)
            .sum::<u64>()
            .max(input.row_limit.unwrap_or(1_000_000));
        let selected_columns = u64::from(input.selected_columns.max(1));
        let average_row_bytes = input
            .relations
            .iter()
            .map(|relation| relation.average_row_bytes)
            .max()
            .unwrap_or(input.default_row_bytes)
            .max(1);
        let selectivity_ppm = u64::from(input.estimated_filter_selectivity_ppm.clamp(1, 1_000_000));
        let mut estimated_rows = relation_rows.saturating_mul(selectivity_ppm) / 1_000_000;
        if let Some(limit) = input.row_limit {
            estimated_rows = estimated_rows.min(limit);
        }
        estimated_rows = estimated_rows.max(1);

        let estimated_bytes = estimated_rows
            .saturating_mul(selected_columns)
            .saturating_mul(average_row_bytes);
        let estimated_shards = input
            .relations
            .iter()
            .filter_map(|relation| relation.shard_count)
            .max()
            .unwrap_or(input.default_shards)
            .max(1);

        let limits = input
            .backend_capabilities
            .limits
            .clone()
            .unwrap_or_default();
        let mut estimate = QueryCostEstimate::classify(
            estimated_rows,
            estimated_bytes,
            estimated_shards,
            limits.interactive_byte_limit,
            limits.batch_byte_limit,
        );

        if input.plan_features.has_joins || input.plan_features.has_windows {
            estimate.diagnostics.push(
                "plan contains joins/windows; interactive execution should be treated conservatively"
                    .to_owned(),
            );
            if estimate.timeout_class == QueryTimeoutClass::Interactive {
                estimate.timeout_class = QueryTimeoutClass::Batch;
                estimate.execution_mode = ResultDeliveryMode::PagedResult;
                estimate.async_required = true;
            }
        }
        if limits
            .max_bytes_scanned
            .is_some_and(|limit| estimated_bytes > limit)
            || limits.max_rows.is_some_and(|limit| estimated_rows > limit)
        {
            estimate.execution_mode = ResultDeliveryMode::RejectedOverBudget;
            estimate.async_required = true;
            estimate
                .diagnostics
                .push("estimated query exceeds backend execution limits".to_owned());
        }
        estimate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstimatedCostClass {
    Low,
    Medium,
    High,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryTimeoutClass {
    Interactive,
    Batch,
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryCostEstimate {
    pub estimated_rows: u64,
    pub estimated_bytes: u64,
    pub estimated_shards: u32,
    pub timeout_class: QueryTimeoutClass,
    pub execution_mode: ResultDeliveryMode,
    pub async_required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

impl QueryCostEstimate {
    pub fn classify(
        estimated_rows: u64,
        estimated_bytes: u64,
        estimated_shards: u32,
        interactive_byte_limit: u64,
        export_byte_limit: u64,
    ) -> Self {
        let timeout_class = if estimated_bytes <= interactive_byte_limit {
            QueryTimeoutClass::Interactive
        } else if estimated_bytes <= export_byte_limit {
            QueryTimeoutClass::Batch
        } else {
            QueryTimeoutClass::Export
        };
        let execution_mode = match timeout_class {
            QueryTimeoutClass::Interactive => ResultDeliveryMode::InteractiveStream,
            QueryTimeoutClass::Batch => ResultDeliveryMode::PagedResult,
            QueryTimeoutClass::Export => ResultDeliveryMode::AsyncMaterializedExport,
        };
        Self {
            estimated_rows,
            estimated_bytes,
            estimated_shards,
            timeout_class,
            execution_mode,
            async_required: !matches!(timeout_class, QueryTimeoutClass::Interactive),
            diagnostics: Vec::new(),
        }
    }
}

#[cfg(test)]
mod cost_tests {
    use super::*;

    #[test]
    fn query_cost_classification_tracks_async_boundary() {
        let interactive = QueryCostEstimate::classify(1_000, 10_000, 1, 1_000_000, 10_000_000);
        assert_eq!(interactive.timeout_class, QueryTimeoutClass::Interactive);
        assert_eq!(
            interactive.execution_mode,
            ResultDeliveryMode::InteractiveStream
        );
        assert!(!interactive.async_required);

        let batch = QueryCostEstimate::classify(1_000_000, 5_000_000, 3, 1_000_000, 10_000_000);
        assert_eq!(batch.timeout_class, QueryTimeoutClass::Batch);
        assert_eq!(batch.execution_mode, ResultDeliveryMode::PagedResult);
        assert!(batch.async_required);

        let export =
            QueryCostEstimate::classify(1_000_000_000, 50_000_000, 5, 1_000_000, 10_000_000);
        assert_eq!(export.timeout_class, QueryTimeoutClass::Export);
        assert_eq!(
            export.execution_mode,
            ResultDeliveryMode::AsyncMaterializedExport
        );
        assert!(export.async_required);
    }

    #[test]
    fn default_cost_model_uses_plan_features_limits_and_relation_statistics() {
        let input = QueryCostInput {
            plan_features: PlanFeatures {
                has_joins: true,
                ..PlanFeatures::default()
            },
            relations: vec![RelationStatistics {
                relation: "synapses".into(),
                estimated_rows: 1_000_000,
                average_row_bytes: 128,
                shard_count: Some(5),
            }],
            selected_columns: 4,
            row_limit: Some(50_000),
            estimated_filter_selectivity_ppm: 500_000,
            backend_capabilities: CapabilitySet::default().with_limits(BackendExecutionLimits {
                interactive_byte_limit: 1_000_000,
                batch_byte_limit: 100_000_000,
                ..BackendExecutionLimits::default()
            }),
            ..QueryCostInput::default()
        };
        let estimate = DefaultQueryCostModel.estimate(&input);
        assert_eq!(estimate.estimated_rows, 50_000);
        assert_eq!(estimate.estimated_shards, 5);
        assert_eq!(estimate.timeout_class, QueryTimeoutClass::Batch);
        assert!(estimate.async_required);
        assert!(!estimate.diagnostics.is_empty());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendAnalysis {
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<QueryDiagnostic>,
    pub estimated_cost_class: EstimatedCostClass,
    pub result_schema: ResultSchema,
    pub provenance: ProvenanceReceipt,
}

impl BackendAnalysis {
    pub fn errors(&self) -> impl Iterator<Item = &QueryDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diag| diag.severity == DiagnosticSeverity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &QueryDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diag| diag.severity == DiagnosticSeverity::Warning)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlArtifact {
    pub dialect: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParameterSchema>,
    pub result_schema: ResultSchema,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    pub provenance: ProvenanceReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpaqueArtifact {
    pub kind: String,
    pub description: String,
    pub provenance: ProvenanceReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitArtifact {
    Sql(SqlArtifact),
    Opaque(OpaqueArtifact),
}

impl EmitArtifact {
    pub fn as_sql(&self) -> Option<&SqlArtifact> {
        match self {
            Self::Sql(sql) => Some(sql),
            Self::Opaque(_) => None,
        }
    }
}

pub trait BackendAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> CapabilitySet;
    fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis;
    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact>;
}
