use queryfabric_ir::{BinaryOperator, DataType, FunctionRef, NameRef, ParameterRef};
use sqlparser::ast::{
    BinaryOperator as SqlBinaryOperator, DataType as SqlDataType, UnaryOperator as SqlUnaryOperator,
};

pub(crate) fn strip_leading_explain(input: &str) -> (bool, &str, usize) {
    let trimmed_start = input.len() - input.trim_start().len();
    let trimmed = &input[trimmed_start..];
    let upper = trimmed.to_ascii_uppercase();
    if upper == "EXPLAIN" {
        (true, "", input.len())
    } else if upper.starts_with("EXPLAIN")
        && trimmed
            .get(7..)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        let explain_rest = &trimmed[7..];
        let rest_trimmed = explain_rest.trim_start();
        let body_offset = input.len() - rest_trimmed.len();
        (true, rest_trimmed, body_offset)
    } else {
        (false, trimmed, trimmed_start)
    }
}

pub(crate) fn lower_name_ref(name: &sqlparser::ast::ObjectName) -> NameRef {
    let parts = name
        .0
        .iter()
        .map(|part| match part {
            sqlparser::ast::ObjectNamePart::Identifier(identifier) => identifier.value.clone(),
            sqlparser::ast::ObjectNamePart::Function(function) => function.to_string(),
        })
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [name] => NameRef {
            namespace: None,
            name: name.clone(),
        },
        [namespace, name] => NameRef {
            namespace: Some(namespace.clone()),
            name: name.clone(),
        },
        _ => NameRef {
            namespace: None,
            name: parts.join("."),
        },
    }
}

pub(crate) fn lower_function_ref(name: &sqlparser::ast::ObjectName) -> FunctionRef {
    let parts = name
        .0
        .iter()
        .map(|part| match part {
            sqlparser::ast::ObjectNamePart::Identifier(identifier) => identifier.value.clone(),
            sqlparser::ast::ObjectNamePart::Function(function) => function.to_string(),
        })
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [name] => FunctionRef {
            namespace: None,
            name: name.to_ascii_lowercase(),
        },
        [namespace, name] => FunctionRef {
            namespace: Some(namespace.to_ascii_lowercase()),
            name: name.to_ascii_lowercase(),
        },
        _ => FunctionRef {
            namespace: None,
            name: parts.join(".").to_ascii_lowercase(),
        },
    }
}

pub(crate) fn placeholder_to_parameter_ref(placeholder: &str) -> ParameterRef {
    if placeholder == "?" {
        ParameterRef::Positional(0)
    } else if let Some(number) = placeholder.strip_prefix('$') {
        number
            .parse::<u32>()
            .map(ParameterRef::Positional)
            .unwrap_or_else(|_| ParameterRef::Named(number.to_owned()))
    } else {
        ParameterRef::Named(placeholder.trim_start_matches([':', '@']).to_owned())
    }
}

pub(crate) fn lower_binary_operator(op: &SqlBinaryOperator) -> Option<BinaryOperator> {
    match op {
        SqlBinaryOperator::Plus => Some(BinaryOperator::Add),
        SqlBinaryOperator::Minus => Some(BinaryOperator::Subtract),
        SqlBinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        SqlBinaryOperator::Divide => Some(BinaryOperator::Divide),
        SqlBinaryOperator::Eq => Some(BinaryOperator::Eq),
        SqlBinaryOperator::NotEq => Some(BinaryOperator::NotEq),
        SqlBinaryOperator::Lt => Some(BinaryOperator::Lt),
        SqlBinaryOperator::LtEq => Some(BinaryOperator::LtEq),
        SqlBinaryOperator::Gt => Some(BinaryOperator::Gt),
        SqlBinaryOperator::GtEq => Some(BinaryOperator::GtEq),
        SqlBinaryOperator::And => Some(BinaryOperator::And),
        SqlBinaryOperator::Or => Some(BinaryOperator::Or),
        _ => None,
    }
}

pub(crate) fn lower_unary_operator(op: &SqlUnaryOperator) -> Option<queryfabric_ir::UnaryOperator> {
    match op {
        SqlUnaryOperator::Plus => Some(queryfabric_ir::UnaryOperator::Plus),
        SqlUnaryOperator::Minus => Some(queryfabric_ir::UnaryOperator::Minus),
        SqlUnaryOperator::Not => Some(queryfabric_ir::UnaryOperator::Not),
        _ => None,
    }
}

pub(crate) fn lower_data_type(data_type: &SqlDataType) -> DataType {
    use sqlparser::ast::DataType as SqlType;

    match data_type {
        SqlType::Boolean => DataType::Boolean,
        SqlType::Int(_) | SqlType::Integer(_) | SqlType::Int4(_) => DataType::Int32,
        SqlType::BigInt(_) | SqlType::Int8(_) => DataType::Int64,
        SqlType::Float(_) | SqlType::Double(_) | SqlType::DoublePrecision => DataType::Float64,
        SqlType::Text | SqlType::String(_) | SqlType::Varchar(_) | SqlType::Char(_) => {
            DataType::Utf8
        }
        SqlType::Uuid => DataType::Uuid,
        SqlType::JSON | SqlType::JSONB => DataType::Json,
        SqlType::Date => DataType::Date,
        SqlType::Decimal(info) | SqlType::Numeric(info) => {
            let (precision, scale) = match info {
                sqlparser::ast::ExactNumberInfo::None => (38, 0),
                sqlparser::ast::ExactNumberInfo::Precision(precision) => (*precision as u8, 0),
                sqlparser::ast::ExactNumberInfo::PrecisionAndScale(precision, scale) => {
                    (*precision as u8, *scale as i8)
                }
            };
            DataType::Decimal { precision, scale }
        }
        SqlType::Timestamp(_, timezone) => DataType::Timestamp {
            timezone: match timezone {
                sqlparser::ast::TimezoneInfo::None
                | sqlparser::ast::TimezoneInfo::WithoutTimeZone => None,
                sqlparser::ast::TimezoneInfo::WithTimeZone | sqlparser::ast::TimezoneInfo::Tz => {
                    Some("UTC".into())
                }
            },
        },
        SqlType::Array(element) => {
            let inner = match element {
                sqlparser::ast::ArrayElemTypeDef::None => DataType::Unknown,
                sqlparser::ast::ArrayElemTypeDef::AngleBracket(inner)
                | sqlparser::ast::ArrayElemTypeDef::SquareBracket(inner, _)
                | sqlparser::ast::ArrayElemTypeDef::Parenthesis(inner) => lower_data_type(inner),
            };
            DataType::List(Box::new(inner))
        }
        _ => DataType::Unknown,
    }
}
