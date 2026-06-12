//! Composable priority job dispatcher.
//!
//! Owns the three mechanics that recur in every job-queue actor: a priority
//! [`BinaryHeap`] with FIFO tie-breaking, a [`Semaphore`] cap on concurrent
//! workers, and a [`CancellationToken`] per running job. Storage, persistence,
//! and execution are left to the caller.
//!
//! Lower `priority` values dispatch first. Equal priorities break by insertion
//! order (older first).

#![warn(missing_docs)]

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq)]
struct Entry<K: Eq> {
    job_id: Uuid,
    priority: i16,
    seq: u64,
    kind: K,
}

impl<K: Eq> Ord for Entry<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; we want lowest priority first, so reverse.
        // FIFO tie-break: older (lower seq) first.
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl<K: Eq> PartialOrd for Entry<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// What [`PriorityRunner::cancel`] did with the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// Job was queued but not yet running; removed from the priority queue.
    RemovedFromQueue,
    /// Job was running; its cancellation token was signalled.
    SignalledRunning,
    /// No pending or running job with that id.
    NotFound,
}

/// A successful dispatch: the next job to run, with the permit and token the
/// caller must associate with the spawned worker task.
#[derive(Debug)]
pub struct Dispatch<K> {
    /// Unique job identifier.
    pub job_id: Uuid,
    /// Domain-specific job kind.
    pub kind: K,
    /// Semaphore permit that must be held for the worker lifetime.
    pub permit: OwnedSemaphorePermit,
    /// Cancellation token shared with the worker.
    pub cancel: CancellationToken,
}

/// Priority-aware queue plus running-job cancellation bookkeeping.
#[derive(Debug)]
pub struct PriorityRunner<K: Clone + Eq + Send + Sync + 'static> {
    semaphore: Arc<Semaphore>,
    pending: BinaryHeap<Entry<K>>,
    seq: u64,
    running: Arc<DashMap<Uuid, (CancellationToken, K)>>,
}

impl<K: Clone + Eq + Send + Sync + 'static> PriorityRunner<K> {
    /// Create an empty runner that is limited by the provided semaphore.
    #[must_use]
    pub fn new(semaphore: Arc<Semaphore>) -> Self {
        Self {
            semaphore,
            pending: BinaryHeap::new(),
            seq: 0,
            running: Arc::new(DashMap::new()),
        }
    }

    /// Enqueue a job. Lower `priority` values dispatch first.
    pub fn submit(&mut self, job_id: Uuid, priority: i16, kind: K) {
        self.seq += 1;
        self.pending.push(Entry {
            job_id,
            priority,
            seq: self.seq,
            kind,
        });
    }

    /// Try to acquire a worker permit and pop the highest-priority pending job.
    /// Returns `None` if the semaphore is saturated or no jobs are pending.
    /// Registers a fresh cancellation token in the running map.
    #[must_use]
    pub fn try_dispatch(&mut self) -> Option<Dispatch<K>> {
        let permit = self.semaphore.clone().try_acquire_owned().ok()?;
        let entry = self.pending.pop()?;
        let cancel = CancellationToken::new();
        self.running
            .insert(entry.job_id, (cancel.clone(), entry.kind.clone()));
        Some(Dispatch {
            job_id: entry.job_id,
            kind: entry.kind,
            permit,
            cancel,
        })
    }

    /// Cancel a job. If it was queued, remove it; if running, signal its token.
    #[must_use]
    pub fn cancel(&mut self, job_id: Uuid) -> CancelOutcome {
        if let Some((_, (token, _))) = self.running.remove(&job_id) {
            token.cancel();
            return CancelOutcome::SignalledRunning;
        }

        let drained: Vec<Entry<K>> = self.pending.drain().collect();
        let mut found = false;
        for entry in drained {
            if entry.job_id == job_id {
                found = true;
            } else {
                self.pending.push(entry);
            }
        }
        if found {
            CancelOutcome::RemovedFromQueue
        } else {
            CancelOutcome::NotFound
        }
    }

    /// Mark a running job as finished (call from the worker task once
    /// `execute()` returns). Drops its cancellation token.
    pub fn finish(&self, job_id: Uuid) {
        self.running.remove(&job_id);
    }

    /// Cheap `Clone + Send + Sync` handle that can [`finish`](RunnerHandle::finish)
    /// jobs from a spawned worker task without holding a reference to the
    /// runner itself.
    #[must_use]
    pub fn handle(&self) -> RunnerHandle<K> {
        RunnerHandle {
            running: Arc::clone(&self.running),
        }
    }

    /// Number of pending jobs of a given kind. O(n) — intended for metrics
    /// refresh, not hot paths.
    #[must_use]
    pub fn pending_count(&self, kind: &K) -> usize {
        self.pending.iter().filter(|e| &e.kind == kind).count()
    }

    /// Number of currently running jobs of a given kind.
    #[must_use]
    pub fn running_count(&self, kind: &K) -> usize {
        self.running.iter().filter(|e| &e.value().1 == kind).count()
    }

    /// Total pending jobs across all kinds.
    #[must_use]
    pub fn total_pending(&self) -> usize {
        self.pending.len()
    }

    /// Total running jobs across all kinds.
    #[must_use]
    pub fn total_running(&self) -> usize {
        self.running.len()
    }
}

