#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use queryfabric::{DataType, MemoryCatalog, ParameterValue, QueryParameters, ResultField};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

pub fn portable_catalog(snapshot_id: &str) -> MemoryCatalog {
    queryfabric::portable_catalog(snapshot_id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableSubsetCorpus {
    pub version: String,
    pub cases: Vec<PortableSubsetCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableSubsetCase {
    pub id: String,
    pub query: String,
    #[serde(default, skip_serializing_if = "QueryParameters::is_empty")]
    pub parameters: QueryParameters,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_schema: Vec<ResultField>,
    pub expected_backends: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_bind_error_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_backend_error_codes: BTreeMap<String, Vec<String>>,
}

pub fn portable_subset_seed() -> PortableSubsetCorpus {
    PortableSubsetCorpus {
        version: env!("CARGO_PKG_VERSION").into(),
        cases: vec![
            case("simple-scan-filter-project")
                .query("SELECT record_id FROM records WHERE score > 100 LIMIT 5")
                .capabilities(["LimitOffset"])
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("distinct-order-limit-offset")
                .query("SELECT DISTINCT record_id FROM records ORDER BY record_id LIMIT 5 OFFSET 2")
                .capabilities(["LimitOffset"])
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("aggregate-group-by-having")
                .query("SELECT record_id, AVG(score) AS mean_score FROM records GROUP BY record_id HAVING AVG(score) > 100")
                .capabilities(["Aggregates"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("mean_score", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("case-expression")
                .query("SELECT source_record_id, CASE WHEN weight > 1.0 THEN 1 ELSE 0 END AS bucket FROM links LIMIT 1")
                .capabilities(["LimitOffset"])
                .schema([
                    field("source_record_id", DataType::Uuid, false),
                    field("bucket", DataType::Int64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("common-scalar-functions")
                .query("SELECT COALESCE(weight, 0.0) AS coalesced, SQRT(weight) AS root, GREATEST(weight, 1.0) AS hi, LEAST(weight, 1.0) AS lo FROM links LIMIT 1")
                .capabilities(["LimitOffset"])
                .schema([
                    field("coalesced", DataType::Float64, false),
                    field("root", DataType::Float64, false),
                    field("hi", DataType::Float64, false),
                    field("lo", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("coalesce-fallback")
                .query("SELECT COALESCE(score, 0.0) AS stabilized_score FROM records LIMIT 1")
                .capabilities(["LimitOffset"])
                .schema([field("stabilized_score", DataType::Float64, false)])
                .backends("supported", "supported")
                .build(),
            case("aggregate-family")
                .query("SELECT SUM(weight) AS total_weight, AVG(weight) AS mean_weight, MIN(weight) AS min_weight, MAX(weight) AS max_weight FROM links")
                .capabilities(["Aggregates"])
                .schema([
                    field("total_weight", DataType::Float64, true),
                    field("mean_weight", DataType::Float64, true),
                    field("min_weight", DataType::Float64, true),
                    field("max_weight", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("distinct-aggregate")
                .query("SELECT COUNT(DISTINCT target_record_id) AS distinct_targets FROM links")
                .capabilities(["Aggregates", "DistinctAggregates"])
                .schema([field("distinct_targets", DataType::Int64, false)])
                .backends("supported", "supported")
                .build(),
            case("inner-join")
                .query("SELECT r.record_id, l.weight FROM records AS r INNER JOIN links AS l ON r.record_id = l.target_record_id")
                .capabilities(["Joins"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("weight", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("left-join")
                .query("SELECT r.record_id, l.weight FROM records AS r LEFT JOIN links AS l ON r.record_id = l.target_record_id")
                .capabilities(["Joins"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("weight", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("right-join")
                .query("SELECT r.record_id, l.weight FROM records AS r RIGHT JOIN links AS l ON r.record_id = l.target_record_id")
                .capabilities(["Joins"])
                .schema([
                    field("record_id", DataType::Uuid, true),
                    field("weight", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("full-join")
                .query("SELECT r.record_id, l.weight FROM records AS r FULL JOIN links AS l ON r.record_id = l.target_record_id")
                .capabilities(["Joins"])
                .schema([
                    field("record_id", DataType::Uuid, true),
                    field("weight", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("cross-join")
                .query("SELECT r.record_id, l.weight FROM records AS r CROSS JOIN links AS l")
                .capabilities(["Joins"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("weight", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("derived-subquery")
                .query("SELECT derived.record_id FROM (SELECT record_id FROM records) AS derived")
                .capabilities(["DerivedTables"])
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("non-recursive-cte")
                .query("WITH recent AS (SELECT record_id FROM records) SELECT record_id FROM recent")
                .capabilities(["CommonTableExpressions"])
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("union-all")
                .query("SELECT record_id FROM records UNION ALL SELECT source_record_id FROM links")
                .capabilities(["SetOperations"])
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("scalar-subquery")
                .query("SELECT record_id, (SELECT COUNT(weight) FROM links) AS link_count FROM records")
                .capabilities(["Aggregates", "ScalarSubqueries"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("link_count", DataType::Int64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("in-subquery")
                .query("SELECT record_id FROM records WHERE record_id IN (SELECT target_record_id FROM links)")
                .capabilities(["InSubqueries"])
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("window-rank")
                .query("SELECT record_id, RANK() OVER (ORDER BY score DESC) AS rk FROM records")
                .capabilities(["Windows"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("rk", DataType::Int64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("window-dense-rank-row-number")
                .query("SELECT record_id, DENSE_RANK() OVER (ORDER BY score DESC) AS dr, ROW_NUMBER() OVER (ORDER BY score DESC) AS rn FROM records")
                .capabilities(["Windows"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("dr", DataType::Int64, false),
                    field("rn", DataType::Int64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("window-lag-lead")
                .query("SELECT record_id, LAG(score) OVER (ORDER BY score) AS prev_score, LEAD(score) OVER (ORDER BY score) AS next_score FROM records")
                .capabilities(["Windows"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("prev_score", DataType::Float64, true),
                    field("next_score", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("window-first-last")
                .query("SELECT record_id, FIRST_VALUE(score) OVER (ORDER BY score) AS first_score, LAST_VALUE(score) OVER (ORDER BY score) AS last_score FROM records")
                .capabilities(["Windows"])
                .schema([
                    field("record_id", DataType::Uuid, false),
                    field("first_score", DataType::Float64, true),
                    field("last_score", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("positional-parameter")
                .query("SELECT record_id FROM records WHERE score > $1")
                .parameters(positional_float(1, "100.0"))
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("named-parameter")
                .query("SELECT record_id FROM records WHERE score > :min_len")
                .parameters(named_float("min_len", "50.0"))
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("list-parameter")
                .query("SELECT record_id FROM records WHERE record_id IN ($1)")
                .parameters(positional_uuid_list(
                    1,
                    &[
                        "11111111-1111-1111-1111-111111111111",
                        "22222222-2222-2222-2222-222222222222",
                    ],
                ))
                .schema([field("record_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("clickhouse-only-function")
                .query("SELECT ch.avg_merge(score) FROM records")
                .capabilities(["Aggregates", "NamespacedFunctions"])
                .backends("supported", "rejected")
                .backend_errors([], ["QF21104", "QF21106"])
                .build(),
            case("clickhouse-approximate-aggregate")
                .query("SELECT quantile(score) FROM records")
                .capabilities(["Aggregates", "ApproximateAggregates"])
                .backends("supported", "rejected")
                .backend_errors([], ["QF21104", "QF21105"])
                .build(),
            case("clickhouse-settings")
                .query("SELECT record_id FROM records SETTINGS max_threads = 4")
                .backends("supported", "rejected")
                .backend_errors([], ["QF21101"])
                .build(),
            case("clickhouse-format")
                .query("SELECT record_id FROM records FORMAT JSONCompact")
                .backends("supported", "rejected")
                .backend_errors([], ["QF21102"])
                .build(),
            case("recursive-cte")
                .query("WITH RECURSIVE recent AS (SELECT record_id FROM records) SELECT record_id FROM recent")
                .backends("rejected", "rejected")
                .bind_errors(["QF0001"])
                .build(),
            case("unsupported-set-op")
                .query("SELECT record_id FROM records INTERSECT SELECT record_id FROM records")
                .backends("rejected", "rejected")
                .bind_errors(["QF0002"])
                .build(),
            case("correlated-subquery")
                .query("SELECT record_id FROM records AS r WHERE EXISTS (SELECT 1 FROM links AS l WHERE l.target_record_id = r.record_id)")
                .backends("rejected", "rejected")
                .bind_errors(["QF0014"])
                .build(),
            case("multi-column-scalar-subquery")
                .query("SELECT (SELECT source_record_id, target_record_id FROM links LIMIT 1) AS bad_scalar")
                .backends("rejected", "rejected")
                .bind_errors(["QF0023"])
                .build(),
            case("multi-column-in-subquery")
                .query("SELECT record_id FROM records WHERE record_id IN (SELECT source_record_id, target_record_id FROM links)")
                .backends("rejected", "rejected")
                .bind_errors(["QF0024"])
                .build(),
        ],
    }
}

pub fn portable_subset_seed_json() -> Value {
    serde_json::to_value(portable_subset_seed()).expect("portable subset seed json")
}

fn field(name: &str, data_type: DataType, nullable: bool) -> ResultField {
    ResultField::new(name, data_type, nullable)
}

fn positional_float(position: u32, value: &str) -> QueryParameters {
    let mut parameters = QueryParameters::default();
    parameters.insert_positional(position, ParameterValue::Float64(value.into()));
    parameters
}

fn named_float(name: &str, value: &str) -> QueryParameters {
    let mut parameters = QueryParameters::default();
    parameters.insert_named(name, ParameterValue::Float64(value.into()));
    parameters
}

fn positional_uuid_list(position: u32, values: &[&str]) -> QueryParameters {
    let mut parameters = QueryParameters::default();
    parameters.insert_positional(
        position,
        ParameterValue::List(
            values
                .iter()
                .map(|value| ParameterValue::Uuid((*value).into()))
                .collect(),
        ),
    );
    parameters
}

fn backend_statuses(clickhouse: &str, postgres: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("clickhouse".into(), clickhouse.into()),
        ("postgres".into(), postgres.into()),
    ])
}

fn backend_errors(clickhouse: &[&str], postgres: &[&str]) -> BTreeMap<String, Vec<String>> {
    let mut errors = BTreeMap::new();
    if !clickhouse.is_empty() {
        errors.insert(
            "clickhouse".into(),
            clickhouse.iter().map(|code| (*code).into()).collect(),
        );
    }
    if !postgres.is_empty() {
        errors.insert(
            "postgres".into(),
            postgres.iter().map(|code| (*code).into()).collect(),
        );
    }
    errors
}

fn case(id: &str) -> PortableSubsetCaseBuilder {
    PortableSubsetCaseBuilder::new(id)
}

struct PortableSubsetCaseBuilder {
    id: String,
    query: String,
    parameters: QueryParameters,
    required_capabilities: Vec<String>,
    expected_schema: Vec<ResultField>,
    expected_backends: BTreeMap<String, String>,
    expected_bind_error_codes: Vec<String>,
    expected_backend_error_codes: BTreeMap<String, Vec<String>>,
}

impl PortableSubsetCaseBuilder {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            query: String::new(),
            parameters: QueryParameters::default(),
            required_capabilities: Vec::new(),
            expected_schema: Vec::new(),
            expected_backends: backend_statuses("supported", "supported"),
            expected_bind_error_codes: Vec::new(),
            expected_backend_error_codes: BTreeMap::new(),
        }
    }

    fn query(mut self, query: &str) -> Self {
        self.query = query.into();
        self
    }

    fn parameters(mut self, parameters: QueryParameters) -> Self {
        self.parameters = parameters;
        self
    }

    fn capabilities<const N: usize>(mut self, capabilities: [&str; N]) -> Self {
        self.required_capabilities = capabilities.into_iter().map(str::to_owned).collect();
        self
    }

    fn schema<const N: usize>(mut self, schema: [ResultField; N]) -> Self {
        self.expected_schema = schema.into_iter().collect();
        self
    }

    fn backends(mut self, clickhouse: &str, postgres: &str) -> Self {
        self.expected_backends = backend_statuses(clickhouse, postgres);
        self
    }

    fn bind_errors<const N: usize>(mut self, codes: [&str; N]) -> Self {
        self.expected_bind_error_codes = codes.into_iter().map(str::to_owned).collect();
        self
    }

    fn backend_errors<const C: usize, const P: usize>(
        mut self,
        clickhouse: [&str; C],
        postgres: [&str; P],
    ) -> Self {
        self.expected_backend_error_codes = backend_errors(&clickhouse, &postgres);
        self
    }

    fn build(self) -> PortableSubsetCase {
        PortableSubsetCase {
            id: self.id,
            query: self.query,
            parameters: self.parameters,
            required_capabilities: self.required_capabilities,
            expected_schema: self.expected_schema,
            expected_backends: self.expected_backends,
            expected_bind_error_codes: self.expected_bind_error_codes,
            expected_backend_error_codes: self.expected_backend_error_codes,
        }
    }
}
