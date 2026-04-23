use queryfabric::{
    ClickHouseAdapter, GenericSqlDialect, QueryCompiler, QueryParameters, bind_and_validate_query,
};

fn main() -> Result<(), queryfabric::QueryFabricError> {
    let compiler = QueryCompiler::default();
    let parsed = compiler.parse(
        &GenericSqlDialect,
        "SELECT neuron_id, cable_length FROM neurons WHERE cable_length > $1 LIMIT 5",
    )?;

    let catalog = queryfabric::portable_catalog("quickstart-catalog");

    let mut parameters = QueryParameters::default();
    parameters.insert_positional(1, queryfabric::ParameterValue::Float64("100.0".into()));

    let bound = bind_and_validate_query(&parsed, &catalog, &parameters)?;
    let analysis = compiler.analyze(&bound, &ClickHouseAdapter, &catalog);
    let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
    let sql = artifact.as_sql().expect("SQL artifact");

    println!("supported: {}", analysis.supported);
    println!("query hash: {}", sql.provenance.query_hash);
    println!("sql:\n{}", sql.text);

    Ok(())
}
