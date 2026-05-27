use std::collections::BTreeMap;

use queryfabric_ir::DataType;

use crate::model::{BackendFunctionMapping, FunctionKind, FunctionSignature, FunctionVolatility};

pub fn portable_builtin_functions() -> Vec<FunctionSignature> {
    vec![
        builtin_scalar("coalesce", DataType::Unknown, true),
        builtin_scalar("sqrt", DataType::Float64, false).with_args(vec![DataType::Float64]),
        builtin_clickhouse_scalar("toString", DataType::Utf8, false),
        builtin_scalar("greatest", DataType::Unknown, true),
        builtin_scalar("least", DataType::Unknown, true),
        builtin_aggregate("count", DataType::Int64),
        builtin_aggregate("sum", DataType::Float64),
        builtin_aggregate("avg", DataType::Float64),
        builtin_aggregate("min", DataType::Unknown),
        builtin_aggregate("max", DataType::Unknown),
        builtin_window("rank", DataType::Int64),
        builtin_window("dense_rank", DataType::Int64),
        builtin_window("row_number", DataType::Int64),
        builtin_window("lag", DataType::Unknown),
        builtin_window("lead", DataType::Unknown),
        builtin_window("first_value", DataType::Unknown),
        builtin_window("last_value", DataType::Unknown),
        builtin_clickhouse_aggregate("quantile", DataType::Float64)
            .with_metadata_flag("approximate"),
        builtin_clickhouse_extension("ch", "quantile_25", "quantile(0.25)", DataType::Float64)
            .with_metadata_flag("approximate"),
        builtin_clickhouse_extension("ch", "quantile_50", "quantile(0.5)", DataType::Float64)
            .with_metadata_flag("approximate"),
        builtin_clickhouse_extension("ch", "quantile_75", "quantile(0.75)", DataType::Float64)
            .with_metadata_flag("approximate"),
        builtin_clickhouse_extension("ch", "avg_merge", "avgMerge", DataType::Float64)
            .with_metadata_flag("state"),
        builtin_clickhouse_extension("ch", "count_merge", "countMerge", DataType::Int64)
            .with_metadata_flag("state"),
        builtin_clickhouse_extension("ch", "min_merge", "minMerge", DataType::Unknown)
            .with_metadata_flag("state"),
        builtin_clickhouse_extension("ch", "max_merge", "maxMerge", DataType::Unknown)
            .with_metadata_flag("state"),
        builtin_clickhouse_extension("ch", "stddevpop_merge", "stddevPopMerge", DataType::Float64)
            .with_metadata_flag("state"),
        builtin_clickhouse_extension("ch", "sum_merge", "sumMerge", DataType::Float64)
            .with_metadata_flag("state"),
        builtin_clickhouse_extension("ch", "varpop_merge", "varPopMerge", DataType::Float64)
            .with_metadata_flag("state"),
    ]
}

pub fn builtin_function_signature(
    namespace: Option<&str>,
    name: &str,
) -> Option<FunctionSignature> {
    portable_builtin_functions().into_iter().find(|signature| {
        signature.namespace.as_deref() == namespace && signature.name.eq_ignore_ascii_case(name)
    })
}

fn builtin_scalar(name: &str, return_type: DataType, variadic: bool) -> FunctionSignature {
    FunctionSignature {
        namespace: None,
        name: name.into(),
        kind: FunctionKind::Scalar,
        volatility: FunctionVolatility::Immutable,
        arg_types: Vec::new(),
        return_type,
        variadic,
        coercions: Vec::new(),
        backend_mappings: vec![
            BackendFunctionMapping {
                backend: "clickhouse".into(),
                namespace: None,
                name: name.into(),
            },
            BackendFunctionMapping {
                backend: "postgres".into(),
                namespace: None,
                name: name.into(),
            },
        ],
        metadata: BTreeMap::new(),
    }
}

fn builtin_aggregate(name: &str, return_type: DataType) -> FunctionSignature {
    FunctionSignature {
        namespace: None,
        name: name.into(),
        kind: FunctionKind::Aggregate,
        volatility: FunctionVolatility::Immutable,
        arg_types: Vec::new(),
        return_type,
        variadic: true,
        coercions: Vec::new(),
        backend_mappings: vec![
            BackendFunctionMapping {
                backend: "clickhouse".into(),
                namespace: None,
                name: name.into(),
            },
            BackendFunctionMapping {
                backend: "postgres".into(),
                namespace: None,
                name: name.into(),
            },
        ],
        metadata: BTreeMap::new(),
    }
}

