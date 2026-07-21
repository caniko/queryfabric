use std::collections::HashMap;
use std::net::TcpListener;
use std::time::Duration;

use bollard::Docker;
use serde::Deserialize;

use crate::{
    cleanup_network, connect_docker, ensure_image, ensure_network,
    start_container_with_port_bindings, start_container_with_ports, upload_tar_to_container,
    wait_for_port,
};

const POSTGRES_IMAGE: &str = "docker.io/library/postgres:17";
const CLICKHOUSE_IMAGE: &str = "docker.io/clickhouse/clickhouse-server:25.8";
const MINIO_IMAGE: &str = "docker.io/minio/minio:RELEASE.2025-02-28T09-55-16Z";
const MEILISEARCH_IMAGE: &str = "docker.io/getmeili/meilisearch:v1.12.8";
const POSTGRES_USER: &str = "postgres";
const POSTGRES_PASSWORD: &str = "postgres";
const POSTGRES_DB: &str = "syndb";
const MINIO_ACCESS_KEY: &str = "minioadmin";
const MINIO_SECRET_KEY: &str = "minioadmin";
const CH_JSON_USERS_XML: &str = r#"<clickhouse>
    <profiles>
        <default>
            <enable_json_type>1</enable_json_type>
        </default>
    </profiles>
</clickhouse>
"#;

fn reserve_local_port() -> eyre::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Running Postgres service inside the test rig.
#[derive(Clone, Debug)]
pub struct PostgresService {
    host_port: u16,
    user: String,
    password: String,
    database: String,
}

impl PostgresService {
    /// Return the host used to reach the Postgres container from the test process.
    #[must_use]
    pub fn host(&self) -> &str {
        "127.0.0.1"
    }

    /// Return the mapped host port for Postgres.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.host_port
    }

    /// Return the configured Postgres username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.user
    }

    /// Return the configured Postgres password.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Return the configured Postgres database name.
    #[must_use]
    pub fn database(&self) -> &str {
        &self.database
    }

    /// Build a libpq-style connection URL for the service.
    #[must_use]
    pub fn url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user,
            self.password,
            self.host(),
            self.host_port,
            self.database
        )
    }

    /// Truncate every table in the configured Postgres database.
    ///
    /// # Errors
    /// Returns any connection, query, or `TRUNCATE` error from Postgres.
    pub async fn truncate_all(&self) -> eyre::Result<()> {
        let (client, connection) =
            tokio_postgres::connect(&self.url(), tokio_postgres::NoTls).await?;
        let connection_task = tokio::spawn(connection);

        let result = match client
            .query(
                "SELECT tablename \
                 FROM pg_tables \
                 WHERE schemaname = 'public' \
                 ORDER BY tablename",
                &[],
            )
            .await
        {
            Ok(rows) if rows.is_empty() => Ok(()),
            Ok(rows) => {
                let tables = rows
                    .into_iter()
                    .map(|row| {
                        format!(
                            "public.\"{}\"",
                            row.get::<_, String>(0).replace('"', "\"\"")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                client
                    .batch_execute(&format!("TRUNCATE TABLE {tables} RESTART IDENTITY CASCADE"))
                    .await
                    .map_err(Into::into)
            }
            Err(error) => Err(error.into()),
        };
        drop(client);
        match (result, connection_task.await) {
            (Err(error), _) => Err(error),
            (Ok(()), Ok(Ok(()))) => Ok(()),
            (Ok(()), Ok(Err(error))) => Err(error.into()),
            (Ok(()), Err(error)) => Err(error.into()),
        }
    }
}

/// Running ClickHouse service inside the test rig.
#[derive(Clone, Debug)]
pub struct ClickHouseService {
    name: String,
    host_port: u16,
    native_host_port: Option<u16>,
}

impl ClickHouseService {
    /// Return the logical service name used inside the test rig.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the host used to reach ClickHouse from the test process.
    #[must_use]
    pub fn host(&self) -> &str {
        "127.0.0.1"
    }

    /// Return the mapped HTTP port for ClickHouse.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.host_port
    }

    /// Return the optional mapped native ClickHouse port.
    #[must_use]
    pub fn native_port(&self) -> Option<u16> {
        self.native_host_port
    }

    /// Build the HTTP base URL for ClickHouse.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host(), self.host_port)
    }

    /// Truncate every table in `database`.
    ///
    /// # Errors
    /// Returns any ClickHouse query or connection error.
    pub async fn reset_database(&self, database: &str) -> eyre::Result<()> {
        #[derive(Debug, Deserialize, clickhouse::Row)]
        struct TableRow {
            name: String,
        }

        let client = clickhouse::Client::default()
            .with_url(self.url())
            .with_database(database);
        let rows = client
            .query(
                "SELECT name \
                 FROM system.tables \
                 WHERE database = currentDatabase() \
                   AND engine NOT IN ('View', 'MaterializedView') \
                   AND is_temporary = 0 \
                 ORDER BY name",
            )
            .fetch_all::<TableRow>()
            .await?;

        for row in rows {
            client
                .query(&format!(
                    "TRUNCATE TABLE IF EXISTS \"{}\"",
                    row.name.replace('"', "\"\"")
                ))
                .execute()
                .await?;
        }
        Ok(())
    }
}

