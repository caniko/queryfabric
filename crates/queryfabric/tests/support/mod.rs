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
                .query("SELECT neuron_id FROM neurons WHERE cable_length > 100 LIMIT 5")
                .capabilities(["LimitOffset"])
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("distinct-order-limit-offset")
                .query("SELECT DISTINCT neuron_id FROM neurons ORDER BY neuron_id LIMIT 5 OFFSET 2")
                .capabilities(["LimitOffset"])
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("aggregate-group-by-having")
                .query("SELECT neuron_id, AVG(cable_length) AS mean_len FROM neurons GROUP BY neuron_id HAVING AVG(cable_length) > 100")
                .capabilities(["Aggregates"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("mean_len", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("case-expression")
                .query("SELECT source_neuron_id, CASE WHEN weight > 1.0 THEN 1 ELSE 0 END AS bucket FROM synapses LIMIT 1")
                .capabilities(["LimitOffset"])
                .schema([
                    field("source_neuron_id", DataType::Uuid, false),
                    field("bucket", DataType::Int64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("common-scalar-functions")
                .query("SELECT COALESCE(weight, 0.0) AS coalesced, SQRT(weight) AS root, GREATEST(weight, 1.0) AS hi, LEAST(weight, 1.0) AS lo FROM synapses LIMIT 1")
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
                .query("SELECT COALESCE(cable_length, 0.0) AS stabilized_length FROM neurons LIMIT 1")
                .capabilities(["LimitOffset"])
                .schema([field("stabilized_length", DataType::Float64, false)])
                .backends("supported", "supported")
                .build(),
            case("aggregate-family")
                .query("SELECT SUM(weight) AS total_weight, AVG(weight) AS mean_weight, MIN(weight) AS min_weight, MAX(weight) AS max_weight FROM synapses")
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
                .query("SELECT COUNT(DISTINCT target_neuron_id) AS distinct_targets FROM synapses")
                .capabilities(["Aggregates", "DistinctAggregates"])
                .schema([field("distinct_targets", DataType::Int64, false)])
                .backends("supported", "supported")
                .build(),
            case("inner-join")
                .query("SELECT n.neuron_id, s.weight FROM neurons AS n INNER JOIN synapses AS s ON n.neuron_id = s.target_neuron_id")
                .capabilities(["Joins"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("weight", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("left-join")
                .query("SELECT n.neuron_id, s.weight FROM neurons AS n LEFT JOIN synapses AS s ON n.neuron_id = s.target_neuron_id")
                .capabilities(["Joins"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("weight", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("right-join")
                .query("SELECT n.neuron_id, s.weight FROM neurons AS n RIGHT JOIN synapses AS s ON n.neuron_id = s.target_neuron_id")
                .capabilities(["Joins"])
                .schema([
                    field("neuron_id", DataType::Uuid, true),
                    field("weight", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("full-join")
                .query("SELECT n.neuron_id, s.weight FROM neurons AS n FULL JOIN synapses AS s ON n.neuron_id = s.target_neuron_id")
                .capabilities(["Joins"])
                .schema([
                    field("neuron_id", DataType::Uuid, true),
                    field("weight", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("cross-join")
                .query("SELECT n.neuron_id, s.weight FROM neurons AS n CROSS JOIN synapses AS s")
                .capabilities(["Joins"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("weight", DataType::Float64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("derived-subquery")
                .query("SELECT derived.neuron_id FROM (SELECT neuron_id FROM neurons) AS derived")
                .capabilities(["DerivedTables"])
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("non-recursive-cte")
                .query("WITH recent AS (SELECT neuron_id FROM neurons) SELECT neuron_id FROM recent")
                .capabilities(["CommonTableExpressions"])
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("union-all")
                .query("SELECT neuron_id FROM neurons UNION ALL SELECT source_neuron_id FROM synapses")
                .capabilities(["SetOperations"])
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("scalar-subquery")
                .query("SELECT neuron_id, (SELECT COUNT(weight) FROM synapses) AS synapse_count FROM neurons")
                .capabilities(["Aggregates", "ScalarSubqueries"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("synapse_count", DataType::Int64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("in-subquery")
                .query("SELECT neuron_id FROM neurons WHERE neuron_id IN (SELECT target_neuron_id FROM synapses)")
                .capabilities(["InSubqueries"])
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("window-rank")
                .query("SELECT neuron_id, RANK() OVER (ORDER BY cable_length DESC) AS rk FROM neurons")
                .capabilities(["Windows"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("rk", DataType::Int64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("window-dense-rank-row-number")
                .query("SELECT neuron_id, DENSE_RANK() OVER (ORDER BY cable_length DESC) AS dr, ROW_NUMBER() OVER (ORDER BY cable_length DESC) AS rn FROM neurons")
                .capabilities(["Windows"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("dr", DataType::Int64, false),
                    field("rn", DataType::Int64, false),
                ])
                .backends("supported", "supported")
                .build(),
            case("window-lag-lead")
                .query("SELECT neuron_id, LAG(cable_length) OVER (ORDER BY cable_length) AS prev_len, LEAD(cable_length) OVER (ORDER BY cable_length) AS next_len FROM neurons")
                .capabilities(["Windows"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("prev_len", DataType::Float64, true),
                    field("next_len", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("window-first-last")
                .query("SELECT neuron_id, FIRST_VALUE(cable_length) OVER (ORDER BY cable_length) AS first_len, LAST_VALUE(cable_length) OVER (ORDER BY cable_length) AS last_len FROM neurons")
                .capabilities(["Windows"])
                .schema([
                    field("neuron_id", DataType::Uuid, false),
                    field("first_len", DataType::Float64, true),
                    field("last_len", DataType::Float64, true),
                ])
                .backends("supported", "supported")
                .build(),
            case("positional-parameter")
                .query("SELECT neuron_id FROM neurons WHERE cable_length > $1")
                .parameters(positional_float(1, "100.0"))
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("named-parameter")
                .query("SELECT neuron_id FROM neurons WHERE cable_length > :min_len")
                .parameters(named_float("min_len", "50.0"))
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("list-parameter")
                .query("SELECT neuron_id FROM neurons WHERE neuron_id IN ($1)")
                .parameters(positional_uuid_list(
                    1,
                    &[
                        "11111111-1111-1111-1111-111111111111",
                        "22222222-2222-2222-2222-222222222222",
                    ],
                ))
                .schema([field("neuron_id", DataType::Uuid, false)])
                .backends("supported", "supported")
                .build(),
            case("clickhouse-only-function")
                .query("SELECT ch.avg_merge(cable_length) FROM neurons")
                .capabilities(["Aggregates", "NamespacedFunctions"])
                .backends("supported", "rejected")
                .backend_errors([], ["QF21104", "QF21106"])
                .build(),
            case("clickhouse-approximate-aggregate")
                .query("SELECT quantile(cable_length) FROM neurons")
                .capabilities(["Aggregates", "ApproximateAggregates"])
                .backends("supported", "rejected")
                .backend_errors([], ["QF21104", "QF21105"])
                .build(),
            case("clickhouse-settings")
                .query("SELECT neuron_id FROM neurons SETTINGS max_threads = 4")
                .backends("supported", "rejected")
                .backend_errors([], ["QF21101"])
                .build(),
            case("clickhouse-format")
                .query("SELECT neuron_id FROM neurons FORMAT JSONCompact")
                .backends("supported", "rejected")
                .backend_errors([], ["QF21102"])
                .build(),
            case("recursive-cte")
                .query("WITH RECURSIVE recent AS (SELECT neuron_id FROM neurons) SELECT neuron_id FROM recent")
                .backends("rejected", "rejected")
                .bind_errors(["QF0001"])
                .build(),
            case("unsupported-set-op")
                .query("SELECT neuron_id FROM neurons INTERSECT SELECT neuron_id FROM neurons")
                .backends("rejected", "rejected")
                .bind_errors(["QF0002"])
                .build(),
            case("correlated-subquery")
                .query("SELECT neuron_id FROM neurons AS n WHERE EXISTS (SELECT 1 FROM synapses AS s WHERE s.target_neuron_id = n.neuron_id)")
                .backends("rejected", "rejected")
                .bind_errors(["QF0014"])
                .build(),
            case("multi-column-scalar-subquery")
                .query("SELECT (SELECT source_neuron_id, target_neuron_id FROM synapses LIMIT 1) AS bad_scalar")
                .backends("rejected", "rejected")
                .bind_errors(["QF0023"])
                .build(),
            case("multi-column-in-subquery")
                .query("SELECT neuron_id FROM neurons WHERE neuron_id IN (SELECT source_neuron_id, target_neuron_id FROM synapses)")
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
