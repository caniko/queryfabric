use queryfabric_catalog::{BackendAdapter, Catalog, SqlArtifact};
use queryfabric_ir::{
    BoundQuery, BoundRelation, BoundRelationBinding, BoundSelect, BoundSetExpr,
    BoundTableWithJoins, NameRef, ResultSchema,
};

use super::aggregate::{
    classify_supported_aggregate, column_expr, column_projection, expr_contains_aggregate,
    field_for_projection, projection_output_name,
};
use super::relations::{
    from_target, primary_relation_binding, top_level_select, wrap_placeholder_relation,
    wrap_query_over_partials,
};
use super::{FederationError, PARTIALS_RELATION, Result, ScatterGatherPlan};

fn emit_sql(
    adapter: &dyn BackendAdapter,
    bound: &BoundQuery,
    catalog: &dyn Catalog,
) -> Result<SqlArtifact> {
    adapter
        .emit(bound, catalog)?
        .as_sql()
        .cloned()
        .ok_or_else(|| {
            FederationError::Unsupported(format!(
                "backend `{}` did not produce a SQL artifact for distributed planning",
                adapter.name()
            ))
        })
}

/// Build a passthrough scatter-gather plan: every node runs the query with
/// ORDER BY/LIMIT/OFFSET stripped; the gather stage re-applies them over the
/// combined partials.
pub fn build_federated_passthrough_plan(
    adapter: &dyn BackendAdapter,
    catalog: &dyn Catalog,
    bound: &BoundQuery,
    artifact: &SqlArtifact,
) -> Result<ScatterGatherPlan> {
    let base_relation = primary_relation_binding(bound)?;
    let from_target = from_target(base_relation)?;

    let mut scatter_plan = bound.plan().clone();
    scatter_plan.order_by.clear();
    scatter_plan.limit = None;
    scatter_plan.offset = None;

    let scatter_bound = bound.clone().with_plan(scatter_plan);
    let scatter_artifact = emit_sql(adapter, &scatter_bound, catalog)?;

    let gather_sql = wrap_query_over_partials(&artifact.text, base_relation)?;

    Ok(ScatterGatherPlan {
        scatter_sql: scatter_artifact.text,
        gather_sql,
        from_target,
        result_schema: artifact.result_schema.clone(),
    })
}

/// Build a two-stage aggregate plan: the scatter stage computes node-local
/// partial aggregates; the gather stage merges them (`SUM`/`MIN`/`MAX` merge
/// with themselves, `COUNT` merges with `SUM`, `AVG` becomes
/// `SUM(sum)/SUM(count)`).
pub fn build_federated_aggregate_plan(
    adapter: &dyn BackendAdapter,
    catalog: &dyn Catalog,
    bound: &BoundQuery,
    artifact: &SqlArtifact,
) -> Result<ScatterGatherPlan> {
    let original_select = top_level_select(bound)?;
    if original_select.distinct {
        return Err(FederationError::Unsupported(
            "federation aggregate planning does not support SELECT DISTINCT".into(),
        ));
    }
    if original_select.having.is_some() {
        return Err(FederationError::Unsupported(
            "federation aggregate planning does not yet support HAVING".into(),
        ));
    }
    if bound.plan().offset.is_some() {
        return Err(FederationError::Unsupported(
            "federation aggregate planning does not yet support OFFSET".into(),
        ));
    }

    let base_relation = primary_relation_binding(bound)?;
    let from_target = from_target(base_relation)?;

    let mut scatter_projection = Vec::new();
    let mut gather_projection = Vec::new();
    let mut gather_group_by = Vec::new();
    let mut scatter_fields = Vec::new();
    let mut aggregate_count = 0usize;

    for (index, item) in original_select.projection.iter().enumerate() {
        let Some(details) = item.as_expr() else {
            return Err(FederationError::Unsupported(
                "federation aggregate planning requires explicit projection expressions".into(),
            ));
        };

        if let Some(aggregate) = classify_supported_aggregate(details, index)? {
            aggregate_count += 1;
            for scatter_item in &aggregate.scatter_projection {
                scatter_fields.push(field_for_projection(scatter_item)?);
            }
            scatter_projection.extend(aggregate.scatter_projection);
            gather_projection.push(aggregate.gather_projection);
            continue;
        }

        if expr_contains_aggregate(&details.expr) {
            return Err(FederationError::Unsupported(
                "federation aggregate planning only supports top-level SUM/COUNT/AVG/MIN/MAX projections".into(),
            ));
        }

        let output_name = projection_output_name(details).ok_or_else(|| {
            FederationError::Unsupported(
                "federation aggregate planning requires aliases for computed non-aggregate projections"
                    .into(),
            )
        })?;

        scatter_fields.push(details.field.clone());
        scatter_projection.push(item.clone());
        gather_projection.push(column_projection(
            &output_name,
            details.field.clone(),
            details.node.clone(),
        ));
        gather_group_by.push(column_expr(
            &output_name,
            details.field.data_type.clone(),
            details.field.nullable,
            details.node.clone(),
        ));
    }

    if aggregate_count == 0 {
        return Err(FederationError::Unsupported(
            "expected aggregate projections for federation aggregate planning".into(),
        ));
    }

    for order_by in &bound.plan().order_by {
        if expr_contains_aggregate(&order_by.expr) {
            return Err(FederationError::Unsupported(
                "federation aggregate ORDER BY must reference projected aliases, not aggregate expressions".into(),
            ));
        }
    }

    let mut scatter_select = original_select.clone();
    scatter_select.projection = scatter_projection;
    scatter_select.having = None;
    scatter_select.result_schema = ResultSchema::new(scatter_fields);

    let mut scatter_plan = bound.plan().clone();
    scatter_plan.body = BoundSetExpr::select(scatter_select);
    scatter_plan.order_by.clear();
    scatter_plan.limit = None;
    scatter_plan.offset = None;
    scatter_plan.result_schema = scatter_plan.body.result_schema().clone();

    let scatter_bound = bound.clone().with_plan(scatter_plan);
    let scatter_artifact = emit_sql(adapter, &scatter_bound, catalog)?;

    let placeholder_binding = BoundRelationBinding {
        binding_name: base_relation.binding_name.clone(),
        relation_name: Some(NameRef {
            namespace: None,
            name: PARTIALS_RELATION.into(),
        }),
        schema: scatter_artifact.result_schema.clone(),
    };

    let gather_select = BoundSelect {
        distinct: false,
        projection: gather_projection,
        from: vec![BoundTableWithJoins {
            relation: BoundRelation::Table {
                binding: placeholder_binding.clone(),
                node: original_select.node.clone(),
            },
            joins: Vec::new(),
            node: original_select.node.clone(),
        }],
        selection: None,
        group_by: gather_group_by,
        having: None,
        result_schema: artifact.result_schema.clone(),
        node: original_select.node.clone(),
    };

    let mut gather_plan = bound.plan().clone();
    gather_plan.body = BoundSetExpr::select(gather_select);
    gather_plan.result_schema = artifact.result_schema.clone();

    let gather_bound = bound.clone().with_plan(gather_plan);
    let gather_artifact = emit_sql(adapter, &gather_bound, catalog)?;

    let gather_sql = wrap_placeholder_relation(&gather_artifact.text, &placeholder_binding)?;

    Ok(ScatterGatherPlan {
        scatter_sql: scatter_artifact.text,
        gather_sql,
        from_target,
        result_schema: artifact.result_schema.clone(),
    })
}