/// Running MinIO service inside the test rig.
#[derive(Clone, Debug)]
pub struct MinioService {
    host_port: u16,
}

impl MinioService {
    /// Return the host used to reach MinIO from the test process.
    #[must_use]
    pub fn host(&self) -> &str {
        "127.0.0.1"
    }

    /// Return the mapped host port for MinIO.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.host_port
    }

    /// Build the HTTP endpoint URL for MinIO.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("http://{}:{}", self.host(), self.host_port)
    }

    /// Return the configured MinIO access key.
    #[must_use]
    pub fn access_key(&self) -> &str {
        MINIO_ACCESS_KEY
    }

    /// Return the configured MinIO secret key.
    #[must_use]
    pub fn secret_key(&self) -> &str {
        MINIO_SECRET_KEY
    }
}

/// Running Meilisearch service inside the test rig.
#[derive(Clone, Debug)]
pub struct MeilisearchService {
    host_port: u16,
}

impl MeilisearchService {
    /// Return the host used to reach Meilisearch from the test process.
    #[must_use]
    pub fn host(&self) -> &str {
        "127.0.0.1"
    }

    /// Return the mapped host port for Meilisearch.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.host_port
    }

    /// Build the HTTP base URL for Meilisearch.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host(), self.host_port)
    }
}

#[derive(Clone, Debug)]
struct ClickHouseSpec {
    name: String,
    host_port: Option<u16>,
    native_host_port: Option<u16>,
}

/// Builder for composing a multi-service integration-test rig.
#[derive(Default, Clone, Debug)]
pub struct TestRigBuilder {
    postgres_port: Option<u16>,
    clickhouse_specs: Vec<ClickHouseSpec>,
    minio_port: Option<u16>,
    meilisearch_port: Option<u16>,
}

impl TestRigBuilder {
    /// Enable a Postgres service on an ephemeral host port.
    #[must_use]
    pub fn with_postgres(mut self) -> Self {
        if self.postgres_port.is_none() {
            self.postgres_port = Some(0);
        }
        self
    }

    /// Enable a Postgres service bound to a specific host port.
    #[must_use]
    pub fn with_postgres_on_port(mut self, port: u16) -> Self {
        self.postgres_port = Some(port);
        self
    }

    /// Enable a default-named ClickHouse service.
    #[must_use]
    pub fn with_clickhouse(self) -> Self {
        self.with_clickhouse_named("clickhouse")
    }

    /// Enable a named ClickHouse service if it has not already been added.
    #[must_use]
    pub fn with_clickhouse_named(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if !self.clickhouse_specs.iter().any(|spec| spec.name == name) {
            self.clickhouse_specs.push(ClickHouseSpec {
                name,
                host_port: None,
                native_host_port: None,
            });
        }
        self
    }

    /// Set the HTTP port for the default ClickHouse service.
    #[must_use]
    pub fn with_clickhouse_on_port(mut self, port: u16) -> Self {
        if self.clickhouse_specs.is_empty() {
            self.clickhouse_specs.push(ClickHouseSpec {
                name: "clickhouse".to_owned(),
                host_port: Some(port),
                native_host_port: None,
            });
        } else if let Some(spec) = self.clickhouse_specs.first_mut() {
            spec.host_port = Some(port);
        }
        self
    }

