use serde::{Deserialize, Serialize};

use crate::bound::ParsedQuery;
use crate::diagnostics::{ProvenanceReceipt, QueryDiagnostic};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindErrorDetails {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_sql: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialect: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<QueryDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProvenanceReceipt>,
}

impl BindErrorDetails {
    pub fn with_compiler_version(mut self, compiler_version: &str) -> Self {
        self.provenance = self
            .provenance
            .map(|receipt| receipt.with_compiler_version(compiler_version));
        self
    }
}

/// Errors produced by QueryFabric dialects, binders, analyzers, and adapters.
#[derive(Debug, thiserror::Error, miette::Diagnostic, Serialize, Deserialize)]
pub enum QueryFabricError {
    #[error("Parse error: {message}")]
    #[diagnostic(help("Check the query syntax for the active dialect."))]
    Parse { message: String },

    #[error("Bind error: {}", .0.message)]
    #[diagnostic(help("Review the attached diagnostics for precise remediation."))]
    Bind(Box<BindErrorDetails>),

    #[error("Unsupported feature: {feature}")]
    #[diagnostic(help("{detail}"))]
    UnsupportedFeature { feature: String, detail: String },

    #[error("Catalog error: {0}")]
    Catalog(String),

    #[error("Emission failed: {0}")]
    Emission(String),
}

impl QueryFabricError {
    pub fn bind(
        message: impl Into<String>,
        query: Option<&ParsedQuery>,
        diagnostics: Vec<QueryDiagnostic>,
        provenance: Option<ProvenanceReceipt>,
    ) -> Self {
        Self::Bind(Box::new(BindErrorDetails {
            message: message.into(),
            source_sql: query.map(|parsed| parsed.source_sql().to_owned()),
            dialect: query.map(|parsed| parsed.dialect().to_owned()),
            diagnostics,
            provenance,
        }))
    }

    pub fn as_bind(&self) -> Option<&BindErrorDetails> {
        match self {
            Self::Bind(details) => Some(details.as_ref()),
            _ => None,
        }
    }

    pub fn with_compiler_version(self, compiler_version: &str) -> Self {
        match self {
            Self::Bind(details) => {
                Self::Bind(Box::new(details.with_compiler_version(compiler_version)))
            }
            other => other,
        }
    }
}

pub type Result<T> = std::result::Result<T, QueryFabricError>;
