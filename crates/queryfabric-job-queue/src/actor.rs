//! Thespis actor that drives dispatch, cancellation, recovery, and cleanup.

use std::fmt::Display;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use crate::priority::{CancelOutcome, PriorityRunner};
use futures::FutureExt;
use thespis::Actor;
use thespis::actor::{ActorRef, Spawn};
use thespis::error::Infallible;
use thespis::message::{Context, Message};
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::observer::{JobQueueObserver, NoopObserver};
use crate::schema::JobStatus;
use crate::traits::{JobExecutor, JobStorage, ResultStore};

fn spawn_traced(name: &'static str, future: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(async move {
        if AssertUnwindSafe(future).catch_unwind().await.is_err() {
            tracing::error!(task = name, "background task panicked");
        }
    });
}

/// Message that enqueues one job into the actor's priority runner.
#[derive(Debug)]
pub struct SubmitJob<K> {
    /// Job id to enqueue.
    pub job_id: Uuid,
    /// Dispatch priority where lower numbers run first.
    pub priority: i16,
    /// Domain-specific job kind.
    pub job_kind: K,
}

/// Message requesting that the actor dispatch the next runnable job.
#[derive(Debug)]
pub struct DispatchNext;

/// Message requesting cancellation of one job.
#[derive(Debug)]
pub struct CancelJob {
    /// Job id to cancel.
    pub job_id: Uuid,
}

/// Message requesting cleanup of expired jobs and result objects.
#[derive(Debug)]
pub struct CleanupExpired;

/// Construction arguments for [`JobQueueActor`].
pub struct JobQueueArgs<S, E, R, O = NoopObserver> {
    /// Persistent job storage implementation.
    pub storage: S,
    /// Domain-specific executor implementation.
    pub executor: E,
    /// Result-object store implementation.
    pub result_store: R,
    /// Concurrency limiter shared by all workers.
    pub semaphore: Arc<Semaphore>,
    /// Period between cleanup sweeps.
    pub cleanup_interval: Duration,
    /// Error string used when recovering stale running jobs after restart.
    pub restart_error: String,
    /// Optional observer for metrics and side effects.
    pub observer: O,
}

/// Generic thespis actor that manages queued jobs.
pub struct JobQueueActor<K, P, S, E, R, U = Uuid, O = NoopObserver>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    storage: S,
    executor: E,
    result_store: R,
    restart_error: String,
    observer: O,
    runner: PriorityRunner<K>,
    _phantom: std::marker::PhantomData<(P, U)>,
}

impl<K, P, S, E, R> JobQueueActor<K, P, S, E, R>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, Uuid>,
    E: JobExecutor<K, P, R, Uuid>,
    R: ResultStore,
{
    /// Spawn the actor with the default [`NoopObserver`].
    pub async fn spawn_and_start(args: JobQueueArgs<S, E, R>) -> ActorRef<Self> {
        Self::spawn_and_start_with_observer(args).await
    }
}

impl<K, P, S, E, R, U, O> JobQueueActor<K, P, S, E, R, U, O>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    /// Spawn the actor and start its periodic cleanup loop.
    pub async fn spawn_and_start_with_observer(args: JobQueueArgs<S, E, R, O>) -> ActorRef<Self> {
        let cleanup_interval = args.cleanup_interval;
        let actor_ref = Self::spawn(args);

        let ref_clone = actor_ref.clone();
        spawn_traced("job-cleanup-ticker", async move {
            let mut tick = tokio::time::interval(cleanup_interval);
            loop {
                tick.tick().await;
                if ref_clone.tell(CleanupExpired).send().await.is_err() {
                    break;
                }
            }
        });

        actor_ref
    }

    async fn refresh_job_state_metrics(&self, kind: &K) {
        self.observer
            .queue_state(
                kind,
                self.runner.pending_count(kind),
                self.runner.running_count(kind),
            )
            .await;
    }
}

impl<K, P, S, E, R, U, O> Actor for JobQueueActor<K, P, S, E, R, U, O>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    type Args = JobQueueArgs<S, E, R, O>;
    type Error = Infallible;

    async fn on_start(args: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        info!("JobQueueActor started");

        let actor = Self {
            storage: args.storage,
            executor: args.executor,
            result_store: args.result_store,
            restart_error: args.restart_error,
            observer: args.observer,
            runner: PriorityRunner::new(args.semaphore),
            _phantom: std::marker::PhantomData,
        };

        if let Err(e) = actor
            .storage
            .mark_stale_running_failed(actor.restart_error.clone())
            .await
        {
            warn!(error = %e, "Failed to mark stale running jobs as failed");
        }

        match actor.storage.pending_jobs().await {
            Ok(pending_jobs) => {
                let count = pending_jobs.len();
                for (job_id, priority, job_kind) in pending_jobs {
                    let _ = actor_ref
                        .tell(SubmitJob {
                            job_id,
                            priority,
                            job_kind,
                        })
                        .send()
                        .await;
                }
                if count > 0 {
                    info!(count, "Re-queued pending jobs from previous run");
                }
            }
            Err(e) => warn!(error = %e, "Failed to load pending jobs for recovery"),
        }

        Ok(actor)
    }
}

