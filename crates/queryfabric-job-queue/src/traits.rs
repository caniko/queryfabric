//! Storage, result-store, and executor traits for the generic queue.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::JobQueueError;
use crate::schema::{JobPriority, JobRecord, JobResultManifest, JobStatus, ResultContent};
use crate::storage::ExpiredJob;

/// Result reported by an executor after running a job.
#[derive(Debug, Clone)]
pub struct JobExecutionResult {
    /// Terminal job status.
    pub status: JobStatus,
    /// Optional storage path for result bytes.
    pub result_path: Option<String>,
    /// Optional row count.
    pub row_count: Option<i64>,
    /// Optional byte count.
    pub byte_count: Option<i64>,
    /// Optional error message.
    pub error: Option<String>,
}

impl JobExecutionResult {
    /// Construct a successful completion result.
    #[must_use]
    pub fn completed(
        result_path: Option<String>,
        row_count: Option<i64>,
        byte_count: Option<i64>,
    ) -> Self {
        Self {
            status: JobStatus::Completed,
            result_path,
            row_count,
            byte_count,
            error: None,
        }
    }

    /// Construct a failed execution result with an error message.
    #[must_use]
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            status: JobStatus::Failed,
            result_path: None,
            row_count: None,
            byte_count: None,
            error: Some(error.into()),
        }
    }
}

/// Persistence interface required by the generic job queue.
#[async_trait]
pub trait JobStorage<K, P, U = Uuid>: Clone + Send + Sync + 'static
where
    K: Clone + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
{
    /// Load one job record by id.
    async fn load_job(&self, job_id: Uuid) -> Result<Option<JobRecord<K, P, U>>, JobQueueError>;
    /// Insert a newly submitted pending job.
    async fn insert_pending(&self, job: JobRecord<K, P, U>) -> Result<(), JobQueueError>;
    /// Mark a job as running.
    async fn mark_running(&self, job_id: Uuid) -> Result<(), JobQueueError>;
    /// Persist a terminal execution result.
    async fn complete_job(
        &self,
        job_id: Uuid,
        result: JobExecutionResult,
    ) -> Result<(), JobQueueError>;
    /// Mark a job as failed with an error message.
    async fn fail_job(&self, job_id: Uuid, error: String) -> Result<(), JobQueueError>;
    /// Mark a job as cancelled.
    async fn cancel_job(&self, job_id: Uuid) -> Result<(), JobQueueError>;
    /// Return all pending jobs as `(job_id, priority, kind)` tuples.
    async fn pending_jobs(&self) -> Result<Vec<(Uuid, JobPriority, K)>, JobQueueError>;
    /// Convert stale running jobs into failed jobs after process restart.
    async fn mark_stale_running_failed(&self, error: String) -> Result<usize, JobQueueError>;
    /// Return expired jobs that should be cleaned up.
    async fn expired_jobs(&self) -> Result<Vec<ExpiredJob>, JobQueueError>;
    /// Delete one job record permanently.
    async fn delete_job(&self, job_id: Uuid) -> Result<(), JobQueueError>;
    /// Load the result manifest for one job and owner.
    async fn result_manifest(
        &self,
        job_id: Uuid,
        user_id: U,
    ) -> Result<JobResultManifest, JobQueueError>;
}

/// Object-store interface for reading and writing job results.
#[async_trait]
pub trait ResultStore: Clone + Send + Sync + 'static {
    /// Write result bytes to `path`.
    async fn write(&self, path: &str, bytes: Vec<u8>) -> Result<(), JobQueueError>;
    /// Read result bytes from `path`.
    async fn read(&self, path: &str) -> Result<Vec<u8>, JobQueueError>;
    /// Delete result bytes at `path`.
    async fn delete(&self, path: &str) -> Result<(), JobQueueError>;
}

/// Domain-specific executor for queued jobs and their downloadable results.
#[async_trait]
pub trait JobExecutor<K, P, R, U = Uuid>: Clone + Send + Sync + 'static
where
    K: Clone + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    R: ResultStore,
    U: Clone + Send + Sync + 'static,
{
    /// Execute one job and return its terminal result.
    async fn execute(
        &self,
        job: JobRecord<K, P, U>,
        result_store: &R,
        cancel_token: &CancellationToken,
    ) -> Result<JobExecutionResult, JobQueueError>;

    /// Produce downloadable result bytes and metadata for a finished job.
    async fn result_content(
        &self,
        job: JobRecord<K, P, U>,
        result_store: &R,
    ) -> Result<ResultContent, JobQueueError>;
}
