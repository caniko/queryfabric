use std::collections::HashMap;

use pyo3::Bound;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use pythonize::pythonize;
use queryfabric::{
    BackendAnalysis, BoundQuery, CatalogDocument, ClickHouseAdapter, ColumnSchema, DataType,
    GenericSqlDialect, MemoryCatalog, ParsedQuery, PostgresAdapter, QueryCompiler, QueryParameters,
    RelationKind, RelationSchema, SqlArtifact, SyqlDialect, inspect_parameters, inspect_query,
    parameter_value_from_json,
};

pyo3::create_exception!(_queryfabric, QueryFabricError, PyException);

#[pyclass(
    name = "ParsedQuery",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyParsedQuery {
    inner: ParsedQuery,
}

#[pyclass(
    name = "BoundQuery",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyBoundQuery {
    inner: BoundQuery,
}

#[pyclass(
    name = "ParameterSummary",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyParameterSummary {
    inner: queryfabric::ParameterSummary,
}

#[pyclass(
    name = "BackendAnalysis",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyBackendAnalysis {
    inner: BackendAnalysis,
}

#[pyclass(
    name = "SqlArtifact",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PySqlArtifact {
    inner: SqlArtifact,
}

#[pyclass(name = "MemoryCatalog", module = "queryfabric", skip_from_py_object)]
#[derive(Clone)]
struct PyMemoryCatalog {
    inner: MemoryCatalog,
}

#[pyclass(
    name = "RelationSchema",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyRelationSchema {
    inner: RelationSchema,
}

#[pyclass(
    name = "ColumnSchema",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyColumnSchema {
    inner: ColumnSchema,
}

#[pyclass(name = "DataType", module = "queryfabric", frozen, skip_from_py_object)]
#[derive(Clone)]
struct PyDataType {
    inner: DataType,
}

#[pyclass(
    name = "RelationKind",
    module = "queryfabric",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyRelationKind {
    inner: RelationKind,
}

#[pyclass(name = "QueryParameters", module = "queryfabric", skip_from_py_object)]
#[derive(Clone, Default)]
struct PyQueryParameters {
    inner: QueryParameters,
}

#[pymethods]
impl PyParsedQuery {
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &inspect_query(&self.inner, None))
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    #[getter]
    fn dialect(&self) -> String {
        self.inner.dialect().to_owned()
    }

    #[getter]
    fn rendered_sql(&self) -> String {
        self.inner.rendered_sql().to_owned()
    }

    #[getter]
    fn table(&self) -> String {
        inspect_query(&self.inner, None)
            .primary_relation
            .unwrap_or_else(|| "<query>".to_owned())
    }

    #[getter]
    fn columns(&self) -> Option<Vec<String>> {
        inspect_query(&self.inner, None).projected_columns
    }

    #[getter]
    fn predicate_count(&self) -> usize {
        inspect_query(&self.inner, None).predicate_count
    }

    #[getter]
    fn limit(&self) -> Option<u64> {
        inspect_query(&self.inner, None).row_limit
    }

    #[getter]
    fn scope(&self) -> String {
        inspect_query(&self.inner, None).scope
    }

    #[getter]
    fn output_format(&self) -> String {
        inspect_query(&self.inner, None).output_format
    }

    fn __repr__(&self) -> String {
        format!(
            "ParsedQuery(dialect={:?}, sql={:?})",
            self.dialect(),
            self.rendered_sql()
        )
    }
}

#[pymethods]
impl PyBoundQuery {
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    #[getter]
    fn result_schema<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, self.inner.result_schema())
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("BoundQuery(sql={:?})", self.inner.parsed().rendered_sql())
    }
}

#[pymethods]
impl PyParameterSummary {
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    #[getter]
    fn positional_count(&self) -> u32 {
        self.inner.positional_count
    }

    #[getter]
    fn named_params(&self) -> Vec<String> {
        self.inner.named_params.clone()
    }
}

#[pymethods]
impl PyBackendAnalysis {
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    #[getter]
    fn supported(&self) -> bool {
        self.inner.supported
    }
}

