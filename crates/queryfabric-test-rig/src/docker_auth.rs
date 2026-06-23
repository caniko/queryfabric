#![allow(missing_docs)]
use std::path::{Path, PathBuf};

/// Resolve Docker registry credentials.
///
/// Checks (in order):
/// 1. Environment variable `{prefix}_{registry}_PAT` (e.g. `DOCKERHUB_PAT`)
/// 2. Docker credential helpers (`docker-credential-*` in config.json)
/// 3. Docker config.json `auths` entries
pub fn resolve_registry_auth(registry: &str, env_prefix: &str) -> Result<Option<Auth>, String> {
    let pat_var = format!("{env_prefix}_{}_PAT", registry.to_uppercase());
    if let Ok(token) = std::env::var(&pat_var)
        && !token.is_empty()
    {
        return Ok(Some(Auth::Bearer(token)));
    }

    let config_path = get_docker_config_path()?;
    let config_content =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Read config.json: {e}"))?;
    let config: DockerConfig =
        serde_json::from_str(&config_content).map_err(|e| format!("Parse config.json: {e}"))?;

    if let Some(auths) = &config.auths
        && let Some(entry) = auths.get(registry)
        && let Some(auth) = &entry.auth
    {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(auth)
            .map_err(|e| format!("base64 decode auth: {e}"))?;
        let creds = String::from_utf8(decoded).map_err(|e| format!("UTF-8: {e}"))?;
        if let Some((user, pass)) = creds.split_once(':') {
            return Ok(Some(Auth::Basic {
                user: user.to_owned(),
                password: pass.to_owned(),
            }));
        }
    }

    if let Some(creds_helpers) = &config.cred_helpers
        && let Some(helper) = creds_helpers.get(registry)
    {
        return resolve_via_credential_helper(helper, registry);
    }

    if let Some(creds_store) = &config.creds_store {
        return resolve_via_credential_helper(creds_store, registry);
    }

    Ok(None)
}

/// Docker registry authentication.
#[derive(Debug, Clone)]
pub enum Auth {
    /// Username and password authentication.
    Basic { user: String, password: String },
    /// Bearer token authentication (e.g. personal access token).
    Bearer(String),
}

fn get_docker_config_path() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_owned())?;
    let config_dir = std::env::var("DOCKER_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(&home).join(".docker"));
    let config_path = config_dir.join("config.json");
    Ok(config_path.to_string_lossy().to_string())
}

fn resolve_via_credential_helper(helper: &str, registry: &str) -> Result<Option<Auth>, String> {
    let output = std::process::Command::new(format!("docker-credential-{helper}"))
        .arg("get")
        .arg(registry)
        .output()
        .map_err(|e| format!("credential helper {helper}: {e}"))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some((user, pass)) = parse_helper_response(&stdout) {
            return Ok(Some(Auth::Basic {
                user,
                password: pass,
            }));
        }
    }
    Ok(None)
}

fn parse_helper_response(output: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(output).ok()?;
    Some((
        value["Username"].as_str()?.to_owned(),
        value["Secret"].as_str()?.to_owned(),
    ))
}

#[derive(serde::Deserialize)]
struct DockerConfig {
    auths: Option<std::collections::BTreeMap<String, DockerAuthEntry>>,
    creds_store: Option<String>,
    cred_helpers: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(serde::Deserialize)]
struct DockerAuthEntry {
    auth: Option<String>,
}
