//! Arrow Flight server skeleton with pluggable authentication and access
//! control.
//!
//! The skeleton owns the gRPC frame (`DoGet`/`DoPut`/`ListFlights`/
//! `GetFlightInfo`) and the auth/access policy enforcement order; the host
//! injects everything domain-specific as trait objects:
//!
//! - [`AuthN`] authenticates request metadata into a contract [`Subject`]
//!   (e.g. bearer-token validation). Stateful impls (key stores, DB handles)
//!   are supported via `Arc`.
//! - [`TicketInterpreter`] decodes opaque tickets/descriptors into the
//!   [`ResourceRef`] they target plus the [`AccessPolicy`] guarding it.
//! - [`AccessDecision`] (from `queryfabric-contract`) decides whether the
//!   subject may touch the resource. A denial maps to `PermissionDenied`.
//! - [`FlightHandlers`] supplies the data plane: record-batch streams for
//!   `DoGet`, flight listings, and uploads. Defaults return `Unimplemented`
//!   so hosts implement only what they serve.

use std::pin::Pin;
use std::sync::Arc;

use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::flight_service_server::FlightService;
use arrow_flight::{
    Action, ActionType, Criteria, Empty, FlightData, FlightDescriptor, FlightInfo,
    HandshakeRequest, HandshakeResponse, PollInfo, PutResult, SchemaResult, Ticket,
};
use async_trait::async_trait;
use futures::StreamExt;
use futures::stream;
use queryfabric_contract::{AccessDecision, AccessOutcome, AccessPolicy, ResourceRef, Subject};
use tonic::metadata::MetadataMap;
use tonic::{Request, Response, Status, Streaming};

use crate::RecordBatchStream;

/// Authenticates gRPC request metadata into a contract [`Subject`].
pub trait AuthN: Send + Sync {
    fn authenticate(&self, metadata: &MetadataMap) -> Result<Subject, Status>;
}

/// The resource a ticket or descriptor targets and the policy guarding it.
#[derive(Debug, Clone)]
pub struct TicketGrant {
    pub resource: ResourceRef,
    pub policy: AccessPolicy,
}

/// Decodes opaque Flight tickets/descriptors into [`TicketGrant`]s.
#[async_trait]
pub trait TicketInterpreter: Send + Sync {
    async fn interpret(&self, ticket: &[u8]) -> Result<TicketGrant, Status>;
}

/// Host-implemented data plane behind the skeleton's auth/access frame.
///
/// Every method receives the already-authenticated subject; `do_get`
/// additionally runs only after the access decision allowed the ticket's
/// resource. Defaults return `Unimplemented` so hosts opt into endpoints.
#[async_trait]
pub trait FlightHandlers: Send + Sync {
    /// Stream record batches for an authorized ticket.
    async fn do_get(&self, subject: &Subject, ticket: &[u8]) -> Result<RecordBatchStream, Status>;

    /// Describe a flight for a descriptor.
    async fn get_flight_info(
        &self,
        _subject: &Subject,
        _descriptor: &FlightDescriptor,
    ) -> Result<FlightInfo, Status> {
        Err(Status::unimplemented("get_flight_info"))
    }

    /// Enumerate available flights.
    async fn list_flights(
        &self,
        _subject: &Subject,
        _criteria: &[u8],
    ) -> Result<Vec<FlightInfo>, Status> {
        Err(Status::unimplemented("list_flights"))
    }

    /// Accept an upload stream. Access enforcement for uploads is the
    /// handler's responsibility because the target descriptor only arrives
    /// inside the first stream element.
    async fn do_put(
        &self,
        _subject: &Subject,
        _stream: Streaming<FlightData>,
    ) -> Result<Vec<PutResult>, Status> {
        Err(Status::unimplemented("do_put"))
    }
}

/// Domain-neutral Arrow Flight service: authn → ticket interpretation →
/// access decision → host handler.
pub struct FlightSkeleton {
    auth: Arc<dyn AuthN>,
    access: Arc<dyn AccessDecision>,
    tickets: Arc<dyn TicketInterpreter>,
    handlers: Arc<dyn FlightHandlers>,
}

impl FlightSkeleton {
    pub fn new(
        auth: Arc<dyn AuthN>,
        access: Arc<dyn AccessDecision>,
        tickets: Arc<dyn TicketInterpreter>,
        handlers: Arc<dyn FlightHandlers>,
    ) -> Self {
        Self {
            auth,
            access,
            tickets,
            handlers,
        }
    }

    async fn authorize(&self, metadata: &MetadataMap, ticket: &[u8]) -> Result<Subject, Status> {
        let subject = self.auth.authenticate(metadata)?;
        let grant = self.tickets.interpret(ticket).await?;
        match self.access.evaluate(&subject, &grant.policy) {
            AccessOutcome::Allow => Ok(subject),
            AccessOutcome::Deny { reason } => Err(Status::permission_denied(reason)),
        }
    }
}

type BoxStream<T> = Pin<Box<dyn futures::Stream<Item = Result<T, Status>> + Send>>;

