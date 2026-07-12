//! Stable public query contracts for QueryFabric.
//!
//! Public consumers interact with syntax-neutral parsed/bound query stages,
//! typed schemas, structured diagnostics, and provenance receipts. The neutral
//! syntax AST and bound semantic IR are carried internally so downstream
//! dialects, binders, analyzers, and emitters no longer need to reparse SQL
//! strings.

mod bound;
mod budget;
mod diagnostics;
mod error;
mod syntax;
mod types;

pub use bound::{
    BoundColumnRef, BoundCte, BoundExpr, BoundExprKind, BoundFunctionCall, BoundJoin,
    BoundOrderByExpr, BoundProjectionExpr, BoundProjectionItem, BoundQuery, BoundQueryPlan,
    BoundRelation, BoundRelationBinding, BoundSelect, BoundSetExpr, BoundTableWithJoins,
    BoundWhenThen, BoundWindowSpec, Dialect, ParsedQuery,
};
pub use budget::{QueryBudget, QueryBudgetDimension, QueryBudgetUsage};
pub use diagnostics::{
    DiagnosticSeverity, ProvenanceReceipt, QueryDiagnostic, QuerySourceSpan, query_hash,
};
pub use error::{BindErrorDetails, QueryFabricError, Result};
pub use syntax::{
    BackendClause, BinaryOperator, JoinKind, LiteralValue, NameRef, SyntaxNode, UnaryOperator,
};
pub use syntax::{
    SyntaxCte, SyntaxExpr, SyntaxExprKind, SyntaxFunctionCall, SyntaxJoin, SyntaxOrderByExpr,
    SyntaxProjectionExpr, SyntaxProjectionItem, SyntaxQuery, SyntaxRelation, SyntaxSelect,
    SyntaxSetExpr, SyntaxTableWithJoins, SyntaxWhenThen, WindowSpec,
};
pub use types::{
    CapabilityRequirement, CapabilityRequirements, CatalogSnapshotId, DataType, DialectMetadata,
    FieldMetadata, FunctionRef, ParameterBinding, ParameterRef, ParameterSchema, ParameterSummary,
    ParameterValue, QueryParameters, ResultField, ResultSchema,
};

#[cfg(test)]
mod tests {
    use super::{
        BoundQuery, DataType, ParameterRef, ParameterValue, ParsedQuery, QueryParameters,
        QuerySourceSpan, SyntaxQuery,
    };

    #[test]
    fn parsed_query_keeps_source_and_canonical_sql() {
        let parsed = ParsedQuery::new("sql", " select 1 ", "SELECT 1").with_syntax(
            SyntaxQuery::unsupported(
                Some(QuerySourceSpan::whole(" select 1 ")),
                "query",
                "manual test",
            ),
        );
        assert_eq!(parsed.dialect(), "sql");
        assert_eq!(parsed.source_sql(), " select 1 ");
        assert_eq!(parsed.canonical_sql(), "SELECT 1");
    }

    #[test]
    fn provenance_defaults_to_unknown_until_facade_sets_version() {
        let bound = BoundQuery::new(ParsedQuery::new("sql", "SELECT 1", "SELECT 1").with_syntax(
            SyntaxQuery::unsupported(
                Some(QuerySourceSpan::whole("SELECT 1")),
                "query",
                "manual test",
            ),
        ));
        assert_eq!(bound.provenance().dialect, "sql");
        assert_eq!(bound.provenance().compiler_version, "unknown");
        assert_eq!(bound.provenance().query_hash.len(), 64);
    }

    #[test]
    fn parameter_value_infers_list_type() {
        let value = ParameterValue::List(vec![ParameterValue::Int64(1)]);
        assert_eq!(
            value.inferred_type(),
            DataType::List(Box::new(DataType::Int64))
        );
    }

    #[test]
    fn query_parameters_lookup_supports_both_kinds() {
        let mut params = QueryParameters::default();
        params.insert_positional(1, ParameterValue::Int64(42));
        params.insert_named("species", ParameterValue::Utf8("mouse".into()));
        assert_eq!(
            params.lookup(&ParameterRef::Positional(1)),
            Some(&ParameterValue::Int64(42))
        );
        assert_eq!(
            params.lookup(&ParameterRef::Named("species".into())),
            Some(&ParameterValue::Utf8("mouse".into()))
        );
    }
}
