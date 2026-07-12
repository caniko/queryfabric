//! Environment-driven configuration.
//!
//! Every knob is a `QFDEMO_*` environment variable so the NixOS module can
//! wire options straight through systemd `Environment=` lines. Secrets are
//! never accepted inline: the database URL may arrive via
//! `QFDEMO_DATABASE_URL_FILE` and S3 credentials only via
//! `QFDEMO_STORE_CREDENTIALS_FILE`, both pointing at files provided by
//! `LoadCredential`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// Configuration errors: each names the offending variable and what to do.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{name} is required: set it (or {name}_FILE for secrets)")]
    Missing { name: &'static str },
    #[error("{name} has invalid value '{value}': {expected}")]
    Invalid {
        name: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("failed to read {name} from '{path}': {source}")]
    Unreadable {
        name: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("credentials file '{path}' is missing key {key} (expected KEY=VALUE lines)")]
    MissingCredential { path: PathBuf, key: &'static str },
}

/// Object-store selection mirroring the NixOS module's `store.backend`.
#[derive(Debug, Clone)]
pub enum StoreConfig {
    /// In-process, non-durable store (development and smoke tests).
    Memory,
    /// Any S3-compatible backend: AWS S3, MinIO, Garage.
    S3 {
        endpoint: String,
        bucket: String,
        region: String,
        credentials_file: PathBuf,
    },
}

impl StoreConfig {
    /// Stable label reported in API responses and logs.
    #[must_use]
    pub fn backend_label(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::S3 { .. } => "s3",
        }
    }
}

/// Optional federation-node identity announced by the demonstrator.
#[derive(Debug, Clone)]
pub struct FederationConfig {
    pub enable: bool,
    pub node_name: String,
    pub hub_multiaddrs: Vec<String>,
    pub flight_port: u16,
}

/// Parsed S3 credentials.
#[derive(Clone)]
pub struct S3Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl std::fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Credentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"<redacted>")
            .finish()
    }
}

/// Full demonstrator configuration.
#[derive(Clone)]
pub struct DemoConfig {
    pub listen_addr: SocketAddr,
    pub database_migration_url: String,
    pub database_query_url: String,
    pub database_import_url: String,
    /// Secret used to validate host-issued PASETO bearer credentials.
    pub auth_secret: String,
    pub db_wait_secs: u64,
    pub public_base_url: String,
    pub seed_demo_data: bool,
    pub store: StoreConfig,
    pub federation: FederationConfig,
}

impl std::fmt::Debug for DemoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DemoConfig")
            .field("listen_addr", &self.listen_addr)
            .field("database_migration_url", &"<redacted>")
            .field("database_query_url", &"<redacted>")
            .field("database_import_url", &"<redacted>")
            .field("auth_secret", &"<redacted>")
            .field("db_wait_secs", &self.db_wait_secs)
            .field("public_base_url", &self.public_base_url)
            .field("seed_demo_data", &self.seed_demo_data)
            .field("store", &self.store)
            .field("federation", &self.federation)
            .finish()
    }
}

fn env_var(name: &'static str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// A value that may arrive inline (`NAME`) or via a secret file
/// (`NAME_FILE`). The file wins so credential mounts override defaults.
fn env_or_file(name: &'static str, file_name: &'static str) -> Result<Option<String>, ConfigError> {
    if let Some(path) = env_var(file_name) {
        let path = PathBuf::from(path);
        let contents =
            std::fs::read_to_string(&path).map_err(|source| ConfigError::Unreadable {
                name: file_name,
                path: path.clone(),
                source,
            })?;
        return Ok(Some(contents.trim().to_owned()));
    }
    Ok(env_var(name))
}

/// Parse `KEY=VALUE` lines (blank lines and `#` comments ignored).
pub fn parse_credentials_file(path: &Path) -> Result<S3Credentials, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Unreadable {
        name: "QFDEMO_STORE_CREDENTIALS_FILE",
        path: path.to_owned(),
        source,
    })?;
    let mut access_key_id = None;
    let mut secret_access_key = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "QFDEMO_STORE_ACCESS_KEY" => access_key_id = Some(value.trim().to_owned()),
                "QFDEMO_STORE_SECRET_KEY" => secret_access_key = Some(value.trim().to_owned()),
                _ => {}
            }
        }
    }
    Ok(S3Credentials {
        access_key_id: access_key_id.ok_or(ConfigError::MissingCredential {
            path: path.to_owned(),
            key: "QFDEMO_STORE_ACCESS_KEY",
        })?,
        secret_access_key: secret_access_key.ok_or(ConfigError::MissingCredential {
            path: path.to_owned(),
            key: "QFDEMO_STORE_SECRET_KEY",
        })?,
    })
}

