use queryfabric_ir::{
    BoundExpr, BoundExprKind, DataType, LiteralValue, QueryDiagnostic, SyntaxExpr,
};

use super::Binder;

impl Binder<'_> {
    pub(super) fn unsupported_expr(
        &mut self,
        expr: &SyntaxExpr,
        code: &str,
        message: impl Into<String>,
        remediation: Option<&str>,
    ) -> BoundExpr {
        self.push_error(code.to_owned(), message, &expr.node, remediation);
        BoundExpr {
            kind: BoundExprKind::Unsupported {
                description: expr.node.node_id.clone(),
            },
            data_type: DataType::Unknown,
            nullable: true,
            node: expr.node.clone(),
        }
    }

    pub(super) fn push_error(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        node: &queryfabric_ir::SyntaxNode,
        remediation: Option<&str>,
    ) {
        let mut diagnostic =
            QueryDiagnostic::error(code, message).with_node_id(node.node_id.clone());
        if let Some(span) = node.span {
            diagnostic = diagnostic.with_span(span);
        }
        if let Some(remediation) = remediation {
            diagnostic = diagnostic.with_remediation(remediation);
        }
        self.diagnostics.push(diagnostic);
    }

    pub(super) fn push_warning(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        node: &queryfabric_ir::SyntaxNode,
        remediation: Option<&str>,
    ) {
        let mut diagnostic =
            QueryDiagnostic::warning(code, message).with_node_id(node.node_id.clone());
        if let Some(span) = node.span {
            diagnostic = diagnostic.with_span(span);
        }
        if let Some(remediation) = remediation {
            diagnostic = diagnostic.with_remediation(remediation);
        }
        self.diagnostics.push(diagnostic);
    }
}

pub(super) fn bind_literal(expr: &SyntaxExpr, value: LiteralValue) -> BoundExpr {
    let (data_type, nullable) = match &value {
        LiteralValue::Null => (DataType::Unknown, true),
        LiteralValue::Boolean(_) => (DataType::Boolean, false),
        LiteralValue::Int64(_) => (DataType::Int64, false),
        LiteralValue::Float64(_) => (DataType::Float64, false),
        LiteralValue::Utf8(_) => (DataType::Utf8, false),
    };
    BoundExpr {
        kind: BoundExprKind::Literal(value),
        data_type,
        nullable,
        node: expr.node.clone(),
    }
}

pub(super) fn expression_name(expr: &SyntaxExpr) -> String {
    match &expr.kind {
        queryfabric_ir::SyntaxExprKind::Column { name, .. } => name.clone(),
        queryfabric_ir::SyntaxExprKind::Function(function) => function.function.display_name(),
        queryfabric_ir::SyntaxExprKind::Parameter(reference) => reference.to_string(),
        _ => expr.node.node_id.clone(),
    }
}

pub(super) fn render_data_type(data_type: &DataType) -> String {
    match data_type {
        DataType::Boolean => "BOOLEAN".into(),
        DataType::Int32 => "INT32".into(),
        DataType::Int64 => "INT64".into(),
        DataType::Float64 => "FLOAT64".into(),
        DataType::Utf8 => "UTF8".into(),
        DataType::Uuid => "UUID".into(),
        DataType::Json => "JSON".into(),
        DataType::Date => "DATE".into(),
        DataType::Decimal { precision, scale } => format!("DECIMAL({precision},{scale})"),
        DataType::Timestamp { timezone } => timezone
            .as_ref()
            .map(|timezone| format!("TIMESTAMP({timezone})"))
            .unwrap_or_else(|| "TIMESTAMP".into()),
        DataType::List(inner) => format!("LIST<{}>", render_data_type(inner)),
        DataType::Struct(_) => "STRUCT".into(),
        DataType::Unknown => "UNKNOWN".into(),
        _ => "UNSUPPORTED".into(),
    }
}
