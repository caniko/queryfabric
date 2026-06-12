use queryfabric_catalog::PlanFeatures;
use queryfabric_ir::{BoundQuery, BoundRelation, BoundRelationBinding, BoundSelect, NameRef};

use super::{FederationError, PARTIALS_PLACEHOLDER, Result};

/// Reject query shapes that cannot be decomposed into scatter+gather stages.
pub fn validate_federation_shape(features: &PlanFeatures, bound: &BoundQuery) -> Result<()> {
    let mut unsupported = Vec::new();
    if features.has_ctes {
        unsupported.push("WITH (CTE)");
    }
    if features.has_derived_tables {
        unsupported.push("FROM subquery");
    }
    if features.has_joins {
        unsupported.push("JOIN");
    }
    if features.has_windows {
        unsupported.push("window functions");
    }
    if features.has_set_operations {
        unsupported.push("set operations");
    }
    if features.has_scalar_subqueries {
        unsupported.push("scalar subqueries");
    }
    if features.has_in_subqueries {
        unsupported.push("WHERE ... IN (SELECT ...)");
    }
    if features.has_clickhouse_settings {
        unsupported.push("SETTINGS");
    }
    if features.has_clickhouse_format {
        unsupported.push("FORMAT");
    }
    if !unsupported.is_empty() {
        return Err(FederationError::Unsupported(format!(
            "distributed execution does not support {}",
            unsupported.join(", ")
        )));
    }

    if primary_relation_binding(bound)?.relation_name.is_none() {
        return Err(FederationError::Unsupported(
            "distributed execution requires a primary base relation".into(),
        ));
    }
    Ok(())
}

pub(super) fn top_level_select(bound: &BoundQuery) -> Result<&BoundSelect> {
    bound.plan().body.as_select().ok_or_else(|| {
        FederationError::Unsupported("distributed execution requires a single SELECT".into())
    })
}

/// The single base-table binding a distributed query reads from.
pub fn primary_relation_binding(bound: &BoundQuery) -> Result<&BoundRelationBinding> {
    let select = top_level_select(bound)?;
    if select.from.len() != 1 {
        return Err(FederationError::Unsupported(
            "distributed execution requires a single FROM relation".into(),
        ));
    }
    let table = &select.from[0];
    if !table.joins.is_empty() {
        return Err(FederationError::Unsupported(
            "distributed execution does not support JOIN".into(),
        ));
    }
    match &table.relation {
        BoundRelation::Table { binding, .. } => Ok(binding),
        _ => Err(FederationError::Unsupported(
            "distributed execution requires a base table relation".into(),
        )),
    }
}

/// Display name of the relation a scatter statement targets.
pub fn from_target(binding: &BoundRelationBinding) -> Result<String> {
    binding
        .relation_name
        .as_ref()
        .map(NameRef::display_name)
        .ok_or_else(|| {
            FederationError::Unsupported(
                "distributed execution requires a named base relation".into(),
            )
        })
}

pub(super) fn render_relation(binding: &BoundRelationBinding) -> String {
    let mut sql = binding
        .relation_name
        .as_ref()
        .map(NameRef::display_name)
        .unwrap_or_else(|| binding.binding_name.clone());
    if binding
        .relation_name
        .as_ref()
        .is_some_and(|name| !name.name.eq_ignore_ascii_case(&binding.binding_name))
    {
        sql.push_str(" AS ");
        sql.push_str(&binding.binding_name);
    }
    sql
}

pub(super) fn wrap_query_over_partials(
    sql: &str,
    binding: &BoundRelationBinding,
) -> Result<String> {
    wrap_relation_source(
        sql,
        binding,
        &format!("({PARTIALS_PLACEHOLDER}) AS {}", binding.binding_name),
    )
}

pub(super) fn wrap_placeholder_relation(
    sql: &str,
    binding: &BoundRelationBinding,
) -> Result<String> {
    wrap_relation_source(
        sql,
        binding,
        &format!("({PARTIALS_PLACEHOLDER}) AS {}", binding.binding_name),
    )
}

fn wrap_relation_source(
    sql: &str,
    binding: &BoundRelationBinding,
    replacement: &str,
) -> Result<String> {
    let rendered = render_relation(binding);
    let target = format!("FROM {rendered}");
    if !sql.contains(&target) {
        return Err(FederationError::Unsupported(format!(
            "could not identify distributed source relation in emitted SQL: expected `{target}`"
        )));
    }
    Ok(sql.replacen(&target, &format!("FROM {replacement}"), 1))
}
