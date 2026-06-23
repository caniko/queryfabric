#![allow(missing_docs)]
use std::path::PathBuf;

/// Auth credentials stored in a config directory.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthStore {
    pub token: String,
    pub email: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

/// Return the config directory for a named application.
pub fn config_dir(name: &str) -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(name))
}

/// Load auth token from `~/.config/{app_name}/auth.json`.
pub fn load_auth(app_name: &str) -> Result<AuthStore, String> {
    let path = config_dir(app_name)
        .ok_or_else(|| "Could not determine config directory".to_owned())?
        .join("auth.json");
    let data = std::fs::read_to_string(&path)
        .map_err(|_| format!("Not logged in. Run `{app_name} auth login` first."))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse auth.json: {e}"))
}

/// Load auth token, checking `{APP}_AUTH_TOKEN` env var first, then config file.
pub fn load_auth_token(app_name: &str, env_prefix: &str) -> Result<String, String> {
    let env_var = format!("{env_prefix}_AUTH_TOKEN");
    if let Ok(token) = std::env::var(&env_var)
        && !token.is_empty()
    {
        return Ok(token);
    }
    load_auth(app_name).map(|a| a.token)
}

/// Save auth credentials to `~/.config/{app_name}/auth.json`.
pub fn save_auth(app_name: &str, store: &AuthStore) -> Result<(), String> {
    let dir = config_dir(app_name).ok_or("Could not determine config directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let data = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize auth: {e}"))?;
    std::fs::write(dir.join("auth.json"), data)
        .map_err(|e| format!("Failed to write auth.json: {e}"))
}

/// Remove auth credentials.
pub fn remove_auth(app_name: &str) -> Result<(), String> {
    let path = config_dir(app_name)
        .ok_or("Could not determine config directory")?
        .join("auth.json");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to remove auth.json: {e}"))
    } else {
        Ok(())
    }
}