#[pymethods]
impl PySqlArtifact {
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    #[getter]
    fn dialect(&self) -> String {
        self.inner.dialect.clone()
    }

    #[getter]
    fn text(&self) -> String {
        self.inner.text.clone()
    }
}

#[pymethods]
impl PyMemoryCatalog {
    #[new]
    fn new() -> Self {
        Self {
            inner: MemoryCatalog::default(),
        }
    }

    fn set_snapshot_id(&mut self, snapshot_id: &str) {
        self.inner.set_snapshot_id(snapshot_id);
    }

    fn register_relation(&mut self, relation: PyRef<'_, PyRelationSchema>) {
        self.inner.register_relation(relation.inner.clone());
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner.to_document())
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner.to_document())
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    #[staticmethod]
    fn from_dict(document: &Bound<'_, PyAny>) -> PyResult<Self> {
        let value = json_value_from_py(document)?;
        let document: CatalogDocument = serde_json::from_value(value)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))?;
        Ok(Self {
            inner: MemoryCatalog::from_document(document),
        })
    }

    #[staticmethod]
    fn from_json(document: &str) -> PyResult<Self> {
        let document: CatalogDocument = serde_json::from_str(document)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))?;
        Ok(Self {
            inner: MemoryCatalog::from_document(document),
        })
    }
}

#[pymethods]
impl PyRelationSchema {
    #[new]
    #[pyo3(signature = (name, columns, namespace=None, aliases=None, kind=None, metadata=None))]
    fn new(
        py: Python<'_>,
        name: String,
        columns: Vec<Py<PyColumnSchema>>,
        namespace: Option<String>,
        aliases: Option<Vec<String>>,
        kind: Option<Py<PyRelationKind>>,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        let columns = columns
            .into_iter()
            .map(|column| column.borrow(py).inner.clone())
            .collect();
        let kind = kind
            .map(|kind| kind.borrow(py).inner.clone())
            .unwrap_or(RelationKind::Table);
        Self {
            inner: RelationSchema {
                namespace,
                name,
                aliases: aliases.unwrap_or_default(),
                kind,
                columns,
                metadata: metadata.unwrap_or_default().into_iter().collect(),
            },
        }
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }
}

#[pymethods]
impl PyColumnSchema {
    #[new]
    #[pyo3(signature = (name, data_type, nullable=true, metadata=None))]
    fn new(
        py: Python<'_>,
        name: String,
        data_type: Py<PyDataType>,
        nullable: bool,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            inner: ColumnSchema {
                name,
                data_type: data_type.borrow(py).inner.clone(),
                nullable,
                metadata: field_metadata(metadata),
            },
        }
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }
}

#[pymethods]
impl PyDataType {
    #[staticmethod]
    fn from_name(name: &str) -> PyResult<Self> {
        Ok(Self {
            inner: data_type_from_name(name)?,
        })
    }

    #[staticmethod]
    fn boolean() -> Self {
        Self {
            inner: DataType::Boolean,
        }
    }

    #[staticmethod]
    fn int32() -> Self {
        Self {
            inner: DataType::Int32,
        }
    }

    #[staticmethod]
    fn int64() -> Self {
        Self {
            inner: DataType::Int64,
        }
    }

    #[staticmethod]
    fn float64() -> Self {
        Self {
            inner: DataType::Float64,
        }
    }

    #[staticmethod]
    fn utf8() -> Self {
        Self {
            inner: DataType::Utf8,
        }
    }

    #[staticmethod]
    fn uuid() -> Self {
        Self {
            inner: DataType::Uuid,
        }
    }

    #[staticmethod]
    fn json() -> Self {
        Self {
            inner: DataType::Json,
        }
    }

    #[staticmethod]
    fn date() -> Self {
        Self {
            inner: DataType::Date,
        }
    }

