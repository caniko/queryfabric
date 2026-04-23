use queryfabric_ir::{DataType, LiteralValue, QueryFabricError, Result};

pub(super) fn unsupported(
    feature: impl Into<String>,
    detail: impl Into<String>,
) -> QueryFabricError {
    QueryFabricError::UnsupportedFeature {
        feature: feature.into(),
        detail: detail.into(),
    }
}

pub(super) fn ordered_parameters(
    query: &queryfabric_ir::BoundQuery,
) -> Vec<queryfabric_ir::ParameterSchema> {
    let mut ordered = query
        .parameters()
        .iter()
        .map(|binding| binding.schema.clone())
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.reference.cmp(&right.reference));
    ordered
}

pub(super) fn render_literal(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Null => "NULL".into(),
        LiteralValue::Boolean(value) => value.to_string().to_uppercase(),
        LiteralValue::Int64(value) => value.to_string(),
        LiteralValue::Float64(value) => value.clone(),
        LiteralValue::Utf8(value) => format!("'{}'", value.replace('\'', "''")),
    }
}

pub(super) fn render_binary_operator(op: queryfabric_ir::BinaryOperator) -> &'static str {
    match op {
        queryfabric_ir::BinaryOperator::Add => "+",
        queryfabric_ir::BinaryOperator::Subtract => "-",
        queryfabric_ir::BinaryOperator::Multiply => "*",
        queryfabric_ir::BinaryOperator::Divide => "/",
        queryfabric_ir::BinaryOperator::Eq => "=",
        queryfabric_ir::BinaryOperator::NotEq => "<>",
        queryfabric_ir::BinaryOperator::Lt => "<",
        queryfabric_ir::BinaryOperator::LtEq => "<=",
        queryfabric_ir::BinaryOperator::Gt => ">",
        queryfabric_ir::BinaryOperator::GtEq => ">=",
        queryfabric_ir::BinaryOperator::And => "AND",
        queryfabric_ir::BinaryOperator::Or => "OR",
    }
}

pub(super) fn backend_type_name(
    backend: super::emit::SqlBackend,
    data_type: &DataType,
) -> Result<String> {
    match backend {
        super::emit::SqlBackend::ClickHouse => match data_type {
            DataType::Boolean => Ok("Bool".into()),
            DataType::Int32 => Ok("Int32".into()),
            DataType::Int64 => Ok("Int64".into()),
            DataType::Float64 => Ok("Float64".into()),
            DataType::Utf8 => Ok("String".into()),
            DataType::Uuid => Ok("UUID".into()),
            DataType::Json => Ok("JSON".into()),
            DataType::Date => Ok("Date".into()),
            DataType::Decimal { precision, scale } => Ok(format!("Decimal({precision}, {scale})")),
            DataType::Timestamp { timezone } => Ok(timezone
                .as_ref()
                .map(|timezone| format!("DateTime64(6, '{timezone}')"))
                .unwrap_or_else(|| "DateTime64(6)".into())),
            DataType::List(inner) => Ok(format!("Array({})", backend_type_name(backend, inner)?)),
            DataType::Struct(_) => Err(unsupported("type", "struct parameters are not supported")),
            DataType::Unknown => Err(unsupported("type", "cannot render unknown type")),
            _ => Err(unsupported("type", "unsupported ClickHouse type mapping")),
        },
        super::emit::SqlBackend::Postgres => match data_type {
            DataType::Boolean => Ok("BOOLEAN".into()),
            DataType::Int32 => Ok("INTEGER".into()),
            DataType::Int64 => Ok("BIGINT".into()),
            DataType::Float64 => Ok("DOUBLE PRECISION".into()),
            DataType::Utf8 => Ok("TEXT".into()),
            DataType::Uuid => Ok("UUID".into()),
            DataType::Json => Ok("JSONB".into()),
            DataType::Date => Ok("DATE".into()),
            DataType::Decimal { precision, scale } => Ok(format!("NUMERIC({precision}, {scale})")),
            DataType::Timestamp { timezone } => Ok(if timezone.is_some() {
                "TIMESTAMPTZ".into()
            } else {
                "TIMESTAMP".into()
            }),
            DataType::List(inner) => Ok(format!("{}[]", backend_type_name(backend, inner)?)),
            DataType::Struct(_) => Err(unsupported("type", "struct parameters are not supported")),
            DataType::Unknown => Err(unsupported("type", "cannot render unknown type")),
            _ => Err(unsupported("type", "unsupported PostgreSQL type mapping")),
        },
    }
}

pub(super) fn backend_code(backend: &str) -> &'static str {
    match backend {
        "clickhouse" => "11",
        "postgres" => "21",
        _ => "99",
    }
}

pub(super) trait DataTypeExt {
    fn is_list(&self) -> bool;
}

impl DataTypeExt for DataType {
    fn is_list(&self) -> bool {
        matches!(self, DataType::List(_))
    }
}