#[tonic::async_trait]
impl FlightService for FlightSkeleton {
    type HandshakeStream = BoxStream<HandshakeResponse>;
    type ListFlightsStream = BoxStream<FlightInfo>;
    type DoGetStream = BoxStream<FlightData>;
    type DoPutStream = BoxStream<PutResult>;
    type DoActionStream = BoxStream<arrow_flight::Result>;
    type ListActionsStream = BoxStream<ActionType>;
    type DoExchangeStream = BoxStream<FlightData>;

    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        Err(Status::unimplemented(
            "authenticate via bearer token in request metadata instead of handshake",
        ))
    }

    async fn list_flights(
        &self,
        request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        let subject = self.auth.authenticate(request.metadata())?;
        let flights = self
            .handlers
            .list_flights(&subject, &request.get_ref().expression)
            .await?;
        Ok(Response::new(Box::pin(stream::iter(
            flights.into_iter().map(Ok),
        ))))
    }

    async fn get_flight_info(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let subject = self.auth.authenticate(request.metadata())?;
        let info = self
            .handlers
            .get_flight_info(&subject, request.get_ref())
            .await?;
        Ok(Response::new(info))
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
        let ticket = request.get_ref().ticket.clone();
        let subject = self.authorize(request.metadata(), &ticket).await?;
        let batches = self.handlers.do_get(&subject, &ticket).await?;
        let flight_data = FlightDataEncoderBuilder::new()
            .build(batches.map(|batch| {
                batch.map_err(|error| arrow_flight::error::FlightError::ExternalError(error.into()))
            }))
            .map(|data| data.map_err(Status::from));
        Ok(Response::new(Box::pin(flight_data)))
    }

    async fn do_put(
        &self,
        request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        let subject = self.auth.authenticate(request.metadata())?;
        let results = self.handlers.do_put(&subject, request.into_inner()).await?;
        Ok(Response::new(Box::pin(stream::iter(
            results.into_iter().map(Ok),
        ))))
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use arrow::array::Int64Array;
    use arrow::datatypes::{DataType as ArrowDataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use futures::TryStreamExt;
    use uuid::Uuid;

    use super::*;

    struct HeaderAuth;

    impl AuthN for HeaderAuth {
        fn authenticate(&self, metadata: &MetadataMap) -> Result<Subject, Status> {
            let registered = metadata.contains_key("authorization");
            Ok(Subject {
                id: Uuid::from_u128(1),
                registered,
                attributes: BTreeMap::new(),
            })
        }
    }

    struct RegisteredOnly;

    impl AccessDecision for RegisteredOnly {
        fn evaluate(&self, subject: &Subject, policy: &AccessPolicy) -> AccessOutcome {
            match policy {
                AccessPolicy::Open => AccessOutcome::Allow,
                _ if subject.registered => AccessOutcome::Allow,
                _ => AccessOutcome::Deny {
                    reason: "registration required".into(),
                },
            }
        }
    }

    struct RestrictedTickets;

    #[async_trait]
    impl TicketInterpreter for RestrictedTickets {
        async fn interpret(&self, _ticket: &[u8]) -> Result<TicketGrant, Status> {
            Ok(TicketGrant {
                resource: ResourceRef::new(Uuid::from_u128(7), Uuid::from_u128(9)),
                policy: AccessPolicy::Registered,
            })
        }
    }

    struct OneBatch;

    #[async_trait]
    impl FlightHandlers for OneBatch {
        async fn do_get(
            &self,
            _subject: &Subject,
            _ticket: &[u8],
        ) -> Result<RecordBatchStream, Status> {
            let schema = Arc::new(Schema::new(vec![Field::new(
                "x",
                ArrowDataType::Int64,
                false,
            )]));
            let batch =
                RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(vec![1_i64, 2]))])
                    .expect("record batch");
            Ok(Box::pin(stream::iter([Ok(batch)])))
        }
    }

    fn skeleton() -> FlightSkeleton {
        FlightSkeleton::new(
            Arc::new(HeaderAuth),
            Arc::new(RegisteredOnly),
            Arc::new(RestrictedTickets),
            Arc::new(OneBatch),
        )
    }

    #[test]
    fn do_get_denies_unregistered_subjects() {
        let service = skeleton();
        let request = Request::new(Ticket::new("anything"));
        let error = match futures::executor::block_on(service.do_get(request)) {
            Err(error) => error,
            Ok(_) => panic!("unregistered subject must be denied"),
        };
        assert_eq!(error.code(), tonic::Code::PermissionDenied);
        assert_eq!(error.message(), "registration required");
    }

    #[test]
    fn do_get_streams_flight_data_for_allowed_subjects() {
        let service = skeleton();
        let mut request = Request::new(Ticket::new("anything"));
        request
            .metadata_mut()
            .insert("authorization", "Bearer test".parse().expect("metadata"));
        let response =
            futures::executor::block_on(service.do_get(request)).expect("authorized do_get");
        let frames: Vec<FlightData> =
            futures::executor::block_on(response.into_inner().try_collect())
                .expect("flight data frames");
        // Schema frame + one data frame.
        assert!(frames.len() >= 2);
    }
}
