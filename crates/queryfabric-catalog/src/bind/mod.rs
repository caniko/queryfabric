use std::collections::BTreeMap;

use queryfabric_ir::{
    BoundQuery, DataType, ParameterRef, ParsedQuery, ProvenanceReceipt, QueryDiagnostic,
    QueryFabricError, QueryParameters, QuerySourceSpan, Result, ResultSchema,
};

use crate::model::Catalog;

mod exprs;
mod functions;
mod helpers;
mod params;
mod query;
mod scope;
mod suggest;

use self::query::capability_requirements_from_plan;
use self::scope::Scope;

pub fn bind_and_validate(
    parsed: &ParsedQuery,
    catalog: &dyn Catalog,
    parameters: &QueryParameters,
) -> Result<BoundQuery> {
    let snapshot = catalog.snapshot_id();
    let provenance = ProvenanceReceipt::for_query(parsed.canonical_sql(), parsed.dialect())
        .with_catalog_snapshot(snapshot.clone());

    let mut binder = Binder::new(catalog, parameters);
    let plan = binder.bind_query(parsed.syntax(), None);
    let mut diagnostics = parsed.lowering_diagnostics().to_vec();
    diagnostics.extend(binder.diagnostics.clone());
    let capability_requirements =
        capability_requirements_from_plan(&plan, parsed.explain(), catalog);

    if diagnostics.iter().any(QueryDiagnostic::is_error) {
        return Err(QueryFabricError::bind(
            diagnostics
                .iter()
                .find(|diag| diag.is_error())
                .map(|diag| diag.message.clone())
                .unwrap_or_else(|| "binding failed".into()),
            Some(parsed),
            diagnostics,
            Some(provenance),
        ));
    }

    let parameters = binder.finalize_parameters()?;
    Ok(BoundQuery::new(parsed.clone())
        .with_catalog_snapshot(snapshot)
        .with_plan(plan.clone())
        .with_parameters(parameters)
        .with_diagnostics(diagnostics)
        .with_capability_requirements(capability_requirements.clone())
        .with_result_schema(plan.result_schema.clone())
        .with_provenance(provenance.with_capability_decision(
            if capability_requirements.is_empty() {
                "portable-v1:trivial"
            } else {
                "portable-v1:checked"
            },
        )))
}

pub fn infer_result_schema(query: &ParsedQuery, catalog: &dyn Catalog) -> Result<ResultSchema> {
    bind_and_validate(query, catalog, &QueryParameters::default())
        .map(|bound| bound.result_schema().clone())
}

pub fn unsupported(feature: impl Into<String>, detail: impl Into<String>) -> QueryFabricError {
    QueryFabricError::UnsupportedFeature {
        feature: feature.into(),
        detail: detail.into(),
    }
}

#[derive(Debug, Clone)]
struct ParameterConstraint {
    data_type: Option<DataType>,
    nullable: NullableConstraint,
    span: Option<QuerySourceSpan>,
    node_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum NullableConstraint {
    #[default]
    Unknown,
    Nullable,
    NonNull,
}

impl NullableConstraint {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::NonNull, _) | (_, Self::NonNull) => Self::NonNull,
            (Self::Nullable, Self::Nullable) => Self::Nullable,
            (Self::Nullable, Self::Unknown) | (Self::Unknown, Self::Nullable) => Self::Nullable,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ExpectedType<'a> {
    data_type: Option<&'a DataType>,
    nullable: NullableConstraint,
}

#[derive(Clone)]
struct Binder<'a> {
    catalog: &'a dyn Catalog,
    parameters: &'a QueryParameters,
    diagnostics: Vec<QueryDiagnostic>,
    parameter_constraints: BTreeMap<ParameterRef, ParameterConstraint>,
    next_auto_position: u32,
}

impl<'a> Binder<'a> {
    fn new(catalog: &'a dyn Catalog, parameters: &'a QueryParameters) -> Self {
        Self {
            catalog,
            parameters,
            diagnostics: Vec::new(),
            parameter_constraints: BTreeMap::new(),
            next_auto_position: params::next_auto_position_seed(parameters),
        }
    }
}
