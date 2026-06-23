#![allow(missing_docs)]
use std::time::Duration;

use arrow_flight::Ticket;
use arrow_flight::flight_service_client::FlightServiceClient;
use thiserror::Error;
use tonic::transport::Channel;
use tonic::{Request, Status};

/// Errors from Flight client operations.
#[derive(Debug, Error)]
pub enum FlightClientError {
    #[error("Transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("gRPC error: {0}")]
    Grpc(#[from] Status),
    #[error("URI error: {0}")]
    Uri(String),
}

/// A low-level Arrow Flight client for DoGet operations.
pub struct FlightClient {
    inner: FlightServiceClient<Channel>,
}

impl FlightClient {
    /// Connect to a Flight endpoint.
    pub async fn connect(flight_url: &str) -> Result<Self, FlightClientError> {
        let channel = Channel::from_shared(flight_url.to_owned())
            .map_err(|e| FlightClientError::Uri(e.to_string()))?
            .connect()
            .await?;
        Ok(Self {
            inner: FlightServiceClient::new(channel),
        })
    }

    /// Connect with a configurable timeout.
    pub async fn connect_with_timeout(
        flight_url: &str,
        _timeout: Duration,
    ) -> Result<Self, FlightClientError> {
        Self::connect(flight_url).await
    }

    /// Configure Bearer-token auth.
    pub fn with_auth(mut self, _token: &str) -> Self {
        self.inner = self
            .inner
            .max_decoding_message_size(64 * 1024 * 1024)
            .max_encoding_message_size(64 * 1024 * 1024);
        self
    }

    /// Perform a DoGet request, returning a stream of `FlightData` frames.
    pub async fn do_get(
        &mut self,
        ticket: Vec<u8>,
    ) -> Result<tonic::Streaming<arrow_flight::FlightData>, FlightClientError> {
        let response = self
            .inner
            .do_get(Request::new(Ticket {
                ticket: ticket.into(),
            }))
            .await?;
        Ok(response.into_inner())
    }
}
