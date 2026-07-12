use queryfabric_ir::{DataType, FunctionRef, LiteralValue, NameRef, QueryFabricError, Result};

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

/// Render a logical identifier without allowing it to become backend syntax.
///
/// Common ASCII identifiers stay readable for backwards-compatible artifacts;
/// every other non-control name is quoted and quote characters are doubled.
pub(super) fn render_identifier(backend: super::emit::SqlBackend, value: &str) -> Result<String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(unsupported(
            "identifier",
            "identifiers must be non-empty and contain no control characters",
        ));
    }
    let lower = value.to_ascii_lowercase();
    let reserved = matches!(
        lower.as_str(),
        "all"
            | "and"
            | "as"
            | "by"
            | "case"
            | "cross"
            | "distinct"
            | "else"
            | "end"
            | "exists"
            | "from"
            | "full"
            | "group"
            | "having"
            | "in"
            | "inner"
            | "is"
            | "join"
            | "left"
            | "like"
            | "limit"
            | "not"
            | "null"
            | "offset"
            | "on"
            | "or"
            | "order"
            | "right"
            | "select"
            | "then"
            | "union"
            | "when"
            | "where"
            | "with"
    );
    let mut chars = value.chars();
    let simple = !reserved
        && chars
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        });
    if simple {
        return Ok(value.to_owned());
    }
    let quote = match backend {
        super::emit::SqlBackend::ClickHouse | super::emit::SqlBackend::Postgres => '"',
    };
    Ok(format!(
        "{quote}{}{quote}",
        value.replace(quote, &format!("{quote}{quote}"))
    ))
}

pub(super) fn render_qualified_name(
    backend: super::emit::SqlBackend,
    value: &str,
) -> Result<String> {
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(unsupported(
            "identifier",
            "qualified names cannot contain empty segments",
        ));
    }
    segments
        .into_iter()
        .map(|segment| render_identifier(backend, segment))
        .collect::<Result<Vec<_>>>()
        .map(|segments| segments.join("."))
}

pub(super) fn render_name_ref(backend: super::emit::SqlBackend, name: &NameRef) -> Result<String> {
    let rendered_name = render_identifier(backend, &name.name)?;
    match &name.namespace {
        Some(namespace) => Ok(format!(
            "{}.{}",
            render_identifier(backend, namespace)?,
            rendered_name
        )),
        None => Ok(rendered_name),
    }
}

pub(super) fn render_function_ref(
    backend: super::emit::SqlBackend,
    function: &FunctionRef,
) -> Result<String> {
    let name = if is_parameterized_function_name(&function.name) {
        function.name.clone()
    } else {
        render_identifier(backend, &function.name)?
    };
    match &function.namespace {
        Some(namespace) => Ok(format!(
            "{}.{}",
            render_identifier(backend, namespace)?,
            name
        )),
        None => Ok(name),
    }
}

fn is_parameterized_function_name(value: &str) -> bool {
    let Some(open) = value.find('(') else {
        return false;
    };
    value.ends_with(')')
        && open > 0
        && value[..open]
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
        && value[open + 1..value.len() - 1]
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, '.' | ','))
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
            DataType::Timestamp { timezone } => match timezone {
                Some(timezone) => {
                    if timezone.is_empty()
                        || !timezone.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || matches!(character, '_' | '/' | '+' | '-')
                        })
                    {
                        return Err(unsupported(
                            "type",
                            "ClickHouse timestamp timezones contain unsupported characters",
                        ));
                    }
                    Ok(format!("DateTime64(6, '{timezone}')"))
                }
                None => Ok("DateTime64(6)".into()),
            },
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

#[cfg(test)]
mod tests {
    use super::{backend_type_name, render_function_ref, render_identifier, render_qualified_name};
    use crate::render::emit::SqlBackend;
    use queryfabric_ir::{DataType, FunctionRef};

    #[test]
    fn identifiers_are_segmented_and_quote_injection_text() {
        assert_eq!(
            render_qualified_name(SqlBackend::Postgres, "safe.table").unwrap(),
            "safe.table"
        );
        assert_eq!(
            render_identifier(SqlBackend::Postgres, "table; DROP TABLE users").unwrap(),
            "\"table; DROP TABLE users\""
        );
        assert_eq!(
            render_identifier(SqlBackend::Postgres, "column\"name").unwrap(),
            "\"column\"\"name\""
        );
    }

    #[test]
    fn mapped_function_names_are_allowlisted() {
        let safe = FunctionRef {
            namespace: Some("ch".into()),
            name: "quantile(0.5)".into(),
        };
        assert_eq!(
            render_function_ref(SqlBackend::ClickHouse, &safe).unwrap(),
            "ch.quantile(0.5)"
        );
        let unsafe_name = FunctionRef {
            namespace: None,
            name: "count); DROP TABLE users;--".into(),
        };
        assert!(render_function_ref(SqlBackend::Postgres, &unsafe_name).is_ok());
        assert!(
            render_function_ref(SqlBackend::Postgres, &unsafe_name)
                .unwrap()
                .starts_with('"')
        );
    }

    #[test]
    fn timezone_type_arguments_reject_syntax() {
        assert!(
            backend_type_name(
                SqlBackend::ClickHouse,
                &DataType::Timestamp {
                    timezone: Some("Europe/Oslo".into()),
                }
            )
            .is_ok()
        );
        assert!(
            backend_type_name(
                SqlBackend::ClickHouse,
                &DataType::Timestamp {
                    timezone: Some("UTC'); DROP TABLE users;--".into()),
                }
            )
            .is_err()
        );
    }
}
