mod helpers;
mod lower;
mod source_map;

use queryfabric_ir::{Dialect, DialectMetadata, ParsedQuery, QueryFabricError, Result};

pub use source_map::{SourceMap, SourcePiece};

#[derive(Debug, Default, Clone, Copy)]
pub struct GenericSqlDialect;

impl Dialect for GenericSqlDialect {
    fn name(&self) -> &'static str {
        "sql"
    }

    fn parse(&self, input: &str) -> Result<ParsedQuery> {
        parse_sql_query(input)
    }
}

pub fn parse_sql_query(input: &str) -> Result<ParsedQuery> {
    if input.trim().is_empty() {
        return Err(QueryFabricError::Parse {
            message: "empty query".into(),
        });
    }

    let (explain, body, body_offset) = helpers::strip_leading_explain(input);
    let source_map = SourceMap::identity(input, body, body_offset);
    parse_sql_with_source_map(
        "sql",
        input,
        source_map,
        explain,
        DialectMetadata::default(),
    )
}

pub fn parse_sql_with_source_map(
    dialect_name: &str,
    source_sql: &str,
    source_map: SourceMap,
    explain: bool,
    dialect_metadata: DialectMetadata,
) -> Result<ParsedQuery> {
    lower::parse_sql_with_source_map(
        dialect_name,
        source_sql,
        source_map,
        explain,
        dialect_metadata,
    )
}

#[cfg(test)]
mod tests;
