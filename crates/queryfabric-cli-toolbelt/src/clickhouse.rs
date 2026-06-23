#![allow(missing_docs)]
/// ClickHouse connection parameters.
#[derive(Debug, Clone)]
pub struct ClickHouseConnArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

impl Default for ClickHouseConnArgs {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 8123,
            user: "default".into(),
            password: String::new(),
        }
    }
}

impl ClickHouseConnArgs {
    pub fn into_url(self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Environment variable names for ClickHouse connection.
pub const ENV_HOST: &str = "CLICKHOUSE_HOST";
pub const ENV_PORT: &str = "CLICKHOUSE_PORT";
pub const ENV_USER: &str = "CLICKHOUSE_USER";
pub const ENV_PASSWORD: &str = "CLICKHOUSE_PASSWORD";

impl ClickHouseConnArgs {
    /// Read connection args from environment variables.
    pub fn from_env() -> Self {
        Self {
            host: std::env::var(ENV_HOST).unwrap_or_else(|_| "localhost".to_owned()),
            port: std::env::var(ENV_PORT)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8123),
            user: std::env::var(ENV_USER).unwrap_or_else(|_| "default".to_owned()),
            password: std::env::var(ENV_PASSWORD).unwrap_or_default(),
        }
    }
}