impl<K, P, S, E, R, U, O> Message<SubmitJob<K>> for JobQueueActor<K, P, S, E, R, U, O>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    type Reply = ();

    async fn handle(
        &mut self,
        msg: SubmitJob<K>,
        ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.observer.enqueued(&msg.job_kind, msg.priority).await;
        self.runner
            .submit(msg.job_id, msg.priority, msg.job_kind.clone());
        self.refresh_job_state_metrics(&msg.job_kind).await;
        debug!(job_id = %msg.job_id, priority = msg.priority, "Job enqueued");

        let _ = ctx.actor_ref().tell(DispatchNext).send().await;
    }
}

impl<K, P, S, E, R, U, O> Message<DispatchNext> for JobQueueActor<K, P, S, E, R, U, O>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    type Reply = ();

    async fn handle(
        &mut self,
        _msg: DispatchNext,
        ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let dispatch = match self.runner.try_dispatch() {
            Some(dispatch) => dispatch,
            None => return,
        };

        let job_id = dispatch.job_id;
        let job_kind = dispatch.kind.clone();
        self.refresh_job_state_metrics(&job_kind).await;

        let cancel_token = dispatch.cancel;
        let permit = dispatch.permit;
        let storage = self.storage.clone();
        let executor = self.executor.clone();
        let result_store = self.result_store.clone();
        let actor_ref = ctx.actor_ref().clone();
        let runner_handle = self.runner.handle();
        let observer = self.observer.clone();

        spawn_traced("job-runner", async move {
            let _permit = permit;
            let started = std::time::Instant::now();

            let result = async {
                storage.mark_running(job_id).await?;
                observer.event(&job_kind, JobStatus::Running).await;
                let job = storage
                    .load_job(job_id)
                    .await?
                    .ok_or_else(|| crate::JobQueueError::not_found(job_id))?;
                let execution = executor.execute(job, &result_store, &cancel_token).await?;
                let status = execution.status;
                let byte_count = execution.byte_count;
                storage.complete_job(job_id, execution).await?;
                observer.event(&job_kind, status).await;
                observer
                    .duration(&job_kind, status, started.elapsed().as_secs_f64())
                    .await;
                if let Some(byte_count) = byte_count {
                    observer
                        .result_bytes(&job_kind, status, byte_count as u64)
                        .await;
                }
                Ok::<(), crate::JobQueueError>(())
            }
            .await;

            runner_handle.finish(job_id);

            if let Err(e) = result {
                let _ = storage.fail_job(job_id, e.to_string()).await;
                observer.event(&job_kind, JobStatus::Failed).await;
                observer
                    .duration(
                        &job_kind,
                        JobStatus::Failed,
                        started.elapsed().as_secs_f64(),
                    )
                    .await;
                warn!(job_id = %job_id, error = %e, "Job failed");
            } else {
                debug!(job_id = %job_id, "Job completed successfully");
            }

            let _ = actor_ref.tell(DispatchNext).send().await;
        });
    }
}

impl<K, P, S, E, R, U, O> Message<CancelJob> for JobQueueActor<K, P, S, E, R, U, O>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    type Reply = Result<(), crate::JobQueueError>;

    async fn handle(
        &mut self,
        msg: CancelJob,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let kind_for_metrics = match self.runner.cancel(msg.job_id) {
            CancelOutcome::SignalledRunning | CancelOutcome::RemovedFromQueue => {
                self.storage.load_job(msg.job_id).await?.map(|job| job.kind)
            }
            CancelOutcome::NotFound => self.storage.load_job(msg.job_id).await?.map(|job| job.kind),
        };

        self.storage.cancel_job(msg.job_id).await?;
        if let Some(kind) = kind_for_metrics {
            self.observer.event(&kind, JobStatus::Cancelled).await;
            self.refresh_job_state_metrics(&kind).await;
        }
        info!(job_id = %msg.job_id, "Job cancelled");
        Ok(())
    }
}

impl<K, P, S, E, R, U, O> Message<CleanupExpired> for JobQueueActor<K, P, S, E, R, U, O>
where
    K: Clone + Eq + Display + Send + Sync + 'static,
    P: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
    S: JobStorage<K, P, U>,
    E: JobExecutor<K, P, R, U>,
    R: ResultStore,
    O: JobQueueObserver<K>,
{
    type Reply = ();

    async fn handle(
        &mut self,
        _msg: CleanupExpired,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let jobs = match self.storage.expired_jobs().await {
            Ok(jobs) => jobs,
            Err(e) => {
                warn!(error = %e, "Failed to query expired jobs");
                return;
            }
        };

        let mut cleaned = 0usize;
        for job in jobs {
            if let Some(path) = job.result_path
                && let Err(e) = self.result_store.delete(&path).await
            {
                warn!(job_id = %job.job_id, error = %e, "Failed to delete expired result");
            }

            if let Err(e) = self.storage.delete_job(job.job_id).await {
                warn!(job_id = %job.job_id, error = %e, "Failed to delete expired job row");
            } else {
                cleaned += 1;
            }
        }

        if cleaned > 0 {
            info!(cleaned, "Cleaned up expired jobs");
        }
    }
}
