#[cfg(feature = "ssr")]
pub mod ssr;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Flash / alert types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flash {
    pub kind: FlashKind,
    pub message: String,
}

impl Flash {
    pub fn class_name(&self) -> &'static str {
        match self.kind {
            FlashKind::Info => "alert-info",
            FlashKind::Success => "alert-success",
            FlashKind::Warning => "alert-warning",
            FlashKind::Error => "alert-danger",
        }
    }
}

// ---------------------------------------------------------------------------
// URL utility functions
// ---------------------------------------------------------------------------

/// Extract a `next` or `next_url` query parameter value from a query string.
pub fn next_query_value(query: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(key, value)| {
            if key == "next" || key == "next_url" {
                Some(
                    urlencoding::decode(value)
                        .map(|value| value.into_owned())
                        .unwrap_or_else(|_| value.to_owned()),
                )
            } else {
                None
            }
        })
}

/// Redirect-safety check: only allow local (relative) paths.
/// Returns the candidate path if safe, otherwise the fallback.
pub fn safe_local_redirect(candidate: Option<&str>, fallback: &str) -> String {
    let Some(candidate) = candidate else {
        return fallback.to_owned();
    };
    let candidate = candidate.trim();
    if candidate.is_empty()
        || !candidate.starts_with('/')
        || candidate.starts_with("//")
        || candidate.contains('\\')
        || candidate.contains('\n')
        || candidate.contains('\r')
    {
        return fallback.to_owned();
    }
    candidate.to_owned()
}

/// Append a query parameter to a path, preserving any existing query string.
pub fn append_query(path: &str, key: &str, value: &str) -> String {
    let separator = if path.contains('?') { '&' } else { '?' };
    format!("{path}{separator}{key}={}", urlencoding::encode(value))
}

// ---------------------------------------------------------------------------
// SyQL validation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyqlValidateRequest {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyqlValidateResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    pub predicate_count: usize,
}

// ---------------------------------------------------------------------------
// Static assets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticAsset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub content: &'static str,
}

const QUERYFABRIC_SYQL_EDITOR_JS: StaticAsset = StaticAsset {
    path: "queryfabric_syql_editor.js",
    content_type: "text/javascript",
    content: include_str!("../assets/queryfabric_syql_editor.js"),
};

const STATIC_ASSETS: &[StaticAsset] = &[QUERYFABRIC_SYQL_EDITOR_JS];

pub fn validate_syql(
    query: &str,
    catalog: &dyn queryfabric_catalog::Catalog,
) -> SyqlValidateResponse {
    let parsed = match queryfabric::parse_syql(query) {
        Ok(parsed) => parsed,
        Err(error) => {
            return SyqlValidateResponse {
                valid: false,
                error: Some(error.to_string()),
                table: None,
                predicate_count: 0,
            };
        }
    };

    let summary = queryfabric::inspect_query(&parsed, None);
    if let Err(error) =
        queryfabric::bind_and_validate(&parsed, catalog, &queryfabric::QueryParameters::default())
    {
        return SyqlValidateResponse {
            valid: false,
            error: Some(error.to_string()),
            table: summary.primary_relation,
            predicate_count: summary.predicate_count,
        };
    }

    SyqlValidateResponse {
        valid: true,
        error: None,
        table: summary.primary_relation,
        predicate_count: summary.predicate_count,
    }
}

pub fn static_assets() -> &'static [StaticAsset] {
    STATIC_ASSETS
}

#[cfg(test)]
mod tests {
    use super::*;
    use queryfabric::{ColumnSchema, DataType, MemoryCatalog, RelationKind, RelationSchema};