    /// Add a named ClickHouse service with optional HTTP/native host ports.
    #[must_use]
    pub fn with_clickhouse_named_native(
        mut self,
        name: impl Into<String>,
        http_port: Option<u16>,
        native_port: Option<u16>,
    ) -> Self {
        self.clickhouse_specs.push(ClickHouseSpec {
            name: name.into(),
            host_port: http_port,
            native_host_port: native_port,
        });
        self
    }

    /// Enable a MinIO service on an ephemeral host port.
    #[must_use]
    pub fn with_minio(mut self) -> Self {
        if self.minio_port.is_none() {
            self.minio_port = Some(0);
        }
        self
    }

    /// Enable a MinIO service bound to a specific host port.
    #[must_use]
    pub fn with_minio_on_port(mut self, port: u16) -> Self {
        self.minio_port = Some(port);
        self
    }

    /// Enable a Meilisearch service on an ephemeral host port.
    #[must_use]
    pub fn with_meilisearch(mut self) -> Self {
        if self.meilisearch_port.is_none() {
            self.meilisearch_port = Some(0);
        }
        self
    }

    /// Enable a Meilisearch service bound to a specific host port.
    #[must_use]
    pub fn with_meilisearch_on_port(mut self, port: u16) -> Self {
        self.meilisearch_port = Some(port);
        self
    }

    /// Build the configured test rig asynchronously.
    ///
    /// # Errors
    /// Returns any container-runtime, image, network, or service startup error.
    pub async fn build(self) -> eyre::Result<TestRig> {
        TestRig::build(self).await
    }

    /// Build the configured test rig from either async or sync contexts.
    ///
    /// # Errors
    /// Returns any runtime-construction or service startup error.
    pub fn build_blocking(self) -> eyre::Result<TestRig> {
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(self.build())
            })
            .join()
            .map_err(|_| eyre::eyre!("TestRig builder thread panicked"))?
        } else {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(self.build())
        }
    }
}

/// Running integration-test rig composed of optional service containers.
pub struct TestRig {
    docker: Docker,
    network: String,
    postgres: Option<PostgresService>,
    clickhouse: HashMap<String, ClickHouseService>,
    minio: Option<MinioService>,
    meilisearch: Option<MeilisearchService>,
}

impl TestRig {
    /// Return a new empty builder.
    #[must_use]
    pub fn builder() -> TestRigBuilder {
        TestRigBuilder::default()
    }

    async fn build(builder: TestRigBuilder) -> eyre::Result<Self> {
        let docker = connect_docker()?;
        let network = format!("syndb-test-rig-{}", uuid::Uuid::now_v7().simple());
        ensure_network(&docker, &network).await?;
        let prefix = network.replace('_', "-");
        let services = async {
            let postgres = if let Some(port) = builder.postgres_port {
                Some(start_postgres(&docker, &network, &prefix, port).await?)
            } else {
                None
            };

            let mut clickhouse = HashMap::new();
            for spec in builder.clickhouse_specs {
                let service = start_clickhouse(&docker, &network, &prefix, spec).await?;
                clickhouse.insert(service.name.clone(), service);
            }

            let minio = if let Some(port) = builder.minio_port {
                Some(start_minio(&docker, &network, &prefix, port).await?)
            } else {
                None
            };

            let meilisearch = if let Some(port) = builder.meilisearch_port {
                Some(start_meilisearch(&docker, &network, &prefix, port).await?)
            } else {
                None
            };

            Ok::<_, eyre::Report>((postgres, clickhouse, minio, meilisearch))
        }
        .await;

        match services {
            Ok((postgres, clickhouse, minio, meilisearch)) => Ok(Self {
                docker,
                network,
                postgres,
                clickhouse,
                minio,
                meilisearch,
            }),
            Err(error) => {
                if let Err(cleanup_error) = cleanup_network(&docker, &network).await {
                    tracing::warn!(%cleanup_error, "failed to roll back test-rig network");
                }
                Err(error)
            }
        }
    }

