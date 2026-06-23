use std::collections::HashMap;

/// Resolve a crates.io crate name to its GitHub repository URL.
pub async fn resolve_crate_repo(crate_name: &str) -> Result<Option<String>, String> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}");
    let client = reqwest::Client::builder()
        .user_agent("queryfabric-changelog/0.1")
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("crates.io API: {e}"))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("crates.io JSON: {e}"))?;

    if let Some(repo) = body["crate"]["repository"].as_str()
        && repo.contains("github.com")
    {
        return Ok(Some(repo.to_owned()));
    }
    Ok(None)
}

/// Resolve a PyPI package name to its GitHub repository URL.
pub async fn resolve_pypi_repo(package_name: &str) -> Result<Option<String>, String> {
    let url = format!("https://pypi.org/pypi/{package_name}/json");
    let client = reqwest::Client::builder()
        .user_agent("queryfabric-changelog/0.1")
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("PyPI API: {e}"))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("PyPI JSON: {e}"))?;

    // Check project_urls and home_page for GitHub links.
    if let Some(urls) = body["info"]["project_urls"].as_object() {
        for (_key, value) in urls {
            if let Some(url) = value.as_str()
                && url.contains("github.com")
            {
                return Ok(Some(url.to_owned()));
            }
        }
    }

    if let Some(home) = body["info"]["home_page"].as_str()
        && home.contains("github.com")
    {
        return Ok(Some(home.to_owned()));
    }

    Ok(None)
}

/// Static image-to-repository mapping for changelog resolution.
/// Key is the image name as it appears in the git diff; value is the
/// GitHub repository URL.
pub fn image_repo_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("postgres", "https://github.com/docker-library/postgres");
    m.insert("clickhouse", "https://github.com/ClickHouse/ClickHouse");
    m.insert("minio", "https://github.com/minio/minio");
    m
}
