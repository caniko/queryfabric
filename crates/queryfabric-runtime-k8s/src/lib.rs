//! Kubernetes execution runtime for isolated SynDB jobs.
//!
//! This crate builds `Job` manifests, waits for worker pods to become ready,
//! and streams Arrow Flight batches back into `queryfabric`.

mod cancel;
pub mod error;
pub mod job_spec;
pub mod storage;
mod stream;

use std::collections::BTreeMap;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::Stream;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::Toleration;
use kube::api::PostParams;
use kube::{Api, Client};
use queryfabric::{
    DriverError, IsolatedExecutionDriver, IsolatedJobSpec, RecordBatchStream, RuntimeError,
};
use tokio_util::sync::CancellationToken;

use crate::cancel::{delete_job_background, spawn_cancel_delete_task, spawn_traced};
use crate::error::{DriverResult, spawn_message};
use crate::job_spec::{build_job, job_name_for_attempt, new_attempt_suffix};

/// How many times `spawn` draws a fresh Job name when the create is refused
/// with 409 `AlreadyExists`.
const CREATE_ATTEMPTS: usize = 3;

/// Driver for running isolated SynDB queries as Kubernetes Jobs.
#[derive(Clone)]
pub struct K8sIsolatedDriver {
    client: Option<Client>,
    namespace: String,
    config: K8sDriverConfig,
}

/// Configuration for the isolated Kubernetes execution driver.
#[derive(Clone, Debug, PartialEq)]
pub struct K8sDriverConfig {
    /// Worker image used for isolated execution.
    pub worker_image: String,
    /// Service account attached to the worker pod.
    pub service_account: String,
    /// Flight server port exposed by the worker container.
    pub flight_port: u16,
    /// TTL applied to completed Jobs.
    pub job_ttl_seconds_after_finished: i32,
    /// Labels propagated to the pod template.
    pub pod_template_labels: BTreeMap<String, String>,
    /// Tolerations added to every worker pod.
    pub default_tolerations: Vec<Toleration>,
    /// ClickHouse database used by worker-side `remote()` rewrites.
    pub clickhouse_database: String,
    /// ClickHouse user used by worker-side `remote()` rewrites.
    pub clickhouse_username: String,
    /// Kubernetes Secret name that stores the ClickHouse password for workers.
    pub clickhouse_password_secret_name: String,
    /// Key within [`K8sDriverConfig::clickhouse_password_secret_name`] that
    /// stores the ClickHouse password for workers.
    pub clickhouse_password_secret_key: String,
    /// Hostname the burst-worker should use when rewriting `FROM <table>`
    /// to a `remote()` call against the main ClickHouse cluster for
    /// `ReplicatedReadOnly` jobs. This is forwarded into the worker pod as
    /// `SYNDB_REPLICATED_CLICKHOUSE_HOST` so that a single source of truth
    /// (the API's own ClickHouse host) drives where the outer query is
    /// federated, and no stale literal hides inside burst-worker.
    pub replicated_clickhouse_host: String,
    /// Native (TCP) port for the main ClickHouse cluster used by the
    /// burst-worker's `remote()` rewrite. Forwarded as
    /// `SYNDB_REPLICATED_CLICKHOUSE_NATIVE_PORT`.
    pub replicated_clickhouse_native_port: u16,
}

impl K8sIsolatedDriver {
    /// Create a driver from an explicit Kubernetes client.
    pub fn new(client: Client, namespace: impl Into<String>, config: K8sDriverConfig) -> Self {
        Self {
            client: Some(client),
            namespace: namespace.into(),
            config,
        }
    }

