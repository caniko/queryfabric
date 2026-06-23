use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Generic SSR settings built from environment variables.
///
/// All env var names are derived from an application prefix.
/// For example, with `prefix = "SYNDB"`:
/// - `SYNDB_UI_ADDR` → `ui_addr`
/// - `SYNDB_API_URL` → `api_url`
#[derive(Debug, Clone)]
pub struct SsrSettings {
    pub ui_addr: SocketAddr,
    pub api_url: String,
    pub api_prefix: String,
    pub api_timeout: Duration,
    pub site_root: PathBuf,
    pub public_root: PathBuf,
}

/// Defaults used when env vars are not set.
#[derive(Debug, Clone)]
pub struct SsrDefaults {
    pub ui_addr: SocketAddr,
    pub api_url: String,
    pub api_prefix: String,
    pub api_timeout_secs: u64,
    pub site_root: PathBuf,
    pub public_root: PathBuf,
}

impl Default for SsrDefaults {
    fn default() -> Self {
        Self {
            ui_addr: "0.0.0.0:8090".parse().expect("default addr"),
            api_url: "http://localhost:8080".into(),
            api_prefix: "/v1".into(),
            api_timeout_secs: 10,
            site_root: PathBuf::from("target/site"),
            public_root: PathBuf::from("public"),
        }
    }
}

impl SsrSettings {
    /// Build settings from environment variables with the given prefix and defaults.
    ///
    /// Env vars read: `{PREFIX}_UI_ADDR`, `{PREFIX}_API_URL`,
    /// `{PREFIX}_API_PREFIX`, `{PREFIX}_API_TIMEOUT_SECS`,
    /// `{PREFIX}_SITE_ROOT` (or `LEPTOS_SITE_ROOT`).
    /// `CARGO_MANIFEST_DIR` is used for public_root if present.
    pub fn from_env(
        prefix: &str,
        defaults: &SsrDefaults,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let ui_addr = env_var_or(&[&format!("{prefix}_UI_ADDR")], defaults.ui_addr)?;
        let api_url = trim_trailing_slash(
            &env_var_or::<String>(&[&format!("{prefix}_API_URL")], defaults.api_url.clone())
                .unwrap_or_else(|_| defaults.api_url.clone()),
        );
        let api_prefix = normalize_prefix(
            &env_var_or::<String>(
                &[&format!("{prefix}_API_PREFIX")],
                defaults.api_prefix.clone(),
            )
            .unwrap_or_else(|_| defaults.api_prefix.clone()),
        );
        let api_timeout = Duration::from_secs(
            env_var_or::<u64>(
                &[&format!("{prefix}_API_TIMEOUT_SECS")],
                defaults.api_timeout_secs,
            )
            .unwrap_or(defaults.api_timeout_secs),
        );
        let site_root: PathBuf = env_var_or::<String>(
            &[&format!("{prefix}_SITE_ROOT"), "LEPTOS_SITE_ROOT"],
            defaults.site_root.to_string_lossy().as_ref().to_owned(),
        )
        .map(PathBuf::from)
        .unwrap_or_else(|_| defaults.site_root.clone());

        let public_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|d| PathBuf::from(d).join("public"))
            .unwrap_or_else(|_| defaults.public_root.clone());

        Ok(Self {
            ui_addr,
            api_url,
            api_prefix,
            api_timeout,
            site_root,
            public_root,
        })
    }

    pub fn api_endpoint(&self, path: &str) -> String {
        let path = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        if path.starts_with(&self.api_prefix) {
            format!("{}{}", self.api_url, path)
        } else {
            format!("{}{}{}", self.api_url, self.api_prefix, path)
        }
    }

    pub fn api_origin_endpoint(&self, path: &str) -> String {
        let path = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        format!("{}{}", self.api_url, path)
    }

    pub fn static_root(&self) -> PathBuf {
        let built_static = self.site_root.join("static");
        if built_static.is_dir() {
            built_static
        } else {
            self.public_root.join("static")
        }
    }

    pub fn pkg_root(&self) -> PathBuf {
        self.site_root.join("pkg")
    }
}

fn env_var_or<T: std::str::FromStr>(
    names: &[&str],
    default: T,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    for name in names {
        if let Ok(val) = std::env::var(name) {
            return val
                .parse()
                .map_err(|e| format!("failed to parse {name}: {e}").into());
        }
    }
    Ok(default)
}

fn trim_trailing_slash(value: &str) -> String {
    value.trim_end_matches('/').to_owned()
}

fn normalize_prefix(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "/" {
        String::new()
    } else if trimmed.starts_with('/') {
        trimmed.trim_end_matches('/').to_owned()
    } else {
        format!("/{}", trimmed.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_or_uses_default_when_env_not_set() {
        let val: String = env_var_or(&["NONEXISTENT_VAR_12345"], "fallback".into()).unwrap();
        assert_eq!(val, "fallback");
    }

    #[test]
    fn trim_trailing_slash_works() {
        assert_eq!(trim_trailing_slash("http://localhost/"), "http://localhost");
        assert_eq!(trim_trailing_slash("http://localhost"), "http://localhost");
        assert_eq!(trim_trailing_slash("/path/"), "/path");
    }

    #[test]
    fn normalize_prefix_works() {
        assert_eq!(normalize_prefix("/v1/"), "/v1");
        assert_eq!(normalize_prefix("/v1"), "/v1");
        assert_eq!(normalize_prefix("v1"), "/v1");
        assert_eq!(normalize_prefix(""), "");
        assert_eq!(normalize_prefix("/"), "");
    }

    #[test]
    fn api_endpoint_includes_prefix() {
        let settings = SsrSettings {
            ui_addr: "0.0.0.0:8090".parse().unwrap(),
            api_url: "http://localhost:8080".into(),
            api_prefix: "/v1".into(),
            api_timeout: Duration::from_secs(10),
            site_root: PathBuf::from("target/site"),
            public_root: PathBuf::from("public"),
        };
        assert_eq!(
            settings.api_endpoint("/datasets"),
            "http://localhost:8080/v1/datasets"
        );
        assert_eq!(
            settings.api_endpoint("datasets"),
            "http://localhost:8080/v1/datasets"
        );
        assert_eq!(
            settings.api_endpoint("/v1/datasets"),
            "http://localhost:8080/v1/datasets"
        );
    }
}
