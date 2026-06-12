//! Error types for the generic job queue.

use thiserror::Error;
use uuid::Uuid;

/// Coarse-grained error category for job queue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueErrorKind {
    /// Requested job or result does not exist.
    NotFound,
    /// Caller is not authorized to perform the action.
    Forbidden,
    /// Job cannot be cancelled in its current state.
    NotCancellable,
    /// Job result is not ready yet.
    NotReady,
    /// Caller supplied an invalid request.
    InvalidRequest,
    /// Result bytes or manifest are missing.
    ResultMissing,
    /// Backing storage layer failed.
    Storage,
    /// Job execution layer failed.
    Execution,
    /// Service is temporarily unavailable.
    Unavailable,
    /// Unexpected internal error.
    Internal,
}

/// Concrete job queue error with a category and displayable message.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct JobQueueError {
    /// Error category used for HTTP/status translation.
    pub kind: QueueErrorKind,
    /// Human-readable error message.
    pub message: String,
}

impl JobQueueError {
    /// Construct a new job queue error.
    #[must_use]
    pub fn new(kind: QueueErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Construct a not-found error for `job_id`.
    #[must_use]
    pub fn not_found(job_id: Uuid) -> Self {
        Self::new(QueueErrorKind::NotFound, format!("job not found: {job_id}"))
    }

    /// Construct a generic internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(QueueErrorKind::Internal, message)
    }
}
