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
use crate::job_spec::{build_job, job_name_for};

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
        let job_name = job_name_for(&spec);
        let job = build_job(&job_name, &spec, &self.config)?;

        jobs.create(&PostParams::default(), &job)
            .await
            .map_err(|error| {
                DriverError::Spawn(format!("create isolated Job {job_name}: {error}"))
            })?;
        spawn_cancel_delete_task(jobs.clone(), job_name.clone(), cancel.clone());

        let stream = stream::connect_job_stream(
            client,
            &self.namespace,
            &job_name,
            &spec,
            self.config.flight_port,
            cancel,
        )
        .await?;

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