    #[staticmethod]
    fn decimal(precision: u8, scale: i8) -> Self {
        Self {
            inner: DataType::Decimal { precision, scale },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (timezone=None))]
    fn timestamp(timezone: Option<String>) -> Self {
        Self {
            inner: DataType::Timestamp { timezone },
        }
    }

    #[staticmethod]
    fn list(py: Python<'_>, inner: Py<PyDataType>) -> Self {
        Self {
            inner: DataType::List(Box::new(inner.borrow(py).inner.clone())),
        }
    }

    #[staticmethod]
    fn unknown() -> Self {
        Self {
            inner: DataType::Unknown,
        }
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn __repr__(&self) -> PyResult<String> {
        self.to_json()
    }
}

#[pymethods]
impl PyRelationKind {
    #[staticmethod]
    fn from_name(name: &str) -> PyResult<Self> {
        let inner = match name.to_ascii_lowercase().as_str() {
            "table" => RelationKind::Table,
            "view" => RelationKind::View,
            "materialized_view" | "materialized-view" | "materializedview" => {
                RelationKind::MaterializedView
            }
            other => {
                return Err(QueryFabricError::new_err(format!(
                    "unsupported relation kind `{other}`"
                )));
            }
        };
        Ok(Self { inner })
    }

    #[staticmethod]
    fn table() -> Self {
        Self {
            inner: RelationKind::Table,
        }
    }

    #[staticmethod]
    fn view() -> Self {
        Self {
            inner: RelationKind::View,
        }
    }

    #[staticmethod]
    fn materialized_view() -> Self {
        Self {
            inner: RelationKind::MaterializedView,
        }
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn __repr__(&self) -> PyResult<String> {
        self.to_json()
    }
}

#[pymethods]
impl PyQueryParameters {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    fn insert_positional(&mut self, position: u32, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .insert_positional(position, parameter_value_from_py(value)?);
        Ok(())
    }

