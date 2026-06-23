use std::collections::BTreeMap;

use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EmptyDirVolumeSource, EnvVar, EnvVarSource, PodSpec, PodTemplateSpec,
    Probe, ResourceRequirements, SecretKeySelector, TCPSocketAction, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use queryfabric::{DriverError, IsolatedJobSpec, ResourceRequest, StorageAccessMode};

use crate::K8sDriverConfig;
use crate::storage::{env, storage_artifacts};

/// Label value for `app.kubernetes.io/managed-by`.
pub const MANAGED_BY: &str = "queryfabric-runtime-k8s";
/// Label key for storage mode annotation.
pub const LABEL_STORAGE_MODE: &str = "queryfabric.io/storage-mode";
/// Label key for query hash annotation.
pub const LABEL_QUERY_HASH: &str = "queryfabric.io/query-hash";
/// Label key for isolated job reference.
pub const LABEL_ISOLATED_JOB: &str = "queryfabric.io/isolated-job";
/// Label key for component.
pub const LABEL_COMPONENT: &str = "queryfabric.io/component";
/// Prefix for generated Job names.
pub const JOB_NAME_PREFIX: &str = "qf-isolated-";
/// Container name for the worker.
pub const WORKER_CONTAINER: &str = "burst-worker";

/// Build a Kubernetes `Job` manifest for an isolated query execution.
pub fn build_job(
    name: &str,
    spec: &IsolatedJobSpec,
    config: &K8sDriverConfig,
) -> Result<Job, DriverError> {
    let storage = storage_artifacts(&spec.storage)?;
    let labels = labels_for_job(name, config);
    let mut env_vars = base_env(spec, config)?;
    env_vars.extend(storage.env);

    let worker = Container {
        name: WORKER_CONTAINER.to_owned(),
        image: Some(config.worker_image.clone()),
        image_pull_policy: Some("IfNotPresent".to_owned()),
        // Let the worker image's default Cmd run (typically `burst-worker`,
        // a shim that reads SYNDB_* env, manages clickhouse-server as a
        // subprocess, and serves results via Arrow Flight). Setting `command`
        // here bypasses the shim and execs clickhouse-server directly, which
        // skips Flight setup and ignores the IsolatedJobSpec env contract.
        command: None,
        env: Some(env_vars),
        ports: Some(vec![ContainerPort {
            name: Some("flight".to_owned()),
            container_port: i32::from(config.flight_port),
            protocol: Some("TCP".to_owned()),
            ..ContainerPort::default()
        }]),
        readiness_probe: Some(flight_probe(config.flight_port)),
        liveness_probe: Some(flight_probe(config.flight_port)),
        resources: Some(resources(&spec.resources)),
        // Always mount /tmp as emptyDir — the shim writes temp files there,
        // and chart-default `readOnlyRootFilesystem: true` would otherwise
        // block every write under "/". Storage-mode-specific mounts are
        // appended after this base mount.
        volume_mounts: Some({
            let mut mounts = vec![VolumeMount {
                name: "tmp".to_owned(),
                mount_path: "/tmp".to_owned(),
                ..VolumeMount::default()
            }];
            mounts.extend(storage.mounts);
            mounts
        }),
        ..Container::default()
    };

    Ok(Job {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            labels: Some(labels.clone()),
            annotations: Some(BTreeMap::from([(
                LABEL_STORAGE_MODE.to_owned(),
                storage_mode(&spec.storage).to_owned(),
            )])),
            ..ObjectMeta::default()
        },
        spec: Some(JobSpec {
            ttl_seconds_after_finished: Some(config.job_ttl_seconds_after_finished),
            backoff_limit: Some(0),
            active_deadline_seconds: Some(spec.timeout.as_secs() as i64),
            // Leave `selector` unset and rely on Kubernetes auto-generation
            // (driven by the controller-uid label the apiserver adds). Setting
            // it manually here triggers a 422 "selector not auto-generated"
            // unless `manual_selector: true` is also set — we don't need that.
            selector: None,
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    annotations: Some(BTreeMap::from([(
                        LABEL_QUERY_HASH.to_owned(),
                        spec.query.provenance().query_hash.clone(),
                    )])),
                    ..ObjectMeta::default()
                }),
                spec: Some(PodSpec {
                    service_account_name: Some(config.service_account.clone()),
                    restart_policy: Some("Never".to_owned()),
                    containers: vec![worker],
                    tolerations: if config.default_tolerations.is_empty() {
                        None
                    } else {
                        Some(config.default_tolerations.clone())
                    },
                    volumes: Some({
                        let mut vols = vec![Volume {
                            name: "tmp".to_owned(),
                            empty_dir: Some(EmptyDirVolumeSource::default()),
                            ..Volume::default()
                        }];
                        vols.extend(storage.volumes);
                        vols
                    }),
                    ..PodSpec::default()
                }),
            },
            ..JobSpec::default()
        }),
        ..Job::default()
    })
}

