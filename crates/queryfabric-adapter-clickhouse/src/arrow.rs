use queryfabric_catalog::SqlArtifact;
use queryfabric_ir::{DataType, ResultField, ResultSchema};

/// Build a ClickHouse SQL statement from an artifact, wrapping it for Arrow
/// output safety so JSON and UUID columns are cast to strings.
pub fn clickhouse_arrow_safe_artifact_sql(artifact: &SqlArtifact) -> String {
    clickhouse_arrow_safe_sql(&artifact.text, &artifact.result_schema)
}

/// Rewrite a ClickHouse query so JSON and UUID output columns are cast to
/// strings via `toJSONString` and `toString`.
///
/// ClickHouse's native Arrow output cannot round-trip UUID or JSON types
/// through the binary protocol. Wrapping the query in a subquery with
/// explicit casts avoids the issue.
pub fn clickhouse_arrow_safe_sql(sql: &str, result_schema: &ResultSchema) -> String {
    if !result_schema
        .fields()
        .iter()
        .any(result_field_needs_arrow_cast)
    {
        return sql.to_owned();
    }

    let projections = result_schema
        .fields()
        .iter()
        .map(arrow_safe_projection_expr)
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "SELECT {projections} FROM ({sql}) AS {}",
        clickhouse_quote_identifier("_syndb_arrow_safe"),
    )
}

fn clickhouse_quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn result_field_needs_arrow_cast(field: &ResultField) -> bool {
    matches!(field.data_type, DataType::Uuid | DataType::Json)
}

fn arrow_safe_projection_expr(field: &ResultField) -> String {
    let ident = clickhouse_quote_identifier(&field.name);
    let alias = ident.clone();
    match field.data_type {
        DataType::Uuid => format!("toString({ident}) AS {alias}"),
        DataType::Json => format!("toJSONString({ident}) AS {alias}"),
        _ => format!("{ident} AS {alias}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_wrap_when_no_uuid_or_json_columns() {
        let schema = ResultSchema::new(vec![ResultField::new("x", DataType::Int64, false)]);
        let sql = clickhouse_arrow_safe_sql("SELECT x FROM t", &schema);
        assert_eq!(sql, "SELECT x FROM t");
    }

    #[test]
    fn wraps_uuid_and_json_outputs() {
        let schema = ResultSchema::new(vec![
            ResultField::new("id", DataType::Uuid, false),
            ResultField::new("attrs", DataType::Json, false),
            ResultField::new("name", DataType::Utf8, false),
        ]);
        let sql = clickhouse_arrow_safe_sql("SELECT id, attrs, name FROM t", &schema);
        assert_eq!(
            sql,
            r#"SELECT toString("id") AS "id", toJSONString("attrs") AS "attrs", "name" AS "name" FROM (SELECT id, attrs, name FROM t) AS "_syndb_arrow_safe""#,
            "name is Utf8, not String"
        );
    }
}
