use super::{GenericSqlDialect, SourceMap, parse_sql_query};
use queryfabric_ir::Dialect;

#[test]
fn parses_plain_select_and_lowers_syntax() {
    let dialect = GenericSqlDialect;
    let query = dialect
        .parse("SELECT record_id FROM records LIMIT 10")
        .expect("parse");
    assert_eq!(
        query.rendered_sql(),
        "SELECT record_id FROM records LIMIT 10"
    );
    assert!(query.syntax().node.span.is_some());
}

#[test]
fn preserves_explain_flag() {
    let query = parse_sql_query("EXPLAIN SELECT * FROM records").expect("parse");
    assert!(query.explain());
    assert_eq!(query.rendered_sql(), "SELECT * FROM records");
}

#[test]
fn source_map_identity_maps_spans() {
    let map = SourceMap::identity("SELECT 1", "SELECT 1", 0);
    assert_eq!(map.rewritten_sql(), "SELECT 1");
}