/// Cheap clone-into-spawned-task handle. Created via [`PriorityRunner::handle`].
#[derive(Debug)]
pub struct RunnerHandle<K: Clone + Eq + Send + Sync + 'static> {
    running: Arc<DashMap<Uuid, (CancellationToken, K)>>,
}

impl<K: Clone + Eq + Send + Sync + 'static> Clone for RunnerHandle<K> {
    fn clone(&self) -> Self {
        Self {
            running: Arc::clone(&self.running),
        }
    }
}

impl<K: Clone + Eq + Send + Sync + 'static> RunnerHandle<K> {
    /// Drop the cancellation token registered for this job.
    pub fn finish(&self, job_id: Uuid) {
        self.running.remove(&job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Eq, PartialEq, Debug)]
    enum Kind {
        A,
        B,
    }

    fn runner(permits: usize) -> PriorityRunner<Kind> {
        PriorityRunner::new(Arc::new(Semaphore::new(permits)))
    }

    #[test]
    fn dispatches_lower_priority_first() {
        let mut r = runner(8);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        r.submit(a, 10, Kind::A);
        r.submit(b, 1, Kind::A);
        r.submit(c, 5, Kind::A);

        assert_eq!(r.try_dispatch().unwrap().job_id, b);
        assert_eq!(r.try_dispatch().unwrap().job_id, c);
        assert_eq!(r.try_dispatch().unwrap().job_id, a);
    }

    #[test]
    fn fifo_tiebreak_on_equal_priority() {
        let mut r = runner(8);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        r.submit(a, 5, Kind::A);
        r.submit(b, 5, Kind::A);
        r.submit(c, 5, Kind::A);
        assert_eq!(r.try_dispatch().unwrap().job_id, a);
        assert_eq!(r.try_dispatch().unwrap().job_id, b);
        assert_eq!(r.try_dispatch().unwrap().job_id, c);
    }

    #[test]
    fn semaphore_caps_concurrency() {
        let mut r = runner(2);
        r.submit(Uuid::new_v4(), 0, Kind::A);
        r.submit(Uuid::new_v4(), 0, Kind::A);
        r.submit(Uuid::new_v4(), 0, Kind::A);
        let d1 = r.try_dispatch().expect("first");
        let _d2 = r.try_dispatch().expect("second");
        assert!(r.try_dispatch().is_none(), "should be saturated");
        // Releasing the first permit lets the third dispatch.
        let id1 = d1.job_id;
        r.finish(id1);
        drop(d1);
        let d3 = r.try_dispatch().expect("third after permit release");
        assert_eq!(r.total_running(), 2);
        let _ = d3;
    }

    #[test]
    fn cancel_pending() {
        let mut r = runner(0);
        let id = Uuid::new_v4();
        r.submit(id, 0, Kind::A);
        assert_eq!(r.cancel(id), CancelOutcome::RemovedFromQueue);
        assert_eq!(r.cancel(id), CancelOutcome::NotFound);
        assert_eq!(r.total_pending(), 0);
    }

    #[test]
    fn cancel_running_signals_token() {
        let mut r = runner(1);
        let id = Uuid::new_v4();
        r.submit(id, 0, Kind::A);
        let d = r.try_dispatch().unwrap();
        let token = d.cancel.clone();
        assert!(!token.is_cancelled());
        assert_eq!(r.cancel(id), CancelOutcome::SignalledRunning);
        assert!(token.is_cancelled());
    }

    #[test]
    fn counts_by_kind() {
        let mut r = runner(8);
        r.submit(Uuid::new_v4(), 0, Kind::A);
        r.submit(Uuid::new_v4(), 0, Kind::A);
        r.submit(Uuid::new_v4(), 0, Kind::B);
        assert_eq!(r.pending_count(&Kind::A), 2);
        assert_eq!(r.pending_count(&Kind::B), 1);
        let _d = r.try_dispatch().unwrap();
        assert_eq!(r.running_count(&Kind::A), 1);
        assert_eq!(r.pending_count(&Kind::A), 1);
    }
}
