//! Axum router helpers for job-queue APIs.

use async_trait::async_trait;
use axum::Json;
use axum::body::Body;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Serialize;
use serde::de::DeserializeOwned;
use utoipa_axum::router::OpenApiRouter;
use uuid::Uuid;

use crate::error::{JobQueueError, QueueErrorKind};
use crate::schema::{JobResultManifest, ListJobsQuery, ListJobsResponse, ResultContent};

/// Application-specific HTTP adapter over the generic queue routes.
#[async_trait]
pub trait JobApi<StateT>: Clone + Send + Sync + 'static {
    /// Request context extracted from the HTTP request.
    type Context: FromRequestParts<StateT> + Send + Sync + 'static;
    /// JSON payload accepted by the submit endpoint.
    type SubmitRequest: DeserializeOwned + Send + Sync + 'static;
    /// JSON payload returned by the submit endpoint.
    type SubmitResponse: Serialize + Send + Sync + 'static;
    /// JSON payload returned by list/status endpoints.
    type StatusResponse: Serialize + Send + Sync + 'static;

    /// Submit a new job.
    async fn submit(
        state: StateT,
        context: Self::Context,
        request: Self::SubmitRequest,
    ) -> Result<Self::SubmitResponse, JobQueueError>;
    /// List jobs visible to the caller.
    async fn list(
        state: StateT,
        context: Self::Context,
        query: ListJobsQuery,
    ) -> Result<ListJobsResponse<Self::StatusResponse>, JobQueueError>;
    /// Return current status for one job id.
    async fn status(
        state: StateT,
        context: Self::Context,
        job_id: Uuid,
    ) -> Result<Self::StatusResponse, JobQueueError>;
    /// Cancel one job id.
    async fn cancel(
        state: StateT,
        context: Self::Context,
        job_id: Uuid,
    ) -> Result<(), JobQueueError>;
    /// Return the result manifest for one job id.
    async fn manifest(
        state: StateT,
        context: Self::Context,
        job_id: Uuid,
    ) -> Result<JobResultManifest, JobQueueError>;
    /// Return downloadable result bytes for one job id.
    async fn result(
        state: StateT,
        context: Self::Context,
        job_id: Uuid,
    ) -> Result<ResultContent, JobQueueError>;
}

/// Build the standard Axum/OpenAPI routes for a `JobApi` implementation.
#[must_use]
pub fn routes<StateT, A>() -> OpenApiRouter<StateT>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
{
    OpenApiRouter::new()
        .route("/", post(submit::<StateT, A>).get(list::<StateT, A>))
        .route(
            "/{job_id}",
            get(status::<StateT, A>).delete(cancel::<StateT, A>),
        )
        .route("/{job_id}/result-manifest", get(manifest::<StateT, A>))
        .route("/{job_id}/result", get(result::<StateT, A>))
}

async fn submit<StateT, A>(
    State(state): State<StateT>,
    context: A::Context,
    Json(body): Json<A::SubmitRequest>,
) -> Result<(StatusCode, Json<A::SubmitResponse>), JobQueueError>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
    <A::Context as FromRequestParts<StateT>>::Rejection: IntoResponse,
{
    let response = A::submit(state, context, body).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn list<StateT, A>(
    State(state): State<StateT>,
    context: A::Context,
    Query(query): Query<ListJobsQuery>,
) -> Result<Json<ListJobsResponse<A::StatusResponse>>, JobQueueError>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
    <A::Context as FromRequestParts<StateT>>::Rejection: IntoResponse,
{
    Ok(Json(A::list(state, context, query).await?))
}

async fn status<StateT, A>(
    State(state): State<StateT>,
    context: A::Context,
    Path(job_id): Path<Uuid>,
) -> Result<Json<A::StatusResponse>, JobQueueError>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
    <A::Context as FromRequestParts<StateT>>::Rejection: IntoResponse,
{
    Ok(Json(A::status(state, context, job_id).await?))
}

async fn cancel<StateT, A>(
    State(state): State<StateT>,
    context: A::Context,
    Path(job_id): Path<Uuid>,
) -> Result<StatusCode, JobQueueError>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
    <A::Context as FromRequestParts<StateT>>::Rejection: IntoResponse,
{
    A::cancel(state, context, job_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn manifest<StateT, A>(
    State(state): State<StateT>,
    context: A::Context,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobResultManifest>, JobQueueError>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
    <A::Context as FromRequestParts<StateT>>::Rejection: IntoResponse,
{
    Ok(Json(A::manifest(state, context, job_id).await?))
}

async fn result<StateT, A>(
    State(state): State<StateT>,
    context: A::Context,
    Path(job_id): Path<Uuid>,
) -> Result<Response, JobQueueError>
where
    StateT: Clone + Send + Sync + 'static,
    A: JobApi<StateT>,
    <A::Context as FromRequestParts<StateT>>::Rejection: IntoResponse,
{
    let content = A::result(state, context, job_id).await?;
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content.content_type)
        .header(
            "content-disposition",
            format!("attachment; filename=\"{}\"", content.filename),
        )
        .body(Body::from(content.bytes))
        .map_err(|e| JobQueueError::internal(format!("failed to build result response: {e}")))
}

impl IntoResponse for JobQueueError {
    fn into_response(self) -> Response {
        let status = match self.kind {
            QueueErrorKind::NotFound | QueueErrorKind::ResultMissing => StatusCode::NOT_FOUND,
            QueueErrorKind::Forbidden => StatusCode::FORBIDDEN,
            QueueErrorKind::NotCancellable | QueueErrorKind::NotReady => StatusCode::CONFLICT,
            QueueErrorKind::InvalidRequest => StatusCode::BAD_REQUEST,
            QueueErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            QueueErrorKind::Storage | QueueErrorKind::Execution | QueueErrorKind::Internal => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let body = serde_json::json!({
            "error": self.message,
            "status": status.as_u16(),
        });
        (status, Json(body)).into_response()
    }
}
