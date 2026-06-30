#![allow(clippy::extra_unused_lifetimes)]

use leptos::prelude::*;

#[component]
pub fn SyqlEditor(
    #[prop(optional, into)] class: Option<String>,
    #[prop(optional, into)] id: Option<String>,
    #[prop(optional, into)] name: Option<String>,
    #[prop(optional, into)] value: Option<String>,
    #[prop(optional, into)] rows: Option<u16>,
    #[prop(optional, into)] catalog_url: Option<String>,
    #[prop(optional, into)] validate_url: Option<String>,
) -> impl IntoView {
    let class = class.unwrap_or_else(|| "form-control syql-editor".to_owned());
    let id = id.unwrap_or_else(|| "syql-query".to_owned());
    let name = name.unwrap_or_else(|| "query".to_owned());
    let value = value.unwrap_or_else(|| "FROM records LIMIT 10".to_owned());
    let rows = rows.unwrap_or(12);
    let catalog_url = catalog_url.unwrap_or_else(|| "/static/queryfabric_catalog.json".to_owned());
    let validate_url = validate_url.unwrap_or_else(|| "/_ui/query/syql/validate".to_owned());

    view! {
        <textarea
            id=id
            name=name
            class=class
            rows=rows
            spellcheck="false"
            data-queryfabric-syql-editor="true"
            data-queryfabric-catalog-url=catalog_url
            data-queryfabric-validate-url=validate_url
        >
            {value}
        </textarea>
    }
}

#[component]
pub fn SyqlEditorScript() -> impl IntoView {
    view! {
        <script
            type="module"
            src="/static/queryfabric_syql_editor.js"
        ></script>
    }
}

#[cfg(test)]
mod tests {
    use super::{SyqlEditor, SyqlEditorScript};
    use leptos::prelude::*;

    #[test]
    fn syql_editor_renders_expected_defaults() {
        let html = view! {
            <SyqlEditor class="form-control syql-editor"/>
        }
        .to_html();

        assert!(html.contains("class=\"form-control syql-editor\""));
        assert!(html.contains("id=\"syql-query\""));
        assert!(html.contains("name=\"query\""));
        assert!(html.contains("data-queryfabric-syql-editor"));
        assert!(html.contains("data-queryfabric-catalog-url=\"/static/queryfabric_catalog.json\""));
        assert!(html.contains("data-queryfabric-validate-url=\"/_ui/query/syql/validate\""));
        assert!(html.contains(">FROM records LIMIT 10<"));
    }

    #[test]
    fn syql_editor_script_renders_packaged_asset() {
        let html = view! {
            <SyqlEditorScript/>
        }
        .to_html();

        assert!(html.contains("type=\"module\""));
        assert!(html.contains("src=\"/static/queryfabric_syql_editor.js\""));
    }
}