impl DemoConfig {
    /// Read the full configuration from `QFDEMO_*` environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let listen_raw =
            env_var("QFDEMO_LISTEN_ADDR").unwrap_or_else(|| "127.0.0.1:8780".to_owned());
        let listen_addr: SocketAddr = listen_raw.parse().map_err(|_| ConfigError::Invalid {
            name: "QFDEMO_LISTEN_ADDR",
            value: listen_raw.clone(),
            expected: "a socket address such as 127.0.0.1:8780",
        })?;

        let database_url = env_or_file("QFDEMO_DATABASE_URL", "QFDEMO_DATABASE_URL_FILE")?.ok_or(
            ConfigError::Missing {
                name: "QFDEMO_DATABASE_URL",
            },
        )?;

        let auth_secret = env_or_file("QFDEMO_AUTH_SECRET", "QFDEMO_AUTH_SECRET_FILE")?.ok_or(
            ConfigError::Missing {
                name: "QFDEMO_AUTH_SECRET",
            },
        )?;
        queryfabric_paseto::validate_paseto_secret(&auth_secret).map_err(|error| {
            ConfigError::Invalid {
                name: "QFDEMO_AUTH_SECRET",
                value: "<redacted>".to_owned(),
                expected: match error {
                    queryfabric_paseto::AuthTokenError::SecretTooShort => {
                        "a secret of at least 32 bytes"
                    }
                    _ => "a valid PASETO secret",
                },
            }
        })?;

        let database_migration_url = env_or_file(
            "QFDEMO_DATABASE_MIGRATION_URL",
            "QFDEMO_DATABASE_MIGRATION_URL_FILE",
        )?
        .unwrap_or_else(|| database_url.clone());
        let database_query_url = env_or_file(
            "QFDEMO_DATABASE_QUERY_URL",
            "QFDEMO_DATABASE_QUERY_URL_FILE",
        )?
        .unwrap_or_else(|| database_url.clone());
        let database_import_url = env_or_file(
            "QFDEMO_DATABASE_IMPORT_URL",
            "QFDEMO_DATABASE_IMPORT_URL_FILE",
        )?
        .unwrap_or_else(|| database_url.clone());

        let db_wait_secs = match env_var("QFDEMO_DB_WAIT_SECS") {
            None => 60,
            Some(raw) => raw.parse().map_err(|_| ConfigError::Invalid {
                name: "QFDEMO_DB_WAIT_SECS",
                value: raw,
                expected: "a non-negative integer number of seconds",
            })?,
        };

        let public_base_url =
            env_var("QFDEMO_PUBLIC_BASE_URL").unwrap_or_else(|| format!("http://{listen_addr}"));

        let seed_demo_data = match env_var("QFDEMO_SEED_DATA").as_deref() {
            None | Some("true") | Some("1") => true,
            Some("false") | Some("0") => false,
            Some(other) => {
                return Err(ConfigError::Invalid {
                    name: "QFDEMO_SEED_DATA",
                    value: other.to_owned(),
                    expected: "'true' or 'false'",
                });
            }
        };