/// Derive a stable Job name from the query hash.
pub fn job_name_for(spec: &IsolatedJobSpec) -> String {
    let hash = &spec.query.provenance().query_hash;
    let suffix: String = hash.chars().take(12).collect();
    format!("{JOB_NAME_PREFIX}{suffix}")
}

/// Build the label selector used to find a worker pod for a given Job.
pub fn job_label_selector(name: &str) -> String {
    format!("app.kubernetes.io/managed-by={MANAGED_BY},{LABEL_ISOLATED_JOB}={name}")
}

fn labels_for_job(name: &str, config: &K8sDriverConfig) -> BTreeMap<String, String> {
    let mut labels = config.pod_template_labels.clone();
    labels.insert(
        "app.kubernetes.io/name".to_owned(),
        WORKER_CONTAINER.to_owned(),
    );
    labels.insert(
        "app.kubernetes.io/managed-by".to_owned(),
        MANAGED_BY.to_owned(),
    );
    labels.insert(LABEL_ISOLATED_JOB.to_owned(), name.to_owned());
    labels
}

fn base_env(spec: &IsolatedJobSpec, config: &K8sDriverConfig) -> Result<Vec<EnvVar>, DriverError> {
    let query_json = serde_json::to_string(&spec.query)
        .map_err(|error| DriverError::Spawn(format!("serialize isolated query: {error}")))?;
    Ok(vec![
        env("SYNDB_ISOLATED_QUERY_JSON", &query_json),
        env("SYNDB_FLIGHT_PORT", &config.flight_port.to_string()),
        env("SYNDB_STORAGE_MODE", storage_mode(&spec.storage)),
        env(
            "SYNDB_REPLICATED_CLICKHOUSE_DATABASE",
            &config.clickhouse_database,
        ),
        env(
            "SYNDB_REPLICATED_CLICKHOUSE_USERNAME",
            &config.clickhouse_username,
        ),
        secret_env(
            "SYNDB_REPLICATED_CLICKHOUSE_PASSWORD",
            &config.clickhouse_password_secret_name,
            &config.clickhouse_password_secret_key,
        ),
        // Pin where the burst-worker's `remote()` rewrite should route the
        // outer query. Sourced from `K8sDriverConfig` so the API's own
        // ClickHouse host/native-port is the single source of truth; we
        // never let the worker fall back to a hardcoded service name.
        env(
            "SYNDB_REPLICATED_CLICKHOUSE_HOST",
            &config.replicated_clickhouse_host,
        ),
        env(
            "SYNDB_REPLICATED_CLICKHOUSE_NATIVE_PORT",
            &config.replicated_clickhouse_native_port.to_string(),
        ),
    ])
}

fn secret_env(name: &str, secret_name: &str, secret_key: &str) -> EnvVar {
    EnvVar {
        name: name.to_owned(),
        value_from: Some(EnvVarSource {
            secret_key_ref: Some(SecretKeySelector {
                name: secret_name.to_owned(),
                key: secret_key.to_owned(),
                optional: Some(false),
            }),
            ..EnvVarSource::default()
        }),
        ..EnvVar::default()
    }
}

fn resources(request: &ResourceRequest) -> ResourceRequirements {
    ResourceRequirements {
        requests: Some(BTreeMap::from([
            ("cpu".to_owned(), Quantity(request.cpu_request.clone())),
            (
                "memory".to_owned(),
                Quantity(request.memory_request.clone()),
            ),
        ])),
        limits: Some(BTreeMap::from([
            ("cpu".to_owned(), Quantity(request.cpu_limit.clone())),
            ("memory".to_owned(), Quantity(request.memory_limit.clone())),
        ])),
        ..ResourceRequirements::default()
    }
}

