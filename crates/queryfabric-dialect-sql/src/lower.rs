mod expr;
mod query;

use crate::source_map::SourceMap;
use queryfabric_ir::{
    DialectMetadata, ParsedQuery, QueryDiagnostic, QueryFabricError, QuerySourceSpan, Result,
    SyntaxNode,
};
use sqlparser::ast::Spanned;
use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

pub(crate) fn parse_sql_with_source_map(
    dialect_name: &str,
    source_sql: &str,
    source_map: SourceMap,
    explain: bool,
    dialect_metadata: DialectMetadata,
) -> Result<ParsedQuery> {
    if source_map.rewritten_sql().trim().is_empty() {
        return Err(QueryFabricError::Parse {
            message: "empty query body".into(),
        });
    }

    let statements =
        Parser::parse_sql(&GenericDialect {}, source_map.rewritten_sql()).map_err(|error| {
            QueryFabricError::Parse {
                message: error.to_string(),
            }
        })?;
    let statement = statements
        .into_iter()
        .next()
        .ok_or_else(|| QueryFabricError::Parse {
            message: "no statement found".into(),
        })?;
    let Statement::Query(query) = statement else {
        return Err(QueryFabricError::UnsupportedFeature {
            feature: "non-query statement".into(),
            detail: "QueryFabric currently accepts only SELECT/UNION query statements.".into(),
        });
    };

    let mut lowerer = Lowerer::new(&source_map);
    let syntax = lowerer.lower_query(&query, "query");
    let canonical_sql = query.to_string();

    Ok(ParsedQuery::new(dialect_name, source_sql, canonical_sql)
        .with_explain(explain)
        .with_dialect_metadata(dialect_metadata)
        .with_syntax(syntax)
        .with_lowering_diagnostics(lowerer.diagnostics))
}

struct Lowerer<'a> {
    source_map: &'a SourceMap,
    diagnostics: Vec<QueryDiagnostic>,
}

impl<'a> Lowerer<'a> {
    fn new(source_map: &'a SourceMap) -> Self {
        Self {
            source_map,
            diagnostics: Vec::new(),
        }
    }

    fn node<T: Spanned>(&self, value: &T, path: &str) -> SyntaxNode {
        SyntaxNode::new(self.source_map.map_sql_span(value.span()), path)
    }

    fn node_with_span(&self, span: Option<QuerySourceSpan>, path: &str) -> SyntaxNode {
        SyntaxNode::new(span, path)
    }

    fn emit_unsupported<T: Spanned>(&mut self, value: &T, path: &str, description: &str) {
        self.emit_unsupported_span(
            self.source_map.map_sql_span(value.span()),
            path,
            description,
        );
    }

    fn emit_unsupported_span(
        &mut self,
        span: Option<QuerySourceSpan>,
        path: &str,
        description: &str,
    ) {
        let mut diagnostic = QueryDiagnostic::error("QF0000", description.to_owned())
            .with_node_id(path.to_owned())
            .with_remediation("Rewrite the query using the verified portable subset.");
        if let Some(span) = span {
            diagnostic = diagnostic.with_span(span);
        }
        self.diagnostics.push(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_sql_with_source_map;
    use crate::source_map::{SourceMap, SourcePiece};
    use queryfabric_ir::{DialectMetadata, QuerySourceSpan};

    #[test]
    fn lowerer_preserves_basic_query_shape() {
        let map = SourceMap::new(
            "SELECT neuron_id FROM neurons LIMIT 10".into(),
            vec![SourcePiece {
                rewritten_start: 0,
                rewritten_len: "SELECT neuron_id FROM neurons LIMIT 10".len(),
                origin: Some(QuerySourceSpan { offset: 0, len: 39 }),
            }],
        );
        let query = parse_sql_with_source_map(
            "sql",
            "SELECT neuron_id FROM neurons LIMIT 10",
            map,
            false,
            DialectMetadata::default(),
        )
        .expect("parse");
        assert_eq!(
            query.rendered_sql(),
            "SELECT neuron_id FROM neurons LIMIT 10"
        );
        assert!(query.syntax().node.span.is_some());
    }
}
