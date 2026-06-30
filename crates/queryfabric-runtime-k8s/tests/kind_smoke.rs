#![cfg(feature = "integration-k8s")]

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use futures::StreamExt;
use queryfabric::{
    BoundQuery, IsolatedExecutionDriver, IsolatedJobSpec, ParsedQuery, ResourceRequest,
    StorageAccessMode,
};
use queryfabric_runtime_k8s::job_spec::LABEL_COMPONENT;
use queryfabric_runtime_k8s::{K8sDriverConfig, K8sIsolatedDriver};
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;

const CLUSTER_NAME: &str = "burst-smoke";
const RELEASE_NAME: &str = "burst-smoke";
const NAMESPACE: &str = "default";
const WORKER_IMAGE: &str = "docker.io/caniko/syndb-burst-worker:nix";
const FLIGHT_PORT: u16 = 50053;
const TIMEOUT: Duration = Duration::from_secs(300);

#[tokio::test]
#[ignore = "manual kind smoke; requires Docker, kind, helm, kubectl, and nix build .#oci-syndb-burst-worker"]
async fn kind_smoke_driver_spawns_worker_and_returns_recordbatch() -> Result<(), Box<dyn Error>> {
    let repo = repo_root()?;
    let image_archive = image_archive_path(&repo)?;
    let kind_config = kind_config_file()?;

    delete_kind_cluster();
    run_checked(
        Command::new("kind")
            .args([
                "create",
                "cluster",
                "--name",
                CLUSTER_NAME,
                "--config",
                path_str(kind_config.path())?,
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )?;
    run_checked(
        Command::new("kind")
            .args([
                "load",
                "image-archive",
                path_str(&image_archive)?,
                "--name",
                CLUSTER_NAME,
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )?;
    apply_burst_worker_chart(&repo)?;
    verify_default_service_account_can_spawn_jobs()?;

    let client = kube::Client::try_default().await?;
    let driver = K8sIsolatedDriver::new(client, NAMESPACE, test_driver_config());
    let cancel = CancellationToken::new();
    let mut stream = driver.spawn(test_job_spec(), cancel).await?;

    let batch = tokio::time::timeout(TIMEOUT, stream.next())
        .await
        .map_err(|_| "timed out waiting for burst-worker RecordBatch")?
        .ok_or("burst-worker stream ended before yielding a RecordBatch")??;

    assert_eq!(batch.num_rows(), 1);
    assert_eq!(batch.num_columns(), 1);

    delete_kind_cluster_checked()?;
    Ok(())
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|path| path.join("flake.nix").exists() && path.join("infrastructure/helm").exists())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "could not find SynDB repo root from {}",
                manifest_dir.display()
            )
            .into()
        })
}

fn image_archive_path(repo: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(path) = std::env::var("SYNDB_BURST_WORKER_IMAGE_ARCHIVE") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!(
            "SYNDB_BURST_WORKER_IMAGE_ARCHIVE points to missing path: {}",
            path.display()
        )
        .into());
    }

    let result = repo.join("result");
    if result.exists() {
        return Ok(result);
    }
    Err(format!(
        "missing burst-worker image archive at {}.\n\
         Build it before running this smoke test:\n\
         nix build .#oci-syndb-burst-worker",
        result.display()
    )
    .into())
}

fn kind_config_file() -> Result<NamedTempFile, Box<dyn Error>> {
    let file = NamedTempFile::new()?;
    std::fs::write(
        file.path(),
        r#"kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
"#,
    )?;
    Ok(file)
}

fn apply_burst_worker_chart(repo: &Path) -> Result<(), Box<dyn Error>> {
    let chart = repo.join("infrastructure/helm/syndb-clickhouse");
    let output = Command::new("helm")
        .args([
            "template",
            RELEASE_NAME,
            path_str(&chart)?,
            "--namespace",
            NAMESPACE,
            "--show-only",
            "templates/burst-worker-rbac.yaml",
            "--show-only",
            "templates/burst-worker-config.yaml",
            "--set",
            "burstWorker.enabled=true",
            "--set",
            &format!("burstWorker.image={WORKER_IMAGE}"),
            "--set",
            "burstWorker.spawnerServiceAccount=default",
            "--set",
            "burstWorker.resources.requests.cpu=500m",
            "--set",
            "burstWorker.resources.requests.memory=1Gi",
            "--set",
            "burstWorker.resources.limits.cpu=600m",
            "--set",
            "burstWorker.resources.limits.memory=1200Mi",
        ])
        .output()?;
    if !output.status.success() {
        return Err(command_error("helm template", &output).into());
    }

    let mut kubectl = Command::new("kubectl")
        .args(["--context", "kind-burst-smoke", "apply", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    std::io::Write::write_all(
        kubectl.stdin.as_mut().ok_or("open kubectl stdin")?,
        &output.stdout,
    )?;
    let status = kubectl.wait()?;
    if !status.success() {
        return Err(format!("kubectl apply failed with status {status}").into());
    }
    Ok(())
}

fn verify_default_service_account_can_spawn_jobs() -> Result<(), Box<dyn Error>> {
    run_checked(
        Command::new("kubectl")
            .args([
                "--context",
                "kind-burst-smoke",
                "auth",
                "can-i",
                "create",
                "jobs",
                "--as=system:serviceaccount:default:default",
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )
}

fn test_driver_config() -> K8sDriverConfig {
    K8sDriverConfig {
        worker_image: WORKER_IMAGE.to_owned(),
        service_account: format!("{RELEASE_NAME}-burst-worker"),
        flight_port: FLIGHT_PORT,
        job_ttl_seconds_after_finished: 60,
        pod_template_labels: BTreeMap::from([(
            LABEL_COMPONENT.to_owned(),
            "kind-smoke".to_owned(),
        )]),
        default_tolerations: Vec::new(),
        clickhouse_database: "syndb".to_owned(),
        clickhouse_username: "syndb".to_owned(),
        clickhouse_password_secret_name: "syndb-api-secrets".to_owned(),
        clickhouse_password_secret_key: "clickhouse_password".to_owned(),
        replicated_clickhouse_host: "syndb-cluster.syndb.svc".to_owned(),
        replicated_clickhouse_native_port: 9000,
    }
}

fn test_job_spec() -> IsolatedJobSpec {
    IsolatedJobSpec {
        query: BoundQuery::new(ParsedQuery::new("sql", "SELECT 1", "SELECT 1")),
        storage: StorageAccessMode::ReplicatedReadOnly,
        resources: ResourceRequest {
            cpu_request: "500m".to_owned(),
            memory_request: "1Gi".to_owned(),
            cpu_limit: "600m".to_owned(),
            memory_limit: "1200Mi".to_owned(),
        },
        timeout: TIMEOUT,
    }
}

fn delete_kind_cluster() {
    let _ = Command::new("kind")
        .args(["delete", "cluster", "--name", CLUSTER_NAME])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
}

fn delete_kind_cluster_checked() -> Result<(), Box<dyn Error>> {
    run_checked(
        Command::new("kind")
            .args(["delete", "cluster", "--name", CLUSTER_NAME])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )
}

fn run_checked(command: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with status {status}: {command:?}").into())
    }
}

fn command_error(label: &str, output: &std::process::Output) -> String {
    format!(
        "{label} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn path_str(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.to_str()
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()).into())
}
