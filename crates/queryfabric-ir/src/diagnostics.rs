use blake3::Hasher;
use serde::{Deserialize, Serialize};

use crate::types::CatalogSnapshotId;

/// Source span used by diagnostics and rewrite lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySourceSpan {
    pub offset: usize,
    pub len: usize,
}

impl QuerySourceSpan {
    pub fn whole(input: &str) -> Self {
        Self {
            offset: 0,
            len: input.len(),
        }
    }

    pub fn end(self) -> usize {
        self.offset + self.len
    }

    pub fn union(self, other: Self) -> Self {
        let start = self.offset.min(other.offset);
        let end = self.end().max(other.end());
        Self {
            offset: start,
            len: end.saturating_sub(start),
        }
    }
}

/// Structured diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

/// Structured compiler diagnostic returned from bind/analyze/emit phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<QuerySourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

impl QueryDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            remediation: None,
            backend: None,
            span: None,
            node_id: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            remediation: None,
            backend: None,
            span: None,
            node_id: None,
        }
    }

    pub fn note(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Note,
            message: message.into(),
            remediation: None,
            backend: None,
            span: None,
            node_id: None,
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    pub fn with_span(mut self, span: QuerySourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }
}

/// Reproducibility envelope attached to analyses and emitted artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceReceipt {
    pub query_hash: String,
    pub compiler_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_snapshot: Option<CatalogSnapshotId>,
    pub dialect: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_identity: Option<String>,
}

impl ProvenanceReceipt {
    pub fn for_query(canonical_sql: &str, dialect: impl Into<String>) -> Self {
        Self {
            query_hash: query_hash(canonical_sql),
            compiler_version: "unknown".into(),
            catalog_snapshot: None,
            dialect: dialect.into(),
            backend: None,
            capability_decision: None,
            artifact_identity: None,
        }
    }

    pub fn with_compiler_version(mut self, compiler_version: impl Into<String>) -> Self {
        self.compiler_version = compiler_version.into();
        self
    }

    pub fn with_catalog_snapshot(mut self, snapshot: CatalogSnapshotId) -> Self {
        self.catalog_snapshot = Some(snapshot);
        self
    }

    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    pub fn with_capability_decision(mut self, decision: impl Into<String>) -> Self {
        self.capability_decision = Some(decision.into());
        self
    }

    pub fn with_artifact_identity(mut self, identity: impl Into<String>) -> Self {
        self.artifact_identity = Some(identity.into());
        self
    }
}

pub fn query_hash(sql: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(sql.as_bytes());
    hasher.finalize().to_hex().to_string()
}
