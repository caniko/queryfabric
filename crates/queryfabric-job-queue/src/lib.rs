//! Generic piying-backed job queue orchestration.
//!
//! This crate owns reusable queue behavior: priority dispatch, cancellation,
//! startup recovery, result cleanup, storage/executor boundaries, and a small
//! generic HTTP schema surface. Domain-specific persistence and execution are
//! supplied through traits.

#![warn(missing_docs)]

mod actor;
mod error;
mod observer;
pub mod priority;
mod router;
mod schema;
mod storage;
mod traits;

pub use actor::{CancelJob, CleanupExpired, DispatchNext, JobQueueActor, JobQueueArgs, SubmitJob};
pub use error::{JobQueueError, QueueErrorKind};
pub use observer::{JobQueueObserver, NoopObserver};
pub use router::{JobApi, routes};
pub use schema::{
    JobId, JobPriority, JobRecord, JobResultManifest, JobStatus, JobStatusResponse, ListJobsQuery,
    ListJobsResponse, ResultContent, SubmitJobResponse,
};
pub use storage::ExpiredJob;
pub use traits::{JobExecutionResult, JobExecutor, JobStorage, ResultStore};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use chrono::Utc;
    use piying::actor::ActorRef;
    use tokio::sync::{Mutex, Semaphore};
    use uuid::Uuid;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Kind {
        Query,
    }

    impl std::fmt::Display for Kind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Kind::Query => f.write_str("query"),
            }
        }
    }

    type Record = JobRecord<Kind, serde_json::Value>;

    #[derive(Clone, Default)]
    struct FakeStorage {
        jobs: Arc<Mutex<HashMap<Uuid, Record>>>,
        manifests: Arc<Mutex<HashMap<Uuid, JobResultManifest>>>,
        stale_marked: Arc<Mutex<usize>>,
    }

    impl FakeStorage {
        async fn insert(&self, status: JobStatus, priority: i16) -> Uuid {
            let id = Uuid::now_v7();
            self.jobs.lock().await.insert(
                id,
                Record {
                    id,
                    user_id: Uuid::now_v7(),
                    status,
                    priority,
                    kind: Kind::Query,
                    payload: serde_json::json!({}),
                    result_path: None,
                    row_count: None,
                    byte_count: None,
                    error: None,
                    created_at: Utc::now(),
                    started_at: None,
                    completed_at: None,
                    expires_at: None,
                },
            );
            id
        }

        async fn status(&self, id: Uuid) -> JobStatus {
            self.jobs.lock().await.get(&id).unwrap().status
        }
    }

    #[async_trait]
    impl JobStorage<Kind, serde_json::Value> for FakeStorage {
        async fn load_job(&self, job_id: Uuid) -> Result<Option<Record>, JobQueueError> {
            Ok(self.jobs.lock().await.get(&job_id).cloned())
        }

        async fn insert_pending(&self, job: Record) -> Result<(), JobQueueError> {
            self.jobs.lock().await.insert(job.id, job);
            Ok(())
        }

        async fn mark_running(&self, job_id: Uuid) -> Result<(), JobQueueError> {
            let mut jobs = self.jobs.lock().await;
            let job = jobs
                .get_mut(&job_id)
                .ok_or_else(|| JobQueueError::not_found(job_id))?;
            job.status = JobStatus::Running;
            job.started_at = Some(Utc::now());
            Ok(())
        }

        async fn complete_job(
            &self,
            job_id: Uuid,
            result: JobExecutionResult,
        ) -> Result<(), JobQueueError> {
            let mut jobs = self.jobs.lock().await;
            let job = jobs
                .get_mut(&job_id)
                .ok_or_else(|| JobQueueError::not_found(job_id))?;
            job.status = result.status;
            job.result_path = result.result_path;
            job.row_count = result.row_count;
            job.byte_count = result.byte_count;
            job.error = result.error;
            job.completed_at = Some(Utc::now());
            Ok(())
        }

        async fn fail_job(&self, job_id: Uuid, error: String) -> Result<(), JobQueueError> {
            let mut jobs = self.jobs.lock().await;
            let job = jobs
                .get_mut(&job_id)
                .ok_or_else(|| JobQueueError::not_found(job_id))?;
            job.status = JobStatus::Failed;
            job.error = Some(error);
            Ok(())
        }

        async fn cancel_job(&self, job_id: Uuid) -> Result<(), JobQueueError> {
            let mut jobs = self.jobs.lock().await;
            let job = jobs
                .get_mut(&job_id)
                .ok_or_else(|| JobQueueError::not_found(job_id))?;
            job.status = JobStatus::Cancelled;
            Ok(())
        }

        async fn pending_jobs(&self) -> Result<Vec<(Uuid, JobPriority, Kind)>, JobQueueError> {
            let mut jobs: Vec<_> = self
                .jobs
                .lock()
                .await
                .values()
                .filter(|job| job.status == JobStatus::Pending)
                .map(|job| (job.id, job.priority, job.kind))
                .collect();
            jobs.sort_by_key(|(_, priority, _)| *priority);
            Ok(jobs)
        }

        async fn mark_stale_running_failed(&self, error: String) -> Result<usize, JobQueueError> {
            let mut count = 0;
            for job in self.jobs.lock().await.values_mut() {
                if job.status == JobStatus::Running {
                    job.status = JobStatus::Failed;
                    job.error = Some(error.clone());
                    count += 1;
                }
            }
            *self.stale_marked.lock().await = count;
            Ok(count)
        }

        async fn expired_jobs(&self) -> Result<Vec<ExpiredJob>, JobQueueError> {
            Ok(self
                .jobs
                .lock()
                .await
                .values()
                .filter(|job| {
                    matches!(
                        job.status,
                        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                    ) && job.expires_at.is_some_and(|expires| expires <= Utc::now())
                })
                .map(|job| ExpiredJob {
                    job_id: job.id,
                    result_path: job.result_path.clone(),
                })
                .collect())
        }

        async fn delete_job(&self, job_id: Uuid) -> Result<(), JobQueueError> {
            self.jobs.lock().await.remove(&job_id);
            Ok(())
        }

        async fn result_manifest(
            &self,
            job_id: Uuid,
            _user_id: Uuid,
        ) -> Result<JobResultManifest, JobQueueError> {
            self.manifests
                .lock()
                .await
                .get(&job_id)
                .cloned()
                .ok_or_else(|| {
                    JobQueueError::new(
                        QueueErrorKind::ResultMissing,
                        format!("manifest missing for {job_id}"),
                    )
                })
        }
    }

    #[derive(Clone, Default)]
    struct FakeResultStore {
        objects: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        deleted: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ResultStore for FakeResultStore {
        async fn write(&self, path: &str, bytes: Vec<u8>) -> Result<(), JobQueueError> {
            self.objects.lock().await.insert(path.to_owned(), bytes);
            Ok(())
        }

        async fn read(&self, path: &str) -> Result<Vec<u8>, JobQueueError> {
            self.objects
                .lock()
                .await
                .get(path)
                .cloned()
                .ok_or_else(|| JobQueueError::new(QueueErrorKind::ResultMissing, path.to_owned()))
        }

        async fn delete(&self, path: &str) -> Result<(), JobQueueError> {
            self.objects.lock().await.remove(path);
            self.deleted.lock().await.push(path.to_owned());
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct FakeExecutor {
        executed: Arc<Mutex<Vec<Uuid>>>,
    }

    #[async_trait]
    impl JobExecutor<Kind, serde_json::Value, FakeResultStore> for FakeExecutor {
        async fn execute(
            &self,
            job: Record,
            result_store: &FakeResultStore,
            cancel_token: &tokio_util::sync::CancellationToken,
        ) -> Result<JobExecutionResult, JobQueueError> {
            self.executed.lock().await.push(job.id);
            if cancel_token.is_cancelled() {
                return Ok(JobExecutionResult {
                    status: JobStatus::Cancelled,
                    result_path: None,
                    row_count: None,
                    byte_count: None,
                    error: None,
                });
            }
            let path = format!("results/{}.json", job.id);
            result_store.write(&path, b"ok".to_vec()).await?;
            Ok(JobExecutionResult::completed(Some(path), Some(1), Some(2)))
        }

        async fn result_content(
            &self,
            job: Record,
            result_store: &FakeResultStore,
        ) -> Result<ResultContent, JobQueueError> {
            let path = format!("results/{}.json", job.id);
            Ok(ResultContent {
                bytes: result_store.read(&path).await?,
                content_type: "application/json".to_owned(),
                filename: format!("{}.json", job.id),
            })
        }
    }

    async fn actor(
        storage: FakeStorage,
        executor: FakeExecutor,
        result_store: FakeResultStore,
        permits: usize,
    ) -> ActorRef<JobQueueActor<Kind, serde_json::Value, FakeStorage, FakeExecutor, FakeResultStore>>
    {
        JobQueueActor::spawn_and_start(JobQueueArgs {
            storage,
            executor,
            result_store,
            semaphore: Arc::new(Semaphore::new(permits)),
            cleanup_interval: Duration::from_millis(10_000),
            restart_error: "restart".to_owned(),
            observer: NoopObserver,
        })
        .await
    }

    #[tokio::test]
    async fn startup_marks_running_failed_and_requeues_pending() {
        let storage = FakeStorage::default();
        let running = storage.insert(JobStatus::Running, 1).await;
        let pending = storage.insert(JobStatus::Pending, 1).await;
        let executor = FakeExecutor::default();
        let result_store = FakeResultStore::default();
        let _actor = actor(storage.clone(), executor.clone(), result_store, 1).await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(storage.status(running).await, JobStatus::Failed);
        assert_eq!(storage.status(pending).await, JobStatus::Completed);
        assert_eq!(*storage.stale_marked.lock().await, 1);
        assert_eq!(executor.executed.lock().await.as_slice(), &[pending]);
    }

    #[tokio::test]
    async fn cancellation_marks_pending_job_cancelled() {
        let storage = FakeStorage::default();
        let pending = storage.insert(JobStatus::Pending, 1).await;
        let queue = actor(
            storage.clone(),
            FakeExecutor::default(),
            FakeResultStore::default(),
            0,
        )
        .await;

        queue
            .ask(CancelJob { job_id: pending })
            .send()
            .await
            .expect("cancel succeeds");

        assert_eq!(storage.status(pending).await, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn cleanup_deletes_expired_results_and_rows() {
        let storage = FakeStorage::default();
        let result_store = FakeResultStore::default();
        let expired = storage.insert(JobStatus::Completed, 1).await;
        {
            let mut jobs = storage.jobs.lock().await;
            let job = jobs.get_mut(&expired).unwrap();
            job.result_path = Some("results/old.json".to_owned());
            job.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        }
        result_store
            .write("results/old.json", b"old".to_vec())
            .await
            .unwrap();

        let queue = actor(
            storage.clone(),
            FakeExecutor::default(),
            result_store.clone(),
            0,
        )
        .await;
        queue
            .tell(CleanupExpired)
            .send()
            .await
            .expect("cleanup tell");

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(!storage.jobs.lock().await.contains_key(&expired));
        assert_eq!(
            result_store.deleted.lock().await.as_slice(),
            &["results/old.json".to_owned()]
        );
    }

    #[tokio::test]
    async fn result_manifest_returns_stored_manifest() {
        let storage = FakeStorage::default();
        let job_id = storage.insert(JobStatus::Completed, 1).await;
        let manifest = JobResultManifest {
            id: Uuid::now_v7(),
            job_id: Some(job_id),
            storage_uri: "results/x.json".to_owned(),
            format: "json".to_owned(),
            row_count: 1,
            byte_count: 2,
            page_count: 1,
            content_hash: None,
            query_cost: serde_json::json!({}),
            manifest_json: serde_json::json!({"version": 1}),
            expires_at: None,
            created_at: Utc::now(),
        };
        storage
            .manifests
            .lock()
            .await
            .insert(job_id, manifest.clone());

        let got = storage
            .result_manifest(job_id, Uuid::now_v7())
            .await
            .unwrap();
        assert_eq!(got, manifest);
    }
}
