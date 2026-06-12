//! HTTP and storage-facing schema types for the generic job queue.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Canonical job identifier type.
pub type JobId = Uuid;
/// Integer priority where lower numbers dispatch first.
pub type JobPriority = i16;

/// Current lifecycle state of a queued job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is enqueued and waiting for dispatch.
    Pending,
    /// Job is currently executing.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed.
    Failed,
    /// Job was cancelled.
    Cancelled,
}

/// Persistent representation of one queued job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord<K, P, U = Uuid> {
    /// Job id.
    pub id: JobId,
    /// User or principal that owns the job.
    pub user_id: U,
    /// Current lifecycle state.
    pub status: JobStatus,
    /// Dispatch priority.
    pub priority: JobPriority,
    /// Domain-specific job kind.
    pub kind: K,
    /// Domain-specific request payload.
    pub payload: P,
    /// Storage path for result bytes, when present.
    pub result_path: Option<String>,
    /// Optional row count reported by the executor.
    pub row_count: Option<i64>,
    /// Optional byte count reported by the executor.
    pub byte_count: Option<i64>,
    /// Optional terminal error message.
    pub error: Option<String>,
    /// Job creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Execution start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Execution completion timestamp.
    pub completed_at: Option<DateTime<Utc>>,
    /// Expiration timestamp after which the job may be cleaned up.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Manifest describing how to fetch and interpret job results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct JobResultManifest {
    /// Manifest id.
    pub id: Uuid,
    /// Owning job id, when linked.
    pub job_id: Option<Uuid>,
    /// Canonical storage URI for the result object.
    pub storage_uri: String,
    /// Output format name.
    pub format: String,
    /// Total row count.
    pub row_count: i64,
    /// Total byte count.
    pub byte_count: i64,
    /// Number of logical pages or chunks.
    pub page_count: i32,
    /// Optional content hash.
    pub content_hash: Option<String>,
    /// Structured query-cost metadata.
    #[schema(value_type = Object)]
    pub query_cost: serde_json::Value,
    /// Structured manifest metadata.
    #[schema(value_type = Object)]
    pub manifest_json: serde_json::Value,
    /// Result-expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
    /// Manifest creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Response returned after successful job submission.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct SubmitJobResponse {
    /// Newly created job id.
    pub job_id: Uuid,
    /// Initial job status.
    pub status: JobStatus,
    /// Stored dispatch priority.
    pub priority: JobPriority,
}

/// Public representation of a job's current status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobStatusResponse<K = String> {
    /// Job id.
    pub id: Uuid,
    /// Current job status.
    pub status: JobStatus,
    /// Domain-specific job kind.
    pub job_kind: K,
    /// Stored dispatch priority.
    pub priority: JobPriority,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional row count reported by the executor.
    pub row_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional byte count reported by the executor.
    pub byte_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional result-manifest id.
    pub result_manifest_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional structured error payload.
    pub error: Option<serde_json::Value>,
    /// Job creation timestamp.
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Execution start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Execution completion timestamp.
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Cleanup-expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Query parameters for listing jobs.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListJobsQuery {
    /// Optional status filter.
    pub status: Option<JobStatus>,
    /// Maximum number of jobs to return.
    pub limit: Option<u64>,
    /// Pagination offset.
    pub offset: Option<u64>,
}

/// Paginated list response for jobs.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListJobsResponse<T = JobStatusResponse> {
    /// Returned job rows.
    pub data: Vec<T>,
    /// Total rows matching the query.
    pub total: u64,
}

/// Downloadable result bytes plus HTTP metadata.
#[derive(Debug, Clone)]
pub struct ResultContent {
    /// Raw result bytes.
    pub bytes: Vec<u8>,
    /// HTTP content type for the bytes.
    pub content_type: String,
    /// Suggested download filename.
    pub filename: String,
}