    fn insert_named(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .insert_named(name, parameter_value_from_py(value)?);
        Ok(())
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize(py, &self.inner).map_err(|error| QueryFabricError::new_err(error.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|error| QueryFabricError::new_err(error.to_string()))
    }
}

#[pyfunction]
fn parse_sql(text: &str) -> PyResult<PyParsedQuery> {
    Ok(PyParsedQuery {
        inner: QueryCompiler::default()
            .parse(&GenericSqlDialect, text)
            .map_err(to_py_err)?,
    })
}

#[pyfunction]
fn parse_syql(text: &str) -> PyResult<PyParsedQuery> {
    Ok(PyParsedQuery {
        inner: QueryCompiler::default()
            .parse(&SyqlDialect, text)
            .map_err(to_py_err)?,
    })
}

#[pyfunction(name = "inspect_parameters")]
fn inspect_parameters_py(parsed: PyRef<'_, PyParsedQuery>) -> PyParameterSummary {
    PyParameterSummary {
        inner: inspect_parameters(&parsed.inner),
    }
}

#[pyfunction(name = "bind_and_validate", signature = (parsed, catalog, params=None))]
fn bind_and_validate_py(
    parsed: PyRef<'_, PyParsedQuery>,
    catalog: PyRef<'_, PyMemoryCatalog>,
    params: Option<PyRef<'_, PyQueryParameters>>,
) -> PyResult<PyBoundQuery> {
    let empty = QueryParameters::default();
    let params = params.as_ref().map(|value| &value.inner).unwrap_or(&empty);
    Ok(PyBoundQuery {
        inner: QueryCompiler::default()
            .bind_and_validate(&parsed.inner, &catalog.inner, params)
            .map_err(to_py_err)?,
    })
}

#[pyfunction]
fn analyze_clickhouse(
    bound: PyRef<'_, PyBoundQuery>,
    catalog: PyRef<'_, PyMemoryCatalog>,
) -> PyBackendAnalysis {
    PyBackendAnalysis {
        inner: QueryCompiler::default().analyze(&bound.inner, &ClickHouseAdapter, &catalog.inner),
    }
}

#[pyfunction]
fn analyze_postgres(
    bound: PyRef<'_, PyBoundQuery>,
    catalog: PyRef<'_, PyMemoryCatalog>,
) -> PyBackendAnalysis {
    PyBackendAnalysis {
        inner: QueryCompiler::default().analyze(&bound.inner, &PostgresAdapter, &catalog.inner),
    }
}

#[pyfunction]
fn emit_clickhouse_sql(
    bound: PyRef<'_, PyBoundQuery>,
    catalog: PyRef<'_, PyMemoryCatalog>,
) -> PyResult<PySqlArtifact> {
    let artifact = QueryCompiler::default()
        .emit(&bound.inner, &ClickHouseAdapter, &catalog.inner)
        .map_err(to_py_err)?;
    Ok(PySqlArtifact {
        inner: artifact
            .as_sql()
            .cloned()
            .ok_or_else(|| QueryFabricError::new_err("expected SQL artifact"))?,
    })
}

#[pyfunction]
fn emit_postgres_sql(
    bound: PyRef<'_, PyBoundQuery>,
    catalog: PyRef<'_, PyMemoryCatalog>,
) -> PyResult<PySqlArtifact> {
    let artifact = QueryCompiler::default()
        .emit(&bound.inner, &PostgresAdapter, &catalog.inner)
        .map_err(to_py_err)?;
    Ok(PySqlArtifact {
        inner: artifact
            .as_sql()
            .cloned()
            .ok_or_else(|| QueryFabricError::new_err("expected SQL artifact"))?,
    })
}

#[pymodule]
fn _queryfabric(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("QueryFabricError", m.py().get_type::<QueryFabricError>())?;
    m.add_class::<PyParsedQuery>()?;
    m.add_class::<PyBoundQuery>()?;
    m.add_class::<PyParameterSummary>()?;
    m.add_class::<PyBackendAnalysis>()?;
    m.add_class::<PySqlArtifact>()?;
    m.add_class::<PyMemoryCatalog>()?;
    m.add_class::<PyRelationSchema>()?;
    m.add_class::<PyColumnSchema>()?;
    m.add_class::<PyDataType>()?;
    m.add_class::<PyRelationKind>()?;
    m.add_class::<PyQueryParameters>()?;
    m.add_function(wrap_pyfunction!(parse_sql, m)?)?;
    m.add_function(wrap_pyfunction!(parse_syql, m)?)?;
    m.add_function(wrap_pyfunction!(inspect_parameters_py, m)?)?;
    m.add_function(wrap_pyfunction!(bind_and_validate_py, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_clickhouse, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_postgres, m)?)?;
    m.add_function(wrap_pyfunction!(emit_clickhouse_sql, m)?)?;
    m.add_function(wrap_pyfunction!(emit_postgres_sql, m)?)?;
    Ok(())
}

fn to_py_err(error: queryfabric::QueryFabricError) -> PyErr {
    QueryFabricError::new_err(error.to_string())
}

fn data_type_from_name(name: &str) -> PyResult<DataType> {
    match name.to_ascii_lowercase().as_str() {
        "boolean" | "bool" => Ok(DataType::Boolean),
        "int32" => Ok(DataType::Int32),
        "int64" | "int" | "integer" => Ok(DataType::Int64),
        "float64" | "float" | "double" => Ok(DataType::Float64),
        "utf8" | "string" | "text" => Ok(DataType::Utf8),
        "uuid" => Ok(DataType::Uuid),
        "json" => Ok(DataType::Json),
        "date" => Ok(DataType::Date),
        "unknown" => Ok(DataType::Unknown),
        other => Err(QueryFabricError::new_err(format!(
            "unsupported data type `{other}`"
        ))),
    }
}

fn field_metadata(metadata: Option<HashMap<String, String>>) -> queryfabric::FieldMetadata {
    let mut field = queryfabric::FieldMetadata::default();
    if let Some(metadata) = metadata {
        field.extensions = metadata.into_iter().collect();
    }
    field
}

fn parameter_value_from_py(value: &Bound<'_, PyAny>) -> PyResult<queryfabric::ParameterValue> {
    let json = json_value_from_py(value)?;
    parameter_value_from_json(&json).map_err(to_py_err)
}

fn json_value_from_py(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(boolean) = value.extract::<bool>() {
        return Ok(serde_json::Value::Bool(boolean));
    }
    if let Ok(integer) = value.extract::<i64>() {
        return Ok(serde_json::Value::Number(integer.into()));
    }
    if let Ok(float) = value.extract::<f64>() {
        let number = serde_json::Number::from_f64(float)
            .ok_or_else(|| QueryFabricError::new_err("unsupported non-finite float value"))?;
        return Ok(serde_json::Value::Number(number));
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(serde_json::Value::String(text));
    }
    if let Ok(list) = value.extract::<Vec<Bound<'_, PyAny>>>() {
        return Ok(serde_json::Value::Array(
            list.iter()
                .map(json_value_from_py)
                .collect::<PyResult<Vec<_>>>()?,
        ));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut map = serde_json::Map::with_capacity(dict.len());
        for (key, value) in dict.iter() {
            let key = key
                .extract::<String>()
                .map_err(|_| QueryFabricError::new_err("parameter objects require string keys"))?;
            map.insert(key, json_value_from_py(&value)?);
        }
        return Ok(serde_json::Value::Object(map));
    }
    let type_name = value
        .get_type()
        .name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".to_owned());