fn builtin_clickhouse_aggregate(name: &str, return_type: DataType) -> FunctionSignature {
    FunctionSignature {
        namespace: None,
        name: name.into(),
        kind: FunctionKind::Aggregate,
        volatility: FunctionVolatility::Immutable,
        arg_types: Vec::new(),
        return_type,
        variadic: true,
        coercions: Vec::new(),
        backend_mappings: vec![BackendFunctionMapping {
            backend: "clickhouse".into(),
            namespace: None,
            name: name.into(),
        }],
        metadata: BTreeMap::new(),
    }
}

fn builtin_clickhouse_scalar(
    name: &str,
    return_type: DataType,
    variadic: bool,
) -> FunctionSignature {
    FunctionSignature {
        namespace: None,
        name: name.into(),
        kind: FunctionKind::Scalar,
        volatility: FunctionVolatility::Immutable,
        arg_types: Vec::new(),
        return_type,
        variadic,
        coercions: Vec::new(),
        backend_mappings: vec![BackendFunctionMapping {
            backend: "clickhouse".into(),
            namespace: None,
            name: name.into(),
        }],
        metadata: BTreeMap::new(),
    }
}

fn builtin_clickhouse_extension(
    namespace: &str,
    name: &str,
    clickhouse_name: &str,
    return_type: DataType,
) -> FunctionSignature {
    FunctionSignature {
        namespace: Some(namespace.into()),
        name: name.into(),
        kind: FunctionKind::Aggregate,
        volatility: FunctionVolatility::Immutable,
        arg_types: Vec::new(),
        return_type,
        variadic: true,
        coercions: Vec::new(),
        backend_mappings: vec![BackendFunctionMapping {
            backend: "clickhouse".into(),
            namespace: None,
            name: clickhouse_name.into(),
        }],
        metadata: BTreeMap::new(),
    }
}

fn builtin_window(name: &str, return_type: DataType) -> FunctionSignature {
    FunctionSignature {
        namespace: None,
        name: name.into(),
        kind: FunctionKind::Window,
        volatility: FunctionVolatility::Immutable,
        arg_types: Vec::new(),
        return_type,
        variadic: true,
        coercions: Vec::new(),
        backend_mappings: vec![
            BackendFunctionMapping {
                backend: "clickhouse".into(),
                namespace: None,
                name: name.into(),
            },
            BackendFunctionMapping {
                backend: "postgres".into(),
                namespace: None,
                name: name.into(),
            },
        ],
        metadata: BTreeMap::new(),
    }
}

trait FunctionSignatureExt {
    fn with_args(self, arg_types: Vec<DataType>) -> Self;
    fn with_metadata_flag(self, key: &str) -> Self;
}

impl FunctionSignatureExt for FunctionSignature {
    fn with_args(mut self, arg_types: Vec<DataType>) -> Self {
        self.arg_types = arg_types;
        self
    }

    fn with_metadata_flag(mut self, key: &str) -> Self {
        self.metadata.insert(key.into(), "true".into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_clickhouse_state_merge_extensions() {
        let stddev = builtin_function_signature(Some("ch"), "stddevpop_merge")
            .expect("stddevpop merge builtin");
        assert_eq!(stddev.backend_mappings[0].name, "stddevPopMerge");
        assert_eq!(stddev.return_type, DataType::Float64);
        assert_eq!(
            stddev.metadata.get("state").map(String::as_str),
            Some("true")
        );

        let var =
            builtin_function_signature(Some("ch"), "varpop_merge").expect("varpop merge builtin");
        assert_eq!(var.backend_mappings[0].name, "varPopMerge");
        assert_eq!(var.return_type, DataType::Float64);
        assert_eq!(var.metadata.get("state").map(String::as_str), Some("true"));

        let min = builtin_function_signature(Some("ch"), "min_merge").expect("min merge builtin");
        assert_eq!(min.backend_mappings[0].name, "minMerge");
        assert_eq!(min.return_type, DataType::Unknown);
        assert_eq!(min.metadata.get("state").map(String::as_str), Some("true"));
    }

    #[test]
    fn resolves_clickhouse_quantile_extensions() {
        let q50 = builtin_function_signature(Some("ch"), "quantile_50")
            .expect("quantile extension builtin");
        assert_eq!(q50.backend_mappings[0].name, "quantile(0.5)");
        assert_eq!(q50.return_type, DataType::Float64);
        assert_eq!(
            q50.metadata.get("approximate").map(String::as_str),
            Some("true")
        );
    }
}