    fn catalog() -> MemoryCatalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "records".into(),
            aliases: vec!["r".into()],
            kind: RelationKind::Table,
            columns: vec![
                ColumnSchema {
                    name: "record_id".into(),
                    data_type: DataType::Uuid,
                    nullable: false,
                    metadata: Default::default(),
                },
                ColumnSchema {
                    name: "score".into(),
                    data_type: DataType::Float64,
                    nullable: true,
                    metadata: Default::default(),
                },
            ],
            metadata: Default::default(),
        });
        catalog
    }

    #[test]
    fn validate_syql_reports_table_and_predicate_count() {
        let response = validate_syql("FROM records WHERE score > 10 AND score < 20", &catalog());

        assert!(response.valid);
        assert_eq!(response.error, None);
        assert_eq!(response.table.as_deref(), Some("records"));
        assert_eq!(response.predicate_count, 2);
    }

    #[test]
    fn validate_syql_preserves_parse_summary_on_binding_error() {
        let response = validate_syql("FROM missing_table LIMIT 1", &MemoryCatalog::default());

        assert!(!response.valid);
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|error| error.contains("missing_table") || error.contains("relation"))
        );
        assert_eq!(response.table.as_deref(), Some("missing_table"));
        assert_eq!(response.predicate_count, 0);
    }

    // -- Flash / FlashKind tests --

    #[test]
    fn flash_class_name_maps_variants() {
        assert_eq!(
            Flash {
                kind: FlashKind::Info,
                message: "x".into()
            }
            .class_name(),
            "alert-info"
        );
        assert_eq!(
            Flash {
                kind: FlashKind::Success,
                message: "x".into()
            }
            .class_name(),
            "alert-success"
        );
        assert_eq!(
            Flash {
                kind: FlashKind::Warning,
                message: "x".into()
            }
            .class_name(),
            "alert-warning"
        );
        assert_eq!(
            Flash {
                kind: FlashKind::Error,
                message: "x".into()
            }
            .class_name(),
            "alert-danger"
        );
    }

    #[test]
    fn flash_round_trips_through_serde() {
        let flash = Flash {
            kind: FlashKind::Warning,
            message: "Disk space low".into(),
        };
        let json = serde_json::to_string(&flash).unwrap();
        let deserialized: Flash = serde_json::from_str(&json).unwrap();
        assert_eq!(flash, deserialized);
    }

    // -- URL utility tests --

    #[test]
    fn next_query_value_extracts_next_param() {
        assert_eq!(
            next_query_value("next=/account/profile/").as_deref(),
            Some("/account/profile/")
        );
        assert_eq!(
            next_query_value("error=Bad&next=/home/").as_deref(),
            Some("/home/")
        );
        assert_eq!(
            next_query_value("next_url=%2Fredirect%3Fa%3D1").as_deref(),
            Some("/redirect?a=1")
        );
    }

    #[test]
    fn next_query_value_returns_none_when_missing() {
        assert_eq!(next_query_value("error=Bad&page=2"), None);
        assert_eq!(next_query_value(""), None);
    }

    #[test]
    fn safe_redirect_accepts_local_paths() {
        assert_eq!(
            safe_local_redirect(Some("/query/jobs/"), "/"),
            "/query/jobs/"
        );
        assert_eq!(safe_local_redirect(Some("/"), "/fallback"), "/");
    }

    #[test]
    fn safe_redirect_rejects_external_urls() {
        assert_eq!(safe_local_redirect(Some("//evil.test"), "/"), "/");
        assert_eq!(safe_local_redirect(Some("https://evil.test"), "/"), "/");
        assert_eq!(safe_local_redirect(Some("/x\r\nLocation:/evil"), "/"), "/");
        assert_eq!(safe_local_redirect(Some(""), "/"), "/");
        assert_eq!(safe_local_redirect(None, "/"), "/");
    }

    #[test]
    fn append_query_appends_with_correct_separator() {
        let url = append_query("/path", "key", "val ue");
        assert_eq!(url, "/path?key=val%20ue");
    }

    #[test]
    fn append_query_uses_ampersand_when_query_exists() {
        let url = append_query("/path?a=1", "b", "2");
        assert_eq!(url, "/path?a=1&b=2");
    }

    #[test]
    fn static_asset_is_packaged() {
        let assets = static_assets();
        assert_eq!(assets.len(), 1);
        let asset = &assets[0];
        assert_eq!(asset.path, "queryfabric_syql_editor.js");
        assert_eq!(asset.content_type, "text/javascript");

        let content = asset.content;
        assert!(content.contains("data-queryfabric-syql-editor"));
        assert!(content.contains("/_ui/query/syql/validate"));
        assert!(content.contains("/static/queryfabric_catalog.json"));
        assert!(!content.contains("location.pathname.includes(\"/query/syql\")"));
    }
}
