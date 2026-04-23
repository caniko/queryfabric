use serde::{Deserialize, Serialize};
use std::fmt;

use crate::diagnostics::{ProvenanceReceipt, QueryDiagnostic, QuerySourceSpan};
use crate::error::Result;
use crate::syntax::{
    BackendClause, BinaryOperator, JoinKind, LiteralValue, NameRef, SyntaxNode, SyntaxQuery,
    UnaryOperator,
};
use crate::types::{
    CapabilityRequirements, CatalogSnapshotId, DataType, DialectMetadata, FunctionRef,
    ParameterBinding, ParameterRef, ResultField, ResultSchema,
};

/// Syntax-level parsed query returned by dialect frontends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedQuery {
    dialect: String,
    source_sql: String,
    canonical_sql: String,
    explain: bool,
    #[serde(default, skip_serializing_if = "DialectMetadata::entries_is_empty")]
    dialect_metadata: DialectMetadata,
    query_span: QuerySourceSpan,
    #[doc(hidden)]
    pub syntax: SyntaxQuery,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lowering_diagnostics: Vec<QueryDiagnostic>,
}

impl ParsedQuery {
    pub fn new(
        dialect: impl Into<String>,
        source_sql: impl Into<String>,
        canonical_sql: impl Into<String>,
    ) -> Self {
        let source_sql = source_sql.into();
        let canonical_sql = canonical_sql.into();
        let query_span = QuerySourceSpan::whole(&source_sql);
        Self {
            dialect: dialect.into(),
            source_sql,
            canonical_sql,
            explain: false,
            dialect_metadata: DialectMetadata::default(),
            query_span,
            syntax: SyntaxQuery::unsupported(
                Some(query_span),
                "syntax.unsupported",
                "query syntax missing",
            ),
            lowering_diagnostics: Vec::new(),
        }
    }

    pub fn dialect(&self) -> &str {
        &self.dialect
    }

    pub fn source_sql(&self) -> &str {
        &self.source_sql
    }

    pub fn canonical_sql(&self) -> &str {
        &self.canonical_sql
    }

    pub fn explain(&self) -> bool {
        self.explain
    }

    pub fn dialect_metadata(&self) -> &DialectMetadata {
        &self.dialect_metadata
    }

    pub fn query_span(&self) -> QuerySourceSpan {
        self.query_span
    }

    pub fn rendered_sql(&self) -> &str {
        &self.canonical_sql
    }

    #[doc(hidden)]
    pub fn syntax(&self) -> &SyntaxQuery {
        &self.syntax
    }

    pub fn lowering_diagnostics(&self) -> &[QueryDiagnostic] {
        &self.lowering_diagnostics
    }

    pub fn with_explain(mut self, explain: bool) -> Self {
        self.explain = explain;
        self
    }

    pub fn with_dialect_metadata(mut self, metadata: DialectMetadata) -> Self {
        self.dialect_metadata = metadata;
        self
    }

    #[doc(hidden)]
    pub fn with_syntax(mut self, syntax: SyntaxQuery) -> Self {
        self.syntax = syntax;
        self
    }

    pub fn with_lowering_diagnostics(mut self, diagnostics: Vec<QueryDiagnostic>) -> Self {
        self.lowering_diagnostics = diagnostics;
        self
    }
}

impl fmt::Display for ParsedQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.canonical_sql.fmt(f)
    }
}

/// Catalog-bound query contract that drives analysis and emission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundQuery {
    parsed: ParsedQuery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_snapshot: Option<CatalogSnapshotId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parameters: Vec<ParameterBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<QueryDiagnostic>,
    #[serde(default, skip_serializing_if = "CapabilityRequirements::is_empty")]
    capability_requirements: CapabilityRequirements,
    #[serde(default, skip_serializing_if = "ResultSchema::is_empty")]
    result_schema: ResultSchema,
    provenance: ProvenanceReceipt,
    #[doc(hidden)]
    pub plan: BoundQueryPlan,
}

impl BoundQuery {
    pub fn new(parsed: ParsedQuery) -> Self {
        let provenance = ProvenanceReceipt::for_query(parsed.canonical_sql(), parsed.dialect());
        let plan = BoundQueryPlan::unsupported(&parsed);
        Self {
            parsed,
            catalog_snapshot: None,
            parameters: Vec::new(),
            diagnostics: Vec::new(),
            capability_requirements: CapabilityRequirements::default(),
            result_schema: ResultSchema::default(),
            provenance,
            plan,
        }
    }