        let backend = env_var("QFDEMO_STORE_BACKEND").unwrap_or_else(|| "memory".to_owned());
        let store = match backend.as_str() {
            "memory" => StoreConfig::Memory,
            "s3" => StoreConfig::S3 {
                endpoint: env_var("QFDEMO_STORE_ENDPOINT").ok_or(ConfigError::Missing {
                    name: "QFDEMO_STORE_ENDPOINT",
                })?,
                bucket: env_var("QFDEMO_STORE_BUCKET").ok_or(ConfigError::Missing {
                    name: "QFDEMO_STORE_BUCKET",
                })?,
                region: env_var("QFDEMO_STORE_REGION").unwrap_or_else(|| "us-east-1".to_owned()),
                credentials_file: env_var("QFDEMO_STORE_CREDENTIALS_FILE")
                    .map(PathBuf::from)
                    .ok_or(ConfigError::Missing {
                        name: "QFDEMO_STORE_CREDENTIALS_FILE",
                    })?,
            },
            other => {
                return Err(ConfigError::Invalid {
                    name: "QFDEMO_STORE_BACKEND",
                    value: other.to_owned(),
                    expected: "'memory' or 's3'",
                });
            }
        };

        let federation_enable = match env_var("QFDEMO_FEDERATION_ENABLE").as_deref() {
            None | Some("false") | Some("0") => false,
            Some("true") | Some("1") => true,
            Some(other) => {
                return Err(ConfigError::Invalid {
                    name: "QFDEMO_FEDERATION_ENABLE",
                    value: other.to_owned(),
                    expected: "'true' or 'false'",
                });
            }
        };
        let flight_port = match env_var("QFDEMO_FEDERATION_FLIGHT_PORT") {
            None => 50051,
            Some(raw) => raw.parse().map_err(|_| ConfigError::Invalid {
                name: "QFDEMO_FEDERATION_FLIGHT_PORT",
                value: raw,
                expected: "a TCP port number",
            })?,
        };
        let federation = FederationConfig {
            enable: federation_enable,
            node_name: env_var("QFDEMO_FEDERATION_NODE_NAME")
                .unwrap_or_else(|| "queryfabric-demo".to_owned()),
            hub_multiaddrs: env_var("QFDEMO_FEDERATION_HUB_MULTIADDRS")
                .map(|raw| {
                    raw.split(',')
                        .map(str::trim)
                        .filter(|addr| !addr.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            flight_port,
        };

        Ok(Self {
            listen_addr,
            database_migration_url,
            database_query_url,
            database_import_url,
            auth_secret,
            db_wait_secs,
            public_base_url,
            seed_demo_data,
            store,
            federation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("qfdemo-test-{}-{name}", std::process::id()));
        std::fs::write(&path, contents).expect("write temp credentials");
        path
    }

    #[test]
    fn credentials_file_round_trips() {
        let path = write_temp(
            "creds-ok",
            "# demo credentials\nQFDEMO_STORE_ACCESS_KEY=minio\nQFDEMO_STORE_SECRET_KEY = hunter2 \n",
        );
        let creds = parse_credentials_file(&path).expect("parse");
        assert_eq!(creds.access_key_id, "minio");
        assert_eq!(creds.secret_access_key, "hunter2");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn credentials_file_missing_key_is_actionable() {
        let path = write_temp("creds-partial", "QFDEMO_STORE_ACCESS_KEY=minio\n");
        let err = parse_credentials_file(&path).expect_err("must fail");
        assert!(err.to_string().contains("QFDEMO_STORE_SECRET_KEY"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn secrets_never_appear_in_debug_output() {
        let config = DemoConfig {
            listen_addr: "127.0.0.1:8780".parse().expect("addr"),
            database_migration_url: "postgres://migration@localhost/db".to_owned(),
            database_query_url: "postgres://query@localhost/db".to_owned(),
            database_import_url: "postgres://import@localhost/db".to_owned(),
            auth_secret: "test-auth-secret-which-is-at-least-32-bytes".to_owned(),
            db_wait_secs: 60,
            public_base_url: "http://127.0.0.1:8780".to_owned(),
            seed_demo_data: true,
            store: StoreConfig::Memory,
            federation: FederationConfig {
                enable: false,
                node_name: "queryfabric-demo".to_owned(),
                hub_multiaddrs: Vec::new(),
                flight_port: 50051,
            },
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("tops3cret"));

        let creds = S3Credentials {
            access_key_id: "minio".to_owned(),
            secret_access_key: "hunter2".to_owned(),
        };
        assert!(!format!("{creds:?}").contains("hunter2"));
    }
}
