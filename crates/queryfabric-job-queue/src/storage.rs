//! Storage-facing helper types for job cleanup.

use uuid::Uuid;

/// Job row scheduled for cleanup because it has expired.
#[derive(Debug, Clone)]
pub struct ExpiredJob {
    /// Job id to delete.
    pub job_id: Uuid,
    /// Optional result object path to delete before removing the job row.
    pub result_path: Option<String>,
}
