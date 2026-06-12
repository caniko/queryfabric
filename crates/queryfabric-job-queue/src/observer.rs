//! Optional observer hooks for metrics and side effects.

use std::fmt::Display;

use async_trait::async_trait;

use crate::schema::{JobPriority, JobStatus};

/// Observer callbacks for queue metrics and side effects.
#[async_trait]
pub trait JobQueueObserver<K>: Clone + Send + Sync + 'static
where
    K: Display + Send + Sync,
{
    /// Observe a job entering a lifecycle state.
    async fn event(&self, _kind: &K, _status: JobStatus) {}
    /// Observe current pending/running counts for a job kind.
    async fn queue_state(&self, _kind: &K, _pending: usize, _running: usize) {}
    /// Observe a newly enqueued job.
    async fn enqueued(&self, kind: &K, _priority: JobPriority) {
        self.event(kind, JobStatus::Pending).await;
    }
    /// Observe a terminal or running duration in seconds.
    async fn duration(&self, _kind: &K, _status: JobStatus, _seconds: f64) {}
    /// Observe produced result bytes for one job.
    async fn result_bytes(&self, _kind: &K, _status: JobStatus, _bytes: u64) {}
}

/// Observer implementation that ignores every callback.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopObserver;

#[async_trait]
impl<K> JobQueueObserver<K> for NoopObserver where K: Display + Send + Sync {}
