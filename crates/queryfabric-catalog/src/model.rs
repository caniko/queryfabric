use std::collections::{BTreeMap, BTreeSet};

use queryfabric_ir::{
    BoundQuery, CatalogSnapshotId, DataType, DiagnosticSeverity, ParameterSchema,
    ProvenanceReceipt, QueryDiagnostic, Result, ResultField, ResultSchema,
};
use serde::{Deserialize, Serialize};

use crate::builtins::{builtin_function_signature, portable_builtin_functions};

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
}

#[derive(Debug, Clone)]
pub struct MemoryCatalog {
    snapshot_id: CatalogSnapshotId,
    relations: BTreeMap<(Option<String>, String), RelationSchema>,
    functions: BTreeMap<(Option<String>, String), FunctionSignature>,
}

impl Default for MemoryCatalog {
    fn default() -> Self {
        Self {
            snapshot_id: CatalogSnapshotId("memory-catalog".into()),
            relations: BTreeMap::new(),
            functions: BTreeMap::new(),
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
}

#[cfg(test)]
mod tests {
    use super::{
        Catalog, CatalogDocument, ColumnSchema, DataType, FunctionKind, FunctionSignature,
        FunctionVolatility, MemoryCatalog, RelationKind, RelationSchema,
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub features: BTreeSet<BackendFeature>,
}

impl CapabilitySet {
    pub fn from_features(features: impl IntoIterator<Item = BackendFeature>) -> Self {
        Self {
            features: features.into_iter().collect(),
        }
    }

    pub fn supports(&self, feature: BackendFeature) -> bool {
        self.features.contains(&feature)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstimatedCostClass {
    Low,
    Medium,
    High,
    Unknown,
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