    Err(QueryFabricError::new_err(format!(
        "unsupported parameter value type `{type_name}`"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> MemoryCatalog {
        queryfabric::portable_catalog("python-tests")
    }

    #[test]
    fn parsed_summary_reports_syql_shape() {
        let parsed = QueryCompiler::default()
            .parse(
                &SyqlDialect,
                "SELECT record_id FROM records WHERE score > 100 LIMIT 5",
            )
            .expect("parse");
        let summary = inspect_query(&parsed, None);
        assert_eq!(summary.primary_relation.as_deref(), Some("records"));
        assert_eq!(
            summary.projected_columns,
            Some(vec!["record_id".to_owned()])
        );
        assert_eq!(summary.predicate_count, 1);
        assert_eq!(summary.row_limit, Some(5));
    }

    #[test]
    fn bind_emit_and_analyze_portable_query() {
        let compiler = QueryCompiler::default();
        let parsed = compiler
            .parse(&SyqlDialect, "SELECT record_id FROM records LIMIT 3")
            .expect("parse");
        let bound = compiler
            .bind_and_validate(&parsed, &catalog(), &QueryParameters::default())
            .expect("bind");
        let clickhouse = compiler.analyze(&bound, &ClickHouseAdapter, &catalog());
        assert!(clickhouse.supported);
        let postgres = compiler
            .emit(&bound, &PostgresAdapter, &catalog())
            .expect("emit");
        let sql = postgres.as_sql().expect("sql artifact");
        assert_eq!(sql.dialect, "postgres");
        assert!(sql.text.contains("SELECT"));
        assert!(sql.text.contains("records"));
    }

    #[test]
    fn python_parameter_conversion_supports_json_objects() {
        Python::initialize();
        Python::attach(|py| {
            let value =
                serde_json::from_str::<serde_json::Value>(r#"{"species":"mouse"}"#).expect("json");
            let py_value = pythonize(py, &value).expect("pythonize");

            let parameter = parameter_value_from_py(&py_value).expect("parameter");

            assert_eq!(
                parameter,
                queryfabric::ParameterValue::Json(r#"{"species":"mouse"}"#.into())
            );
        });
    }

    #[test]
    fn memory_catalog_document_roundtrip_preserves_relations() {
        let document = catalog().to_document();
        let roundtrip = MemoryCatalog::from_document(document.clone()).to_document();

        assert_eq!(roundtrip.snapshot_id, document.snapshot_id);
        assert_eq!(roundtrip.relations, document.relations);
        assert_eq!(roundtrip.functions, document.functions);
    }
}