    pub fn parsed(&self) -> &ParsedQuery {
        &self.parsed
    }

    pub fn catalog_snapshot(&self) -> Option<&CatalogSnapshotId> {
        self.catalog_snapshot.as_ref()
    }

    pub fn parameters(&self) -> &[ParameterBinding] {
        &self.parameters
    }

    pub fn diagnostics(&self) -> &[QueryDiagnostic] {
        &self.diagnostics
    }

    pub fn capability_requirements(&self) -> &CapabilityRequirements {
        &self.capability_requirements
    }

    pub fn result_schema(&self) -> &ResultSchema {
        &self.result_schema
    }

    pub fn provenance(&self) -> &ProvenanceReceipt {
        &self.provenance
    }

    #[doc(hidden)]
    pub fn plan(&self) -> &BoundQueryPlan {
        &self.plan
    }

    pub fn with_catalog_snapshot(mut self, snapshot: CatalogSnapshotId) -> Self {
        self.provenance = self
            .provenance
            .clone()
            .with_catalog_snapshot(snapshot.clone());
        self.catalog_snapshot = Some(snapshot);
        self
    }

    pub fn with_parameters(mut self, parameters: Vec<ParameterBinding>) -> Self {
        self.parameters = parameters;
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: Vec<QueryDiagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    pub fn with_capability_requirements(
        mut self,
        capability_requirements: CapabilityRequirements,
    ) -> Self {
        self.capability_requirements = capability_requirements;
        self
    }

    pub fn with_result_schema(mut self, result_schema: ResultSchema) -> Self {
        self.result_schema = result_schema;
        self
    }

    pub fn with_provenance(mut self, provenance: ProvenanceReceipt) -> Self {
        self.provenance = provenance;
        self
    }

    #[doc(hidden)]
    pub fn with_plan(mut self, plan: BoundQueryPlan) -> Self {
        self.result_schema = plan.result_schema.clone();
        self.plan = plan;
        self
    }
}

/// Neutral dialect interface.
pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, input: &str) -> Result<ParsedQuery>;
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundQueryPlan {
    pub node: SyntaxNode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ctes: Vec<BoundCte>,
    pub body: BoundSetExpr,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_by: Vec<BoundOrderByExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<BoundExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<BoundExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backend_clauses: Vec<BackendClause>,
    pub result_schema: ResultSchema,
}

