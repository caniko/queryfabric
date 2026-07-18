use serde::{Deserialize, Serialize};

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
    #[must_use]
    pub fn class_name(&self) -> &'static str {
        match self.kind {
            FlashKind::Info => "alert-info",
            FlashKind::Success => "alert-success",
            FlashKind::Warning => "alert-warning",
            FlashKind::Error => "alert-danger",
        }
    }
}

#[must_use]
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

#[must_use]
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

#[must_use]
pub fn append_query(path: &str, key: &str, value: &str) -> String {
    let separator = if path.contains('?') { '&' } else { '?' };
    format!("{path}{separator}{key}={}", urlencoding::encode(value))
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticAsset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub content: &'static str,
}

#[cfg(feature = "server")]
const QUERYFABRIC_SYQL_EDITOR_JS: StaticAsset = StaticAsset {
    path: "queryfabric_syql_editor.js",
    content_type: "text/javascript",
    content: include_str!("../assets/queryfabric_syql_editor.js"),
};

#[cfg(feature = "server")]
const STATIC_ASSETS: &[StaticAsset] = &[QUERYFABRIC_SYQL_EDITOR_JS];

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
pub fn static_assets() -> &'static [StaticAsset] {
    STATIC_ASSETS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "server")]
    use queryfabric::{ColumnSchema, DataType, MemoryCatalog, RelationKind, RelationSchema};

    #[cfg(feature = "server")]
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

    #[cfg(feature = "server")]
    #[test]
    fn validate_syql_reports_table_and_predicate_count() {
        let response = validate_syql("FROM records WHERE score > 10 AND score < 20", &catalog());

        assert!(response.valid);
        assert_eq!(response.error, None);
        assert_eq!(response.table.as_deref(), Some("records"));
        assert_eq!(response.predicate_count, 2);
    }

    #[cfg(feature = "server")]
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

    #[cfg(feature = "server")]
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

    #[test]
    fn redirect_helpers_reject_external_and_header_injection_paths() {
        assert_eq!(
            safe_local_redirect(Some("/query/jobs/"), "/"),
            "/query/jobs/"
        );
        assert_eq!(safe_local_redirect(Some("//evil.test"), "/"), "/");
        assert_eq!(safe_local_redirect(Some("https://evil.test"), "/"), "/");
        assert_eq!(safe_local_redirect(Some("/x\r\nLocation:/evil"), "/"), "/");
        assert_eq!(append_query("/path", "key", "val ue"), "/path?key=val%20ue");
        assert_eq!(append_query("/path?a=1", "b", "2"), "/path?a=1&b=2");
    }

    #[test]
    fn flash_and_next_query_helpers_preserve_the_shared_contract() {
        let flash = Flash {
            kind: FlashKind::Warning,
            message: "Disk space low".into(),
        };
        assert_eq!(flash.class_name(), "alert-warning");
        assert_eq!(
            next_query_value("error=Bad&next=/home/").as_deref(),
            Some("/home/")
        );
        assert_eq!(
            next_query_value("next_url=%2Fredirect%3Fa%3D1").as_deref(),
            Some("/redirect?a=1")
        );
        assert_eq!(next_query_value("error=Bad&page=2"), None);
    }
}
