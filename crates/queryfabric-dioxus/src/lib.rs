//! Dioxus components for the QueryFabric SyQL editor.
//!
//! The component surface mirrors `queryfabric-leptos` deliberately. Query
//! validation, catalog loading, and editor behavior remain in the existing
//! QueryFabric web contract and JavaScript asset; this crate only renders the
//! host elements and their stable data attributes.

use dioxus::prelude::*;

const DEFAULT_CLASS: &str = "form-control syql-editor";
const DEFAULT_ID: &str = "syql-query";
const DEFAULT_NAME: &str = "query";
const DEFAULT_VALUE: &str = "FROM records LIMIT 10";
const DEFAULT_CATALOG_URL: &str = "/static/queryfabric_catalog.json";
const DEFAULT_VALIDATE_URL: &str = "/_ui/query/syql/validate";

#[component]
pub fn SyqlEditor(
    class: Option<String>,
    id: Option<String>,
    name: Option<String>,
    value: Option<String>,
    rows: Option<u16>,
    catalog_url: Option<String>,
    validate_url: Option<String>,
) -> Element {
    let class = class.unwrap_or_else(|| DEFAULT_CLASS.to_owned());
    let id = id.unwrap_or_else(|| DEFAULT_ID.to_owned());
    let name = name.unwrap_or_else(|| DEFAULT_NAME.to_owned());
    let value = value.unwrap_or_else(|| DEFAULT_VALUE.to_owned());
    let rows = rows.unwrap_or(12);
    let catalog_url = catalog_url.unwrap_or_else(|| DEFAULT_CATALOG_URL.to_owned());
    let validate_url = validate_url.unwrap_or_else(|| DEFAULT_VALIDATE_URL.to_owned());

    rsx! {
        textarea {
            id: id,
            name: name,
            class: class,
            rows: rows,
            spellcheck: "false",
            "data-queryfabric-syql-editor": "true",
            "data-queryfabric-catalog-url": catalog_url,
            "data-queryfabric-validate-url": validate_url,
            "{value}"
        }
    }
}

#[component]
pub fn SyqlEditorScript() -> Element {
    rsx! {
        script {
            r#type: "module",
            src: "/static/queryfabric_syql_editor.js"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syql_editor_preserves_existing_defaults_and_data_contract() {
        let html = dioxus_ssr::render_element(rsx! { SyqlEditor {} });

        assert!(html.contains("class=\"form-control syql-editor\""));
        assert!(html.contains("id=\"syql-query\""));
        assert!(html.contains("name=\"query\""));
        assert!(html.contains("rows=12"));
        assert!(html.contains("spellcheck=\"false\""));
        assert!(html.contains("data-queryfabric-syql-editor=\"true\""));
        assert!(html.contains("data-queryfabric-catalog-url=\"/static/queryfabric_catalog.json\""));
        assert!(html.contains("data-queryfabric-validate-url=\"/_ui/query/syql/validate\""));
        assert!(html.contains(">FROM records LIMIT 10</textarea>"));
    }

    #[test]
    fn syql_editor_accepts_overrides_without_changing_attribute_names() {
        let html = dioxus_ssr::render_element(rsx! {
            SyqlEditor {
                class: "custom-editor".to_owned(),
                id: "custom-id".to_owned(),
                name: "custom-name".to_owned(),
                value: "FROM samples LIMIT 3".to_owned(),
                rows: 8,
                catalog_url: "/catalog.json".to_owned(),
                validate_url: "/validate".to_owned(),
            }
        });

        assert!(html.contains("class=\"custom-editor\""));
        assert!(html.contains("id=\"custom-id\""));
        assert!(html.contains("name=\"custom-name\""));
        assert!(html.contains("rows=8"));
        assert!(html.contains("data-queryfabric-catalog-url=\"/catalog.json\""));
        assert!(html.contains("data-queryfabric-validate-url=\"/validate\""));
        assert!(html.contains(">FROM samples LIMIT 3</textarea>"));
    }

    #[test]
    fn syql_editor_script_preserves_packaged_asset_url() {
        let html = dioxus_ssr::render_element(rsx! { SyqlEditorScript {} });

        assert!(html.contains("type=\"module\""));
        assert!(html.contains("src=\"/static/queryfabric_syql_editor.js\""));
    }
}
