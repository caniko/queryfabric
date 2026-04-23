use serde::{Deserialize, Serialize};

use crate::diagnostics::QuerySourceSpan;
use crate::types::{DataType, FunctionRef, ParameterRef};

/// Parser-facing syntax node with stable span and identity metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<QuerySourceSpan>,
    pub node_id: String,
}

impl SyntaxNode {
    pub fn new(span: Option<QuerySourceSpan>, node_id: impl Into<String>) -> Self {
        Self {
            span,
            node_id: node_id.into(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
}

impl NameRef {
    pub fn display_name(&self) -> String {
        match &self.namespace {
            Some(namespace) => format!("{namespace}.{}", self.name),
            None => self.name.clone(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendClause {
    ClickHouseSettings { text: String, node: SyntaxNode },
    ClickHouseFormat { text: String, node: SyntaxNode },
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxQuery {
    pub node: SyntaxNode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ctes: Vec<SyntaxCte>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub with_recursive: bool,
    pub body: SyntaxSetExpr,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_by: Vec<SyntaxOrderByExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<SyntaxExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<SyntaxExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backend_clauses: Vec<BackendClause>,
}

impl SyntaxQuery {
    pub fn unsupported(
        span: Option<QuerySourceSpan>,
        node_id: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            node: SyntaxNode::new(span, node_id),
            ctes: Vec::new(),
            with_recursive: false,
            body: SyntaxSetExpr::Unsupported {
                description: description.into(),
                node: SyntaxNode::new(span, "unsupported"),
            },
            order_by: Vec::new(),
            limit: None,
            offset: None,
            backend_clauses: Vec::new(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxCte {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    pub query: Box<SyntaxQuery>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntaxSetExpr {
    Select(Box<SyntaxSelect>),
    UnionAll {
        left: Box<SyntaxSetExpr>,
        right: Box<SyntaxSetExpr>,
        node: SyntaxNode,
    },
    Unsupported {
        description: String,
        node: SyntaxNode,
    },
}

impl SyntaxSetExpr {
    pub fn select(select: SyntaxSelect) -> Self {
        Self::Select(Box::new(select))
    }

    pub fn as_select(&self) -> Option<&SyntaxSelect> {
        match self {
            Self::Select(select) => Some(select.as_ref()),
            _ => None,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxSelect {
    pub distinct: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection: Vec<SyntaxProjectionItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<SyntaxTableWithJoins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SyntaxExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_by: Vec<SyntaxExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub having: Option<SyntaxExpr>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxProjectionExpr {
    pub expr: SyntaxExpr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntaxProjectionItem {
    Wildcard {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        qualifier: Option<String>,
        node: SyntaxNode,
    },
    Expr(Box<SyntaxProjectionExpr>),
    Unsupported {
        description: String,
        node: SyntaxNode,
    },
}

impl SyntaxProjectionItem {
    pub fn expr(expr: SyntaxExpr, alias: Option<String>, node: SyntaxNode) -> Self {
        Self::Expr(Box::new(SyntaxProjectionExpr { expr, alias, node }))
    }

    pub fn as_expr(&self) -> Option<&SyntaxProjectionExpr> {
        match self {
            Self::Expr(details) => Some(details.as_ref()),
            _ => None,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxTableWithJoins {
    pub relation: SyntaxRelation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<SyntaxJoin>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxJoin {
    pub kind: JoinKind,
    pub relation: SyntaxRelation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<SyntaxExpr>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntaxRelation {
    Table {
        name: NameRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
        node: SyntaxNode,
    },
    Derived {
        query: Box<SyntaxQuery>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
        node: SyntaxNode,
    },
    NestedJoin {
        table_with_joins: Box<SyntaxTableWithJoins>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
        node: SyntaxNode,
    },
    Unsupported {
        description: String,
        node: SyntaxNode,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxOrderByExpr {
    pub expr: SyntaxExpr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asc: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nulls_first: Option<bool>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralValue {
    Null,
    Boolean(bool),
    Int64(i64),
    Float64(String),
    Utf8(String),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition_by: Vec<SyntaxExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_by: Vec<SyntaxOrderByExpr>,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxFunctionCall {
    pub function: FunctionRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<SyntaxExpr>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub distinct: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Box<SyntaxExpr>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<WindowSpec>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxWhenThen {
    pub condition: SyntaxExpr,
    pub result: SyntaxExpr,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxExpr {
    pub kind: SyntaxExprKind,
    pub node: SyntaxNode,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntaxExprKind {
    Column {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        relation: Option<String>,
        name: String,
    },
    Literal(LiteralValue),
    Parameter(ParameterRef),
    Unary {
        op: UnaryOperator,
        expr: Box<SyntaxExpr>,
    },
    Binary {
        op: BinaryOperator,
        left: Box<SyntaxExpr>,
        right: Box<SyntaxExpr>,
    },
    Function(SyntaxFunctionCall),
    Case {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        operand: Option<Box<SyntaxExpr>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        when_then: Vec<SyntaxWhenThen>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        else_result: Option<Box<SyntaxExpr>>,
    },
    Cast {
        expr: Box<SyntaxExpr>,
        data_type: DataType,
    },
    Between {
        expr: Box<SyntaxExpr>,
        low: Box<SyntaxExpr>,
        high: Box<SyntaxExpr>,
        negated: bool,
    },
    InList {
        expr: Box<SyntaxExpr>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        list: Vec<SyntaxExpr>,
        negated: bool,
    },
    InSubquery {
        expr: Box<SyntaxExpr>,
        subquery: Box<SyntaxQuery>,
        negated: bool,
    },
    ScalarSubquery(Box<SyntaxQuery>),
    Exists(Box<SyntaxQuery>),
    Like {
        expr: Box<SyntaxExpr>,
        pattern: Box<SyntaxExpr>,
        negated: bool,
        case_insensitive: bool,
    },
    IsNull {
        expr: Box<SyntaxExpr>,
        negated: bool,
    },
    Tuple(Vec<SyntaxExpr>),
    Array(Vec<SyntaxExpr>),
    Unsupported {
        description: String,
    },
}