    /// Create a driver using the default in-cluster or kubeconfig client.
    pub async fn infer(
        namespace: impl Into<String>,
        config: K8sDriverConfig,
    ) -> DriverResult<Self> {
        let client = Client::try_default()
            .await
            .map_err(|error| DriverError::Spawn(format!("load Kubernetes client: {error}")))?;
        Ok(Self::new(client, namespace, config))
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(config: K8sDriverConfig) -> Self {
        Self {
            client: None,
            namespace: "syndb".to_owned(),
            config,
        }
    }

    /// Return the driver configuration.
    pub fn config(&self) -> &K8sDriverConfig {
        &self.config
    }

    /// Return the namespace used for isolated Jobs.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

#[async_trait]
impl IsolatedExecutionDriver for K8sIsolatedDriver {
    async fn spawn(
        &self,
        spec: IsolatedJobSpec,
        cancel: CancellationToken,
    ) -> Result<RecordBatchStream, DriverError> {
        if cancel.is_cancelled() {
            return Err(DriverError::Cancelled);
        }

        let client = self
            .client
            .clone()
            .ok_or_else(|| spawn_message("Kubernetes client is not configured"))?;
        let jobs: Api<Job> = Api::namespaced(client.clone(), &self.namespace);

        // Each attempt gets its own Job name, so a leftover Job from an earlier
        // attempt (or a concurrent caller running the same query) cannot make
        // the create fail with 409 AlreadyExists. A 409 anyway (suffix
        // collision) simply draws a new suffix.
        let mut created = None;
        for _ in 0..CREATE_ATTEMPTS {
            let job_name = job_name_for_attempt(&spec, &new_attempt_suffix());
            let job = build_job(&job_name, &spec, &self.config)?;
            match jobs.create(&PostParams::default(), &job).await {
                Ok(_) => {
                    created = Some(job_name);
                    break;
                }
                Err(kube::Error::Api(response)) if response.code == 409 => {
                    tracing::warn!(
                        job_name,
                        "isolated Job name already exists; retrying with a new suffix"
                    );
                }
                Err(error) => {
                    return Err(DriverError::Spawn(format!(
                        "create isolated Job {job_name}: {error}"
                    )));
                }
            }
        }
        let job_name = created.ok_or_else(|| {
            DriverError::Spawn(format!(
                "create isolated Job: name still taken after {CREATE_ATTEMPTS} attempts"
            ))
        })?;
        spawn_cancel_delete_task(jobs.clone(), job_name.clone(), cancel.clone());

        let stream = match stream::connect_job_stream(
            client,
            &self.namespace,
            &job_name,
            &spec,
            self.config.flight_port,
            cancel,
        )
        .await
        {
            Ok(stream) => stream,
            Err(error) => {
                // The worker is useless once we cannot reach it, and it would
                // otherwise run until its deadline. Delete it now.
                if let Err(delete_error) = delete_job_background(&jobs, &job_name).await {
                    tracing::warn!(job_name, %delete_error, "failed to delete isolated Job after a failed spawn");
                }
                return Err(error);
            }
        };

        Ok(Box::pin(CleanupStream::new(stream, jobs, job_name)))
    }
}

struct CleanupStream {
    inner: RecordBatchStream,
    jobs: Option<Api<Job>>,
    job_name: String,
}

impl CleanupStream {
    fn new(inner: RecordBatchStream, jobs: Api<Job>, job_name: String) -> Self {
        Self {
            inner,
            jobs: Some(jobs),
            job_name,
        }
    }

    fn schedule_cleanup(&mut self) {
        if let Some(jobs) = self.jobs.take() {
            let job_name = self.job_name.clone();
            spawn_traced("cleanup-delete-job", async move {
                if let Err(error) = delete_job_background(&jobs, &job_name).await {
                    tracing::warn!(job_name, %error, "failed to delete isolated execution Job");
                }
            });
        }
    }
}

impl Stream for CleanupStream {
    type Item = Result<arrow::record_batch::RecordBatch, RuntimeError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let next = self.inner.as_mut().poll_next(cx);
        if matches!(next, Poll::Ready(None)) {
            self.schedule_cleanup();
        }
        next
    }
}

impl Drop for CleanupStream {
    fn drop(&mut self) {
        self.schedule_cleanup();
    }
}

#[cfg(test)]
mod spawn_tests {
    use std::pin::pin;
    use std::time::Duration;