impl BoundQueryPlan {
    pub fn unsupported(parsed: &ParsedQuery) -> Self {
        Self {
            node: SyntaxNode::new(Some(parsed.query_span()), "bound.unsupported"),
            ctes: Vec::new(),
            body: BoundSetExpr::Unsupported {
                description: "query has not been bound".into(),
                node: SyntaxNode::new(Some(parsed.query_span()), "bound.unsupported.body"),
                result_schema: ResultSchema::default(),
            },
            order_by: Vec::new(),
            limit: None,
            offset: None,
            backend_clauses: Vec::new(),
            result_schema: ResultSchema::default(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundCte {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    pub query: Box<BoundQueryPlan>,
    pub result_schema: ResultSchema,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundSetExpr {
    Select(Box<BoundSelect>),
    UnionAll {
        left: Box<BoundSetExpr>,
        right: Box<BoundSetExpr>,
        node: SyntaxNode,
        result_schema: ResultSchema,
    },
    Unsupported {
        description: String,
        node: SyntaxNode,
        result_schema: ResultSchema,
    },
}

impl BoundSetExpr {
    pub fn select(select: BoundSelect) -> Self {
        Self::Select(Box::new(select))
    }

    pub fn as_select(&self) -> Option<&BoundSelect> {
        match self {
            Self::Select(select) => Some(select.as_ref()),
            _ => None,
        }
    }

    pub fn result_schema(&self) -> &ResultSchema {
        match self {
            Self::Select(select) => &select.result_schema,
            Self::UnionAll { result_schema, .. } | Self::Unsupported { result_schema, .. } => {
                result_schema
            }
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundSelect {
    pub distinct: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection: Vec<BoundProjectionItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<BoundTableWithJoins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<BoundExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_by: Vec<BoundExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub having: Option<BoundExpr>,
    pub result_schema: ResultSchema,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundProjectionExpr {
    pub expr: BoundExpr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub field: ResultField,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundProjectionItem {
    Wildcard {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        qualifier: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<ResultField>,
        node: SyntaxNode,
    },
    Expr(Box<BoundProjectionExpr>),
    Unsupported {
        description: String,
        node: SyntaxNode,
    },
}

impl BoundProjectionItem {
    pub fn expr(
        expr: BoundExpr,
        alias: Option<String>,
        field: ResultField,
        node: SyntaxNode,
    ) -> Self {
        Self::Expr(Box::new(BoundProjectionExpr {
            expr,
            alias,
            field,
            node,
        }))
    }

    pub fn as_expr(&self) -> Option<&BoundProjectionExpr> {
        match self {
            Self::Expr(details) => Some(details.as_ref()),
            _ => None,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundTableWithJoins {
    pub relation: BoundRelation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<BoundJoin>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundJoin {
    pub kind: JoinKind,
    pub relation: BoundRelation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<BoundExpr>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundRelationBinding {
    pub binding_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_name: Option<NameRef>,
    pub schema: ResultSchema,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundRelation {
    Table {
        binding: BoundRelationBinding,
        node: SyntaxNode,
    },
    Derived {
        binding: BoundRelationBinding,
        query: Box<BoundQueryPlan>,
        node: SyntaxNode,
    },
    NestedJoin {
        binding: BoundRelationBinding,
        table_with_joins: Box<BoundTableWithJoins>,
        node: SyntaxNode,
    },
    Unsupported {
        description: String,
        binding_name: String,
        node: SyntaxNode,
    },
}

impl BoundRelation {
    pub fn binding(&self) -> Option<&BoundRelationBinding> {
        match self {
            Self::Table { binding, .. }
            | Self::Derived { binding, .. }
            | Self::NestedJoin { binding, .. } => Some(binding),
            Self::Unsupported { .. } => None,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundOrderByExpr {
    pub expr: BoundExpr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asc: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nulls_first: Option<bool>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundColumnRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    pub name: String,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundFunctionCall {
    pub function: FunctionRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_backend_name: Option<FunctionRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<BoundExpr>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub distinct: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Box<BoundExpr>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<BoundWindowSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_signature_name: Option<String>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundWindowSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition_by: Vec<BoundExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_by: Vec<BoundOrderByExpr>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundWhenThen {
    pub condition: BoundExpr,
    pub result: BoundExpr,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundExpr {
    pub kind: BoundExprKind,
    pub data_type: DataType,
    pub nullable: bool,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundExprKind {
    Column(BoundColumnRef),
    Literal(LiteralValue),
    Parameter(ParameterRef),
    Unary {
        op: UnaryOperator,
        expr: Box<BoundExpr>,
    },
    Binary {
        op: BinaryOperator,
        left: Box<BoundExpr>,
        right: Box<BoundExpr>,
    },
    Function(Box<BoundFunctionCall>),
    Case {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        operand: Option<Box<BoundExpr>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        when_then: Vec<BoundWhenThen>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        else_result: Option<Box<BoundExpr>>,
    },
    Cast {
        expr: Box<BoundExpr>,
        data_type: DataType,
    },
    Between {
        expr: Box<BoundExpr>,
        low: Box<BoundExpr>,
        high: Box<BoundExpr>,
        negated: bool,
    },
    InList {
        expr: Box<BoundExpr>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        list: Vec<BoundExpr>,
        negated: bool,
    },
    InSubquery {
        expr: Box<BoundExpr>,
        subquery: Box<BoundQueryPlan>,
        negated: bool,
    },
    ScalarSubquery(Box<BoundQueryPlan>),
    Exists(Box<BoundQueryPlan>),
    Like {
        expr: Box<BoundExpr>,
        pattern: Box<BoundExpr>,
        negated: bool,
        case_insensitive: bool,
    },
    IsNull {
        expr: Box<BoundExpr>,
        negated: bool,
    },
    Tuple(Vec<BoundExpr>),
    Array(Vec<BoundExpr>),
    Unsupported {
        description: String,
    },
}

impl BoundExprKind {
    pub fn function(function: BoundFunctionCall) -> Self {
        Self::Function(Box::new(function))
    }

    pub fn as_function(&self) -> Option<&BoundFunctionCall> {
        match self {
            Self::Function(function) => Some(function.as_ref()),
            _ => None,
        }
    }
}