    /// Return the underlying Docker client used by the rig.
    pub fn docker(&self) -> &Docker {
        &self.docker
    }

    /// Return the dedicated Docker network name used by the rig.
    #[must_use]
    pub fn network(&self) -> &str {
        &self.network
    }

    /// Shut down all containers and remove this rig's dedicated network.
    ///
    /// Cleanup is explicit so it does not depend on an unsafe process-exit
    /// hook or a hidden global registry.
    pub async fn shutdown(self) -> eyre::Result<()> {
        cleanup_network(&self.docker, &self.network).await
    }

    /// Return the configured Postgres service.
    ///
    /// # Errors
    /// Returns an error if the rig was built without Postgres.
    pub fn postgres(&self) -> eyre::Result<&PostgresService> {
        self.postgres
            .as_ref()
            .ok_or_else(|| eyre::eyre!("TestRig missing postgres"))
    }

    /// Return the default ClickHouse service, or the first configured one.
    ///
    /// # Errors
    /// Returns an error if the rig was built without any ClickHouse service.
    pub fn clickhouse(&self) -> eyre::Result<&ClickHouseService> {
        self.clickhouse
            .get("clickhouse")
            .or_else(|| self.clickhouse.values().next())
            .ok_or_else(|| eyre::eyre!("TestRig missing clickhouse"))
    }

    /// Return a named ClickHouse service.
    ///
    /// # Errors
    /// Returns an error if no ClickHouse service with `name` exists.
    pub fn clickhouse_named(&self, name: &str) -> eyre::Result<&ClickHouseService> {
        self.clickhouse
            .get(name)
            .ok_or_else(|| eyre::eyre!("TestRig missing clickhouse node {name}"))
    }

    /// Return the configured MinIO service.
    ///
    /// # Errors
    /// Returns an error if the rig was built without MinIO.
    pub fn minio(&self) -> eyre::Result<&MinioService> {
        self.minio
            .as_ref()
            .ok_or_else(|| eyre::eyre!("TestRig missing minio"))
    }

    /// Return the configured Meilisearch service.
    ///
    /// # Errors
    /// Returns an error if the rig was built without Meilisearch.
    pub fn meilisearch(&self) -> eyre::Result<&MeilisearchService> {
        self.meilisearch
            .as_ref()
            .ok_or_else(|| eyre::eyre!("TestRig missing meilisearch"))
    }
}

async fn start_postgres(
    docker: &Docker,
    network: &str,
    prefix: &str,
    configured_port: u16,
) -> eyre::Result<PostgresService> {
    let host_port = if configured_port == 0 {
        reserve_local_port()?
    } else {
        configured_port
    };
    ensure_image(docker, POSTGRES_IMAGE).await?;
    start_container_with_ports(
        docker,
        &format!("{prefix}-postgres"),
        network,
        POSTGRES_IMAGE,
        vec![
            format!("POSTGRES_USER={POSTGRES_USER}"),
            format!("POSTGRES_PASSWORD={POSTGRES_PASSWORD}"),
            format!("POSTGRES_DB={POSTGRES_DB}"),
        ],
        None,
        &[(5432, host_port)],
    )
    .await?;
    wait_for_port("127.0.0.1", host_port, Duration::from_secs(30)).await?;
    Ok(PostgresService {
        host_port,
        user: POSTGRES_USER.to_owned(),
        password: POSTGRES_PASSWORD.to_owned(),
        database: POSTGRES_DB.to_owned(),
    })
}