    use http::{Method, Response};
    use k8s_openapi::api::batch::v1::Job;
    use kube::Client;
    use kube::client::Body;
    use queryfabric::{IsolatedExecutionDriver, StorageAccessMode};
    use serde_json::json;
    use tokio_util::sync::CancellationToken;
    use tower_test::mock;

    use crate::K8sIsolatedDriver;
    use crate::job_spec::tests::{test_config, test_spec};

    fn status_body(code: u16, reason: &str) -> Body {
        Body::from(
            serde_json::to_vec(&json!({
                "kind": "Status", "apiVersion": "v1", "status": "Failure",
                "reason": reason, "code": code, "message": reason,
            }))
            .unwrap(),
        )
    }

    fn json_response(code: u16, value: serde_json::Value) -> Response<Body> {
        Response::builder()
            .status(code)
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap()
    }

    fn job_name_of(body: &[u8]) -> String {
        let job: Job = serde_json::from_slice(body).expect("Job body");
        job.metadata.name.expect("name")
    }

    /// A refused create (409) is retried under a new name, and when the worker
    /// then turns out to be unreachable (its pod failed) the Job is deleted
    /// instead of being left to run until its deadline.
    #[tokio::test]
    async fn spawn_retries_a_taken_name_and_deletes_the_job_when_connect_fails() {
        let (service, handle) = mock::pair();
        let client = Client::new(service, "syndb");
        let driver = K8sIsolatedDriver::new(client, "syndb", test_config());
        let spec = test_spec(StorageAccessMode::ReplicatedReadOnly);

        let spawn = tokio::spawn(async move {
            driver
                .spawn(spec, CancellationToken::new())
                .await
                .map(|_| ())
        });

        let mut handle = pin!(handle);
        let next = async |handle: &mut std::pin::Pin<
            &mut mock::Handle<http::Request<Body>, Response<Body>>,
        >| {
            tokio::time::timeout(Duration::from_secs(5), handle.next_request())
                .await
                .expect("request within 5s")
                .expect("request")
        };

        // 1st create: 409.
        let (req, send) = next(&mut handle).await;
        assert_eq!(req.method(), Method::POST);
        let first = job_name_of(&req.into_body().collect_bytes().await.unwrap());
        send.send_response(
            Response::builder()
                .status(409)
                .body(status_body(409, "AlreadyExists"))
                .unwrap(),
        );

        // 2nd create: a different name, accepted.
        let (req, send) = next(&mut handle).await;
        assert_eq!(req.method(), Method::POST);
        let second_body = req.into_body().collect_bytes().await.unwrap();
        let second = job_name_of(&second_body);
        assert_ne!(first, second, "the retry must use a new Job name");
        send.send_response(json_response(
            201,
            serde_json::from_slice(&second_body).unwrap(),
        ));

        // Pod lookup: the worker pod has failed.
        let (req, send) = next(&mut handle).await;
        assert_eq!(req.method(), Method::GET);
        assert!(req.uri().path().ends_with("/pods"), "{}", req.uri());
        send.send_response(json_response(
            200,
            json!({
                "kind": "PodList", "apiVersion": "v1", "metadata": {},
                "items": [{
                    "metadata": {"name": "worker-0"},
                    "status": {"phase": "Failed"}
                }]
            }),
        ));

        // The failed spawn deletes exactly the Job it created.
        let (req, send) = next(&mut handle).await;
        assert_eq!(req.method(), Method::DELETE);
        assert_eq!(
            req.uri().path(),
            format!("/apis/batch/v1/namespaces/syndb/jobs/{second}")
        );
        send.send_response(json_response(
            200,
            json!({"kind": "Status", "apiVersion": "v1", "status": "Success"}),
        ));

        let result = spawn.await.expect("spawn task");
        assert!(result.is_err(), "spawn must report the worker failure");
    }
}