fn flight_probe(port: u16) -> Probe {
    Probe {
        tcp_socket: Some(TCPSocketAction {
            port: IntOrString::Int(i32::from(port)),
            ..TCPSocketAction::default()
        }),
        period_seconds: Some(2),
        failure_threshold: Some(30),
        ..Probe::default()
    }
}

fn storage_mode(storage: &StorageAccessMode) -> &'static str {
    match storage {
        StorageAccessMode::ReplicatedReadOnly => "replicated-read-only",
        StorageAccessMode::SnapshotClone { .. } => "snapshot-clone",
        StorageAccessMode::ObjectStore { .. } => "object-store",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use queryfabric::{
        BoundQuery, IsolatedJobSpec, ParsedQuery, ResourceRequest, StorageAccessMode,
    };
    use serde_json::json;

    use super::LABEL_COMPONENT;
    use crate::{K8sDriverConfig, job_spec};

    fn test_spec(storage: StorageAccessMode) -> IsolatedJobSpec {
        let parsed = ParsedQuery::new("sql", "SELECT 1", "SELECT 1");
        IsolatedJobSpec {
            query: BoundQuery::new(parsed),
            storage,
            resources: ResourceRequest {
                cpu_request: "500m".to_owned(),
                memory_request: "1Gi".to_owned(),
                cpu_limit: "600m".to_owned(),
                memory_limit: "1200Mi".to_owned(),
            },
            timeout: Duration::from_secs(300),
        }
    }

    fn test_config() -> K8sDriverConfig {
        K8sDriverConfig {
            worker_image: "registry.example/syndb/burst-worker:test".to_owned(),
            service_account: "syndb-burst-worker".to_owned(),
            flight_port: 8815,
            job_ttl_seconds_after_finished: 60,
            pod_template_labels: BTreeMap::from([(LABEL_COMPONENT.to_owned(), "burst".to_owned())]),
            default_tolerations: Vec::new(),
            clickhouse_database: "syndb".to_owned(),
            clickhouse_username: "syndb".to_owned(),
            clickhouse_password_secret_name: "syndb-api-secrets".to_owned(),
            clickhouse_password_secret_key: "clickhouse_password".to_owned(),
            replicated_clickhouse_host: "syndb-cluster.syndb.svc".to_owned(),
            replicated_clickhouse_native_port: 9000,
        }
    }

    #[test]
    fn replicated_read_only_manifest_matches_golden_projection() {
        let spec = test_spec(StorageAccessMode::ReplicatedReadOnly);
        let job = job_spec::build_job("qf-isolated-test", &spec, &test_config()).expect("job");
        let container = &job
            .spec
            .as_ref()
            .expect("spec")
            .template
            .spec
            .as_ref()
            .expect("pod spec")
            .containers[0];
        let resources = container.resources.as_ref().expect("resources");
        let projection = json!({
            "name": job.metadata.name,
            "labels": job.metadata.labels,
            "serviceAccount": job.spec.as_ref().unwrap().template.spec.as_ref().unwrap().service_account_name,
            "ttlSecondsAfterFinished": job.spec.as_ref().unwrap().ttl_seconds_after_finished,
            "activeDeadlineSeconds": job.spec.as_ref().unwrap().active_deadline_seconds,
            "container": {
                "name": container.name,
                "image": container.image,
                "command": container.command,
                "ports": container.ports.as_ref().unwrap().iter().map(|port| port.container_port).collect::<Vec<_>>(),
                "envNames": container.env.as_ref().unwrap().iter().map(|env| env.name.clone()).collect::<Vec<_>>(),
                "requests": resources.requests,
                "limits": resources.limits,
            },
            "volumes": job.spec.as_ref().unwrap().template.spec.as_ref().unwrap().volumes,
        });
        let actual = serde_json::to_string_pretty(&projection).expect("json");
        assert_eq!(
            actual,
            include_str!("../tests/golden/replicated_read_only_job.json").trim()
        );
    }

    #[test]
    fn driver_can_be_boxed_as_isolated_execution_driver() {
        let driver = crate::K8sIsolatedDriver::new_for_tests(test_config());
        let _: Box<dyn queryfabric::IsolatedExecutionDriver> = Box::new(driver);
    }

    #[test]
    fn job_name_uses_query_hash_prefix() {
        let spec = test_spec(StorageAccessMode::ReplicatedReadOnly);
        let name = job_spec::job_name_for(&spec);
        assert!(name.starts_with("qf-isolated-"));
        assert!(name.len() <= 28);
    }
}
