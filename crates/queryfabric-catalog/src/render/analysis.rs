use queryfabric_ir::{BoundQuery, CapabilityRequirement, QueryDiagnostic};

use super::helpers::backend_code;
use crate::features::plan_features_from_bound;
use crate::model::{BackendAnalysis, BackendFeature, CapabilitySet, Catalog, EstimatedCostClass};

pub fn analyze_backend_support(
    query: &BoundQuery,
    catalog: &dyn Catalog,
    backend: &str,
    capabilities: CapabilitySet,
    allow_clickhouse_clauses: bool,
) -> BackendAnalysis {
    let mut diagnostics = query.diagnostics().to_vec();
    let features = plan_features_from_bound(query.plan(), catalog);

    if features.has_clickhouse_settings && !allow_clickhouse_clauses {
        diagnostics.push(
            QueryDiagnostic::error(
                format!("QF{}101", backend_code(backend)),
                "ClickHouse SETTINGS are not supported on this backend.",
            )
            .with_backend(backend),
        );
    }
    if features.has_clickhouse_format && !allow_clickhouse_clauses {
        diagnostics.push(
            QueryDiagnostic::error(
                format!("QF{}102", backend_code(backend)),
                "ClickHouse FORMAT is not supported on this backend.",
            )
            .with_backend(backend),
        );
    }

    for function in &features.functions {
        let Some(signature) =
            catalog.resolve_function(function.namespace.as_deref(), &function.name)
        else {
            diagnostics.push(
                QueryDiagnostic::error(
                    format!("QF{}103", backend_code(backend)),
                    format!(
                        "Function `{}` is not registered in the catalog.",
                        function.display_name()
                    ),
                )
                .with_backend(backend),
            );
            continue;
        };
        if signature.backend_mapping(backend).is_none() {
            diagnostics.push(
                QueryDiagnostic::error(
                    format!("QF{}104", backend_code(backend)),
                    format!(
                        "Function `{}` has no `{backend}` backend mapping.",
                        function.display_name()
                    ),
                )
                .with_backend(backend),
            );
        }
        if !capabilities.supports(BackendFeature::ApproximateAggregates)
            && signature.metadata_flag("approximate")
        {
            diagnostics.push(
                QueryDiagnostic::error(
                    format!("QF{}105", backend_code(backend)),
                    format!(
                        "Approximate aggregate `{}` is not in the verified `{backend}` subset.",
                        function.display_name()
                    ),
                )
                .with_backend(backend),
            );
        }
        if !capabilities.supports(BackendFeature::NamespacedFunctions)
            && function.namespace.is_some()
        {
            diagnostics.push(
                QueryDiagnostic::error(
                    format!("QF{}106", backend_code(backend)),
                    format!(
                        "Namespaced function `{}` is not in the verified `{backend}` subset.",
                        function.display_name()
                    ),
                )
                .with_backend(backend),
            );
        }
    }

    let estimated_cost_class = if query
        .capability_requirements()
        .required()
        .contains(&CapabilityRequirement::Windows)
        || query
            .capability_requirements()
            .required()
            .contains(&CapabilityRequirement::Joins)
    {
        EstimatedCostClass::High
    } else if query
        .capability_requirements()
        .required()
        .contains(&CapabilityRequirement::SetOperations)
        || query
            .capability_requirements()
            .required()
            .contains(&CapabilityRequirement::CommonTableExpressions)
    {
        EstimatedCostClass::Medium
    } else {
        EstimatedCostClass::Low
    };

    BackendAnalysis {
        supported: !diagnostics.iter().any(QueryDiagnostic::is_error),
        diagnostics,
        estimated_cost_class,
        result_schema: query.result_schema().clone(),
        provenance: query
            .provenance()
            .clone()
            .with_backend(backend)
            .with_capability_decision(format!("{backend}:checked")),
    }
}
