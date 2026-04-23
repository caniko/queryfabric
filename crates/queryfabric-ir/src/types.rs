use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// Portable logical types used across QueryFabric catalogs, analyses, and
/// emitted artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DataType {
    Boolean,
    Int32,
    Int64,
    Float64,
    Utf8,
    Uuid,
    Json,
    Date,
    Decimal { precision: u8, scale: i8 },
    Timestamp { timezone: Option<String> },
    List(Box<DataType>),
    Struct(Vec<crate::ResultField>),
    Unknown,
}

impl DataType {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Int32 | Self::Int64 | Self::Float64 | Self::Decimal { .. }
        )
    }

    pub fn common_type(left: &Self, right: &Self) -> Option<Self> {
        if left == right {
            return Some(left.clone());
        }
        match (left, right) {
            (Self::Unknown, other) | (other, Self::Unknown) => Some(other.clone()),
            (Self::Int32, Self::Int64) | (Self::Int64, Self::Int32) => Some(Self::Int64),
            (Self::Int32, Self::Float64)
            | (Self::Float64, Self::Int32)
            | (Self::Int64, Self::Float64)
            | (Self::Float64, Self::Int64) => Some(Self::Float64),
            (Self::List(left), Self::List(right)) => {
                Self::common_type(left, right).map(|inner| Self::List(Box::new(inner)))
            }
            _ => None,
        }
    }
}

/// Optional scientific and backend-agnostic metadata attached to catalog and
/// result fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinate_reference_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ontology_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_scale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shape: Vec<usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, String>,
}

impl FieldMetadata {
    pub fn is_empty(&self) -> bool {
        self.unit.is_none()
            && self.coordinate_reference_system.is_none()
            && self.ontology_id.is_none()
            && self.modality.is_none()
            && self.provenance.is_none()
            && self.measurement_scale.is_none()
            && self.shape.is_empty()
            && self.extensions.is_empty()
    }
}

/// Field entry in a result or relation schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultField {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "FieldMetadata::is_empty")]
    pub metadata: FieldMetadata,
}

impl ResultField {
    pub fn new(name: impl Into<String>, data_type: DataType, nullable: bool) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable,
            metadata: FieldMetadata::default(),
        }
    }
}

/// Arrow-friendly result schema contract used by analyses and emitted
/// artifacts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultSchema {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<ResultField>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl ResultSchema {
    pub fn new(fields: Vec<ResultField>) -> Self {
        Self {
            fields,
            metadata: BTreeMap::new(),
        }
    }

    pub fn fields(&self) -> &[ResultField] {
        &self.fields
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Stable parameter identifier used in prepared/bound query contracts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ParameterRef {
    Positional(u32),
    Named(String),
}

impl fmt::Display for ParameterRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Positional(position) => write!(f, "${position}"),
            Self::Named(name) => write!(f, ":{name}"),
        }
    }
}

/// Portable parameter value envelope. QueryFabric does not execute queries, so
/// this is only used for binding validation and artifact metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ParameterValue {
    Null,
    Boolean(bool),
    Int64(i64),
    Float64(String),
    Utf8(String),
    Uuid(String),
    Json(String),
    List(Vec<ParameterValue>),
}

impl ParameterValue {
    pub fn inferred_type(&self) -> DataType {
        match self {
            Self::Null => DataType::Unknown,
            Self::Boolean(_) => DataType::Boolean,
            Self::Int64(_) => DataType::Int64,
            Self::Float64(_) => DataType::Float64,
            Self::Utf8(_) => DataType::Utf8,
            Self::Uuid(_) => DataType::Uuid,
            Self::Json(_) => DataType::Json,
            Self::List(values) => DataType::List(Box::new(
                values
                    .iter()
                    .filter_map(|value| {
                        let ty = value.inferred_type();
                        (!ty.is_unknown()).then_some(ty)
                    })
                    .reduce(|left, right| {
                        DataType::common_type(&left, &right).unwrap_or(DataType::Unknown)
                    })
                    .unwrap_or(DataType::Unknown),
            )),
        }
    }
}

/// Parameter schema exposed by the binder and emitted artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterSchema {
    pub reference: ParameterRef,
    pub data_type: DataType,
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "FieldMetadata::is_empty")]
    pub metadata: FieldMetadata,
}

/// Stable summary of placeholders referenced by a parsed query.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterSummary {
    /// Highest positional placeholder index referenced by the query.
    pub positional_count: u32,
    /// Deduplicated, sorted named placeholders referenced by the query.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_params: Vec<String>,
}

/// Bound parameter with an optional concrete value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterBinding {
    pub schema: ParameterSchema,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ParameterValue>,
}

/// Parameter input bag passed to the binder. Values are optional at bind time;
/// unresolved placeholders still become part of the bound query contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryParameters {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub positional: BTreeMap<u32, ParameterValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub named: BTreeMap<String, ParameterValue>,
}

impl QueryParameters {
    pub fn insert_positional(&mut self, position: u32, value: ParameterValue) {
        self.positional.insert(position, value);
    }

    pub fn insert_named(&mut self, name: impl Into<String>, value: ParameterValue) {
        self.named.insert(name.into(), value);
    }

    pub fn lookup(&self, reference: &ParameterRef) -> Option<&ParameterValue> {
        match reference {
            ParameterRef::Positional(position) => self.positional.get(position),
            ParameterRef::Named(name) => self.named.get(name),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.positional.is_empty() && self.named.is_empty()
    }
}

/// Stable function identifier recorded in capability requirements.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FunctionRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
}

impl FunctionRef {
    pub fn display_name(&self) -> String {
        match &self.namespace {
            Some(namespace) => format!("{namespace}.{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// Capability requirement expressed by the bound query before backend
/// selection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CapabilityRequirement {
    CommonTableExpressions,
    DerivedTables,
    Joins,
    Windows,
    SetOperations,
    Aggregates,
    DistinctAggregates,
    ScalarSubqueries,
    InSubqueries,
    Explain,
    LimitOffset,
    NamespacedFunctions,
    ApproximateAggregates,
    BackendSpecific(String),
}

/// Capability summary gathered during binding/analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequirements {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required: BTreeSet<CapabilityRequirement>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub referenced_functions: BTreeSet<FunctionRef>,
}

impl CapabilityRequirements {
    pub fn require(&mut self, requirement: CapabilityRequirement) {
        self.required.insert(requirement);
    }

    pub fn record_function(&mut self, function: FunctionRef) {
        self.referenced_functions.insert(function);
    }

    pub fn required(&self) -> &BTreeSet<CapabilityRequirement> {
        &self.required
    }

    pub fn referenced_functions(&self) -> &BTreeSet<FunctionRef> {
        &self.referenced_functions
    }

    pub fn is_empty(&self) -> bool {
        self.required.is_empty() && self.referenced_functions.is_empty()
    }
}

/// Stable catalog snapshot identity threaded through binding, analysis, and
/// emission.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CatalogSnapshotId(pub String);

impl fmt::Display for CatalogSnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Opaque dialect-owned metadata. Host directives such as SyQL `SCOPE` and
/// `DOWNLOAD` belong here rather than in the neutral core semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialectMetadata {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entries: BTreeMap<String, String>,
}

impl DialectMetadata {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }

    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.entries
    }

    pub(crate) fn entries_is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
