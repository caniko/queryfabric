use queryfabric_ir::{
    BoundExpr, BoundExprKind, BoundFunctionCall, BoundWindowSpec, DataType, SyntaxExpr,
    SyntaxFunctionCall,
};

use crate::builtins::builtin_function_signature;
use crate::model::{FunctionKind, FunctionSignature};

use super::{Binder, ExpectedType, NullableConstraint};

pub(super) fn bind_function(
    binder: &mut Binder<'_>,
    expr: &SyntaxExpr,
    function: &SyntaxFunctionCall,
    scope: &super::scope::Scope,
    outer_scope: Option<&super::scope::Scope>,
) -> BoundExpr {
    let signature = binder
        .catalog
        .resolve_function(
            function.function.namespace.as_deref(),
            &function.function.name,
        )
        .or_else(|| {
            builtin_function_signature(
                function.function.namespace.as_deref(),
                &function.function.name,
            )
        });

    let Some(signature) = signature else {
        return binder.unsupported_expr(
            expr,
            "QF0013",
            format!("Unknown function `{}`.", function.function.display_name()),
            Some("Register the function in the catalog or portable builtin registry."),
        );
    };

    let args = function
        .args
        .iter()
        .enumerate()
        .map(|(idx, arg)| {
            let data_type = signature.arg_types.get(idx).or_else(|| {
                signature
                    .variadic
                    .then_some(signature.arg_types.last())
                    .flatten()
            });
            binder.bind_expr(
                arg,
                scope,
                outer_scope,
                ExpectedType {
                    data_type,
                    nullable: NullableConstraint::NonNull,
                },
            )
        })
        .collect::<Vec<_>>();

    let filter = function.filter.as_ref().map(|filter| {
        Box::new(binder.bind_expr(
            filter,
            scope,
            outer_scope,
            ExpectedType {
                data_type: Some(&DataType::Boolean),
                nullable: NullableConstraint::NonNull,
            },
        ))
    });

    let over = function.over.as_ref().map(|window| BoundWindowSpec {
        partition_by: window
            .partition_by
            .iter()
            .map(|expr| binder.bind_expr(expr, scope, outer_scope, ExpectedType::default()))
            .collect(),
        order_by: window
            .order_by
            .iter()
            .map(|expr| binder.bind_order_by_expr(expr, scope, outer_scope))
            .collect(),
        node: window.node.clone(),
    });

    if signature.metadata_flag("approximate") || function.function.namespace.is_some() {
        binder.push_warning(
            "QF0104",
            format!(
                "Function `{}` is backend-specific and outside the verified portable subset.",
                function.function.display_name()
            ),
            &expr.node,
            Some("Rely on adapter-specific capability checks before executing this query."),
        );
    }

    BoundExpr {
        kind: BoundExprKind::function(BoundFunctionCall {
            function: function.function.clone(),
            resolved_backend_name: None,
            args: args.clone(),
            distinct: function.distinct,
            filter,
            over,
            resolved_signature_name: Some(signature.name.clone()),
        }),
        data_type: infer_function_return_type(&signature, &args),
        nullable: infer_function_nullability(&signature, &args),
        node: expr.node.clone(),
    }
}

pub(super) fn infer_function_return_type(
    signature: &FunctionSignature,
    args: &[BoundExpr],
) -> DataType {
    if !signature.return_type.is_unknown() {
        return signature.return_type.clone();
    }
    match signature.name.as_str() {
        "coalesce" | "greatest" | "least" | "min" | "max" | "lag" | "lead" | "first_value"
        | "last_value" => args
            .iter()
            .map(|arg| arg.data_type.clone())
            .reduce(|left, right| DataType::common_type(&left, &right).unwrap_or(DataType::Unknown))
            .unwrap_or(DataType::Unknown),
        _ => DataType::Unknown,
    }
}

pub(super) fn infer_function_nullability(
    signature: &FunctionSignature,
    args: &[BoundExpr],
) -> bool {
    match (signature.kind, signature.name.as_str()) {
        (FunctionKind::Aggregate, "count") => false,
        (FunctionKind::Aggregate, _) => true,
        (FunctionKind::Window, "rank" | "dense_rank" | "row_number") => false,
        (FunctionKind::Window, "lag" | "lead") => true,
        (_, "coalesce") => args.iter().all(|arg| arg.nullable),
        _ => args.iter().any(|arg| arg.nullable),
    }
}
