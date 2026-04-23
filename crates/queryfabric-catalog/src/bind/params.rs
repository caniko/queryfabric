use queryfabric_ir::{
    DataType, ParameterBinding, ParameterRef, ParameterSchema, ParameterValue, QueryDiagnostic,
    QueryFabricError, QueryParameters, Result, SyntaxNode,
};

use super::{Binder, NullableConstraint, ParameterConstraint, helpers::render_data_type};

impl Binder<'_> {
    pub(super) fn finalize_parameters(&mut self) -> Result<Vec<ParameterBinding>> {
        let mut bindings = Vec::new();
        let constraints = self
            .parameter_constraints
            .iter()
            .map(|(reference, constraint)| (reference.clone(), constraint.clone()))
            .collect::<Vec<_>>();
        for (reference, constraint) in constraints {
            let node = SyntaxNode {
                span: constraint.span,
                node_id: constraint
                    .node_id
                    .clone()
                    .unwrap_or_else(|| format!("param:{reference}")),
            };
            let mut has_error = false;

            let data_type = match constraint.data_type.clone().filter(|ty| !ty.is_unknown()) {
                Some(data_type) => data_type,
                None => {
                    self.push_error(
                        "QF0018",
                        format!("Parameter `{reference}` has unresolved type."),
                        &node,
                        Some("Cast the parameter or use it in a typed comparison."),
                    );
                    has_error = true;
                    DataType::Unknown
                }
            };
            let nullable = match constraint.nullable {
                NullableConstraint::Nullable => true,
                NullableConstraint::NonNull => false,
                NullableConstraint::Unknown => {
                    self.push_error(
                        "QF0019",
                        format!("Parameter `{reference}` has unresolved nullability."),
                        &node,
                        Some("Use the parameter in a context that constrains nullability."),
                    );
                    has_error = true;
                    true
                }
            };
            if has_error {
                continue;
            }
            let schema = ParameterSchema {
                reference: reference.clone(),
                data_type: data_type.clone(),
                nullable,
                metadata: queryfabric_ir::FieldMetadata::default(),
            };
            let value = self.parameters.lookup(&reference).cloned();
            if let Some(value) = &value {
                if !parameter_value_matches_schema(value, &schema) {
                    self.push_error(
                        "QF0020",
                        format!(
                            "Parameter `{reference}` value is incompatible with inferred type `{}`.",
                            render_data_type(&schema.data_type)
                        ),
                        &node,
                        Some("Supply a value that matches the inferred parameter schema."),
                    );
                    continue;
                }
                if matches!(value, ParameterValue::Null) && !schema.nullable {
                    self.push_error(
                        "QF0021",
                        format!("Parameter `{reference}` cannot be NULL in this query."),
                        &node,
                        Some(
                            "Supply a non-null parameter value or rewrite the query to allow NULL.",
                        ),
                    );
                    continue;
                }
            }
            bindings.push(ParameterBinding { schema, value });
        }

        if self.diagnostics.iter().any(QueryDiagnostic::is_error) {
            return Err(QueryFabricError::bind(
                self.diagnostics
                    .iter()
                    .find(|diag| diag.is_error())
                    .map(|diag| diag.message.clone())
                    .unwrap_or_else(|| "parameter binding failed".into()),
                None,
                self.diagnostics.clone(),
                None,
            ));
        }

        bindings.sort_by(|left, right| left.schema.reference.cmp(&right.schema.reference));
        Ok(bindings)
    }

    pub(super) fn allocate_parameter(&mut self, reference: ParameterRef) -> ParameterRef {
        match reference {
            ParameterRef::Positional(0) => {
                let reference = ParameterRef::Positional(self.next_auto_position);
                self.next_auto_position += 1;
                reference
            }
            other => other,
        }
    }

    pub(super) fn record_parameter_use(
        &mut self,
        reference: &ParameterRef,
        expected: super::ExpectedType<'_>,
        node: &SyntaxNode,
    ) {
        let entry = self
            .parameter_constraints
            .entry(reference.clone())
            .or_insert(ParameterConstraint {
                data_type: None,
                nullable: NullableConstraint::Unknown,
                span: node.span,
                node_id: Some(node.node_id.clone()),
            });
        entry.nullable = entry.nullable.merge(expected.nullable);
        if let Some(data_type) = expected.data_type.filter(|ty| !ty.is_unknown()) {
            entry.data_type = Some(match &entry.data_type {
                Some(existing) => {
                    DataType::common_type(existing, data_type).unwrap_or(DataType::Unknown)
                }
                None => data_type.clone(),
            });
        }
    }
}

pub(super) fn next_auto_position_seed(parameters: &QueryParameters) -> u32 {
    parameters
        .positional
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

pub(super) fn parameter_value_matches_schema(
    value: &ParameterValue,
    schema: &ParameterSchema,
) -> bool {
    match value {
        ParameterValue::Null => schema.nullable,
        ParameterValue::Boolean(_) => matches!(schema.data_type, DataType::Boolean),
        ParameterValue::Int64(_) => matches!(schema.data_type, DataType::Int32 | DataType::Int64),
        ParameterValue::Float64(_) => matches!(
            schema.data_type,
            DataType::Float64 | DataType::Decimal { .. }
        ),
        ParameterValue::Utf8(_) => matches!(schema.data_type, DataType::Utf8),
        ParameterValue::Uuid(_) => matches!(schema.data_type, DataType::Uuid),
        ParameterValue::Json(_) => matches!(schema.data_type, DataType::Json),
        ParameterValue::List(values) => match &schema.data_type {
            DataType::List(inner) => values.iter().all(|value| {
                parameter_value_matches_schema(
                    value,
                    &ParameterSchema {
                        reference: schema.reference.clone(),
                        data_type: (**inner).clone(),
                        nullable: schema.nullable,
                        metadata: schema.metadata.clone(),
                    },
                )
            }),
            _ => false,
        },
        _ => false,
    }
}