async fn start_clickhouse(
    docker: &Docker,
    network: &str,
    prefix: &str,
    spec: ClickHouseSpec,
) -> eyre::Result<ClickHouseService> {
    let host_port = spec.host_port.unwrap_or(0);
    let http_port = if host_port == 0 {
        reserve_local_port()?
    } else {
        host_port
    };
    let native_port = spec
        .native_host_port
        .map(|port| {
            if port == 0 {
                reserve_local_port()
            } else {
                Ok(port)
            }
        })
        .transpose()?;
    ensure_image(docker, CLICKHOUSE_IMAGE).await?;
    let id = if let Some(native_host_port) = native_port {
        start_container_with_port_bindings(
            docker,
            &format!("{prefix}-{}", spec.name),
            network,
            CLICKHOUSE_IMAGE,
            vec![
                "CLICKHOUSE_DB=syndb".into(),
                "CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1".into(),
                "CLICKHOUSE_LISTEN_HOST=0.0.0.0".into(),
            ],
            None,
            &[
                (8123, http_port, "127.0.0.1"),
                (9000, native_host_port, "0.0.0.0"),
            ],
        )
        .await?
    } else {
        start_container_with_ports(
            docker,
            &format!("{prefix}-{}", spec.name),
            network,
            CLICKHOUSE_IMAGE,
            vec![
                "CLICKHOUSE_DB=syndb".into(),
                "CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1".into(),
                "CLICKHOUSE_LISTEN_HOST=0.0.0.0".into(),
            ],
            None,
            &[(8123, http_port)],
        )
        .await?
    };

    upload_clickhouse_config(
        docker,
        &id,
        "/etc/clickhouse-server/users.d/",
        "json.xml",
        CH_JSON_USERS_XML,
    )
    .await?;
    wait_for_port("127.0.0.1", http_port, Duration::from_secs(30)).await?;
    if let Some(native_host_port) = native_port {
        wait_for_port("127.0.0.1", native_host_port, Duration::from_secs(30)).await?;
    }
    wait_for_clickhouse_http(http_port).await?;

    Ok(ClickHouseService {
        name: spec.name,
        host_port: http_port,
        native_host_port: native_port,
    })
}

async fn start_minio(
    docker: &Docker,
    network: &str,
    prefix: &str,
    configured_port: u16,
) -> eyre::Result<MinioService> {
    let host_port = if configured_port == 0 {
        reserve_local_port()?
    } else {
        configured_port
    };
    ensure_image(docker, MINIO_IMAGE).await?;
    start_container_with_ports(
        docker,
        &format!("{prefix}-minio"),
        network,
        MINIO_IMAGE,
        vec![
            format!("MINIO_ROOT_USER={MINIO_ACCESS_KEY}"),
            format!("MINIO_ROOT_PASSWORD={MINIO_SECRET_KEY}"),
            "MINIO_BROWSER=off".into(),
        ],
        Some(vec!["minio".into(), "server".into(), "/data".into()]),
        &[(9000, host_port)],
    )
    .await?;
    wait_for_port("127.0.0.1", host_port, Duration::from_secs(30)).await?;
    Ok(MinioService { host_port })
}

async fn start_meilisearch(
    docker: &Docker,
    network: &str,
    prefix: &str,
    configured_port: u16,
) -> eyre::Result<MeilisearchService> {
    let host_port = if configured_port == 0 {
        reserve_local_port()?
    } else {
        configured_port
    };
    ensure_image(docker, MEILISEARCH_IMAGE).await?;
    start_container_with_ports(
        docker,
        &format!("{prefix}-meilisearch"),
        network,
        MEILISEARCH_IMAGE,
        Vec::new(),
        None,
        &[(7700, host_port)],
    )
    .await?;
    wait_for_port("127.0.0.1", host_port, Duration::from_secs(30)).await?;
    wait_for_meilisearch_http(host_port).await?;
    Ok(MeilisearchService { host_port })
}

async fn upload_clickhouse_config(
    docker: &Docker,
    container_id: &str,
    target_dir: &str,
    file_name: &str,
    xml: &str,
) -> eyre::Result<()> {
    let data = xml.as_bytes();
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    let mut archive = tar::Builder::new(Vec::new());
    archive.append_data(&mut header, file_name, data)?;
    let tar_bytes = archive.into_inner()?;
    upload_tar_to_container(docker, container_id, target_dir, tar_bytes).await
}

async fn wait_for_clickhouse_http(host_port: u16) -> eyre::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{host_port}/?query=SELECT%201");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    eyre::bail!("ClickHouse on 127.0.0.1:{host_port} did not become query-ready")
}

async fn wait_for_meilisearch_http(host_port: u16) -> eyre::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{host_port}/health");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    eyre::bail!("Meilisearch on 127.0.0.1:{host_port} did not become healthy")
}
