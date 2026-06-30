//! One-shot Arrow Flight worker for isolated query execution.
//!
//! Provides the [`WorkerFlightService`] which implements the Flight gRPC
//! protocol (DoGet-only) and shuts down after serving exactly one request.
//! The backend query engine is abstracted behind the [`QueryExecutor`] trait.

use std::pin::Pin;
use std::sync::Arc;

use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::flight_service_server::FlightService;
use arrow_flight::{
    Action, Criteria, Empty, FlightData, FlightDescriptor, FlightInfo, HandshakeRequest,
    HandshakeResponse, PollInfo, PutResult, SchemaResult, Ticket,
};
use async_trait::async_trait;
use futures::stream::StreamExt;
use queryfabric::{IsolatedJobSpec, RecordBatchStream};
use std::sync::Mutex;
use thiserror::Error;
use tokio::sync::oneshot;
use tonic::{Request, Response, Status, Streaming};

/// Errors from worker operations.
#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("Missing environment variable: {0}")]
    MissingEnv(&'static str),
    #[error("gRPC error: {0}")]
    Grpc(#[from] Status),
}

/// Abstract query execution backend.
///
/// Implement this trait for your query engine (ClickHouse, DuckDB, etc.).
#[async_trait]
pub trait QueryExecutor: Send + Sync {
    /// Execute the spec's query and return a stream of record batches.
    async fn execute(&self, spec: &IsolatedJobSpec) -> Result<RecordBatchStream, WorkerError>;
}

type BoxStream<T> = Pin<Box<dyn futures::Stream<Item = Result<T, Status>> + Send>>;

/// One-shot Arrow Flight service that serves a single DoGet request.
///
/// After serving one request, the worker signals shutdown via the provided
/// `oneshot::Sender`.
pub struct WorkerFlightService {
    spec: IsolatedJobSpec,
    executor: Arc<dyn QueryExecutor>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

impl WorkerFlightService {
    pub fn new(
        spec: IsolatedJobSpec,
        executor: Arc<dyn QueryExecutor>,
        shutdown: oneshot::Sender<()>,
    ) -> Self {
        Self {
            spec,
            executor,
            shutdown: Mutex::new(Some(shutdown)),
        }
    }
}

#[tonic::async_trait]
impl FlightService for WorkerFlightService {
    type HandshakeStream = BoxStream<HandshakeResponse>;
    type ListFlightsStream = BoxStream<FlightInfo>;
    type DoGetStream = BoxStream<FlightData>;
    type DoPutStream = BoxStream<PutResult>;
    type DoActionStream = BoxStream<arrow_flight::Result>;
    type ListActionsStream = BoxStream<arrow_flight::ActionType>;
    type DoExchangeStream = BoxStream<FlightData>;

    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        Err(Status::unimplemented(
            "handshake not supported — use Bearer token",
        ))
    }

    async fn list_flights(
        &self,
        _request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        Err(Status::unimplemented("list_flights"))
    }

    async fn get_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        Err(Status::unimplemented("get_flight_info"))
    }

    async fn poll_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<PollInfo>, Status> {
        Err(Status::unimplemented("poll_flight_info"))
    }

    async fn get_schema(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        Err(Status::unimplemented("get_schema"))
    }

    async fn do_get(
        &self,
        request: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        let ticket = request.into_inner();

        // Validate the ticket matches our spec's provenance hash.
        let ticket_hash = String::from_utf8(ticket.ticket.to_vec())
            .map_err(|_| Status::invalid_argument("ticket must be UTF-8 query hash"))?;
        let spec_hash = &self.spec.query.provenance().query_hash;
        if ticket_hash != *spec_hash {
            return Err(Status::permission_denied("ticket hash mismatch"));
        }

        let stream = self
            .executor
            .execute(&self.spec)
            .await
            .map_err(|e| Status::internal(format!("execution failed: {e}")))?;

        let flight_data = FlightDataEncoderBuilder::new()
            .build(stream.map(|batch| {
                batch.map_err(|e| arrow_flight::error::FlightError::ExternalError(Box::new(e)))
            }))
            .map(|data| data.map_err(Status::from));

        // Signal shutdown after serving.
        let mut shutdown = self
            .shutdown
            .lock()
            .map_err(|_| Status::internal("worker shutdown mutex poisoned"))?;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(());
        }

        Ok(Response::new(Box::pin(flight_data)))
    }

    async fn do_put(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        Err(Status::unimplemented("do_put"))
    }

    async fn do_action(
        &self,
        _request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        Err(Status::unimplemented("do_action"))
    }

    async fn list_actions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        Err(Status::unimplemented("list_actions"))
    }

    async fn do_exchange(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        Err(Status::unimplemented("do_exchange"))
    }
}
