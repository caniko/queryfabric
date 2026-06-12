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
    use super::{static_assets, validate_syql};
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
