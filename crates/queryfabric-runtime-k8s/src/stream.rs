use std::time::Duration;

use arrow_flight::Ticket;
use arrow_flight::decode::FlightRecordBatchStream;
use arrow_flight::error::FlightError as ArrowFlightError;
use arrow_flight::flight_service_client::FlightServiceClient;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::api::ListParams;
use kube::{Api, Client};
use queryfabric::{DriverError, IsolatedJobSpec, RecordBatchStream};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;

use crate::error::{flight_to_runtime, spawn_error, spawn_message, tonic_to_runtime};
use crate::job_spec::job_label_selector;

/// Connect to the worker pod and stream Arrow Flight batches back to the caller.
pub async fn connect_job_stream(
    client: Client,
    namespace: &str,
    job_name: &str,
    spec: &IsolatedJobSpec,
    flight_port: u16,
    cancel: CancellationToken,
) -> Result<RecordBatchStream, DriverError> {
    let pod = wait_for_ready_pod(client, namespace, job_name, spec.timeout, cancel.clone()).await?;
    let endpoint = pod_flight_endpoint(&pod, flight_port)?;
    let mut client = connect_flight(&endpoint).await?;
    let ticket = Ticket {
        ticket: serde_json::to_vec(&spec.query)
            .map_err(|error| {
                DriverError::Spawn(format!(
                    "serialize isolated worker Flight ticket from query spec: {error}"
                ))
            })?
            .into(),
    };
    let response = tokio::select! {
        () = cancel.cancelled() => return Err(DriverError::Cancelled),
        response = client.do_get(ticket) => response.map_err(|error| {
            DriverError::Spawn(format!(
                "issue Flight DoGet to isolated worker endpoint {endpoint}: {error}"
            ))
        })?,
    };
    let flight_data = response
        .into_inner()
        .map_err(|error| ArrowFlightError::Tonic(Box::new(error)));
    let batches = FlightRecordBatchStream::new_from_flight_data(flight_data).map(|result| {
        result.map_err(|error| match error {
            ArrowFlightError::Tonic(status) => tonic_to_runtime(*status),
            other => flight_to_runtime(other),
        })
    });
    Ok(Box::pin(batches))
}

async fn wait_for_ready_pod(
    client: Client,
    namespace: &str,
    job_name: &str,
    timeout: Duration,
    cancel: CancellationToken,
) -> Result<Pod, DriverError> {
    let pods: Api<Pod> = Api::namespaced(client, namespace);
    let selector = job_label_selector(job_name);
    let list_params = ListParams::default().labels(&selector);
    let deadline = Instant::now() + timeout;

    loop {
        if cancel.is_cancelled() {
            return Err(DriverError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(DriverError::Timeout);
        }

        let pod_list = pods
            .list(&list_params)
            .await
            .map_err(|error| spawn_error("list isolated worker pods", error))?;
        for pod in pod_list {
            if pod_is_failed(&pod) {
                return Err(DriverError::WorkerFailure {
                    exit_code: 1,
                    message: pod_failure_message(&pod),
                });
            }
            if pod_is_ready(&pod) {
                return Ok(pod);
            }
        }

        tokio::select! {
            () = cancel.cancelled() => return Err(DriverError::Cancelled),
            () = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }
}

fn pod_flight_endpoint(pod: &Pod, flight_port: u16) -> Result<String, DriverError> {
    let status = pod
        .status
        .as_ref()
        .ok_or_else(|| spawn_message("Ready worker pod is missing status"))?;
    let pod_ip = status
        .pod_ip
        .as_ref()
        .ok_or_else(|| spawn_message("Ready worker pod is missing pod IP"))?;
    Ok(format!("{pod_ip}:{flight_port}"))
}

async fn connect_flight(endpoint: &str) -> Result<FlightServiceClient<Channel>, DriverError> {
    let uri = format!("http://{endpoint}");
    let channel = Channel::from_shared(uri)
        .map_err(|error| {
            DriverError::Spawn(format!(
                "build tonic channel from isolated worker Flight endpoint {endpoint}: {error}"
            ))
        })?
        .connect_timeout(Duration::from_secs(10))
        .connect()
        .await
        .map_err(|error| {
            DriverError::Spawn(format!(
                "connect to isolated worker Flight endpoint {endpoint}: {error}"
            ))
        })?;
    Ok(FlightServiceClient::new(channel))
}

fn pod_is_ready(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|status| status.conditions.as_ref())
        .is_some_and(|conditions| {
            conditions
                .iter()
                .any(|condition| condition.type_ == "Ready" && condition.status == "True")
        })
}

fn pod_is_failed(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|status| status.phase.as_deref())
        .is_some_and(|phase| phase == "Failed")
}

fn pod_failure_message(pod: &Pod) -> String {
    let name = pod
        .metadata
        .name
        .as_deref()
        .unwrap_or("<unknown isolated worker pod>");
    let phase = pod
        .status
        .as_ref()
        .and_then(|status| status.phase.as_deref())
        .unwrap_or("Failed");
    format!("{name} entered phase {phase}")
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::{PodCondition, PodStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::{pod_failure_message, pod_is_failed, pod_is_ready};

    #[test]
    fn pod_ready_requires_ready_true_condition() {
        let pod = pod_with_status(PodStatus {
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_owned(),
                status: "True".to_owned(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert!(pod_is_ready(&pod));
    }

    #[test]
    fn failed_pod_maps_to_worker_failure_context() {
        let pod = pod_with_status(PodStatus {
            phase: Some("Failed".to_owned()),
            ..PodStatus::default()
        });
        assert!(pod_is_failed(&pod));
        assert_eq!(pod_failure_message(&pod), "worker-0 entered phase Failed");
    }

    fn pod_with_status(status: PodStatus) -> k8s_openapi::api::core::v1::Pod {
        k8s_openapi::api::core::v1::Pod {
            metadata: ObjectMeta {
                name: Some("worker-0".to_owned()),
                ..ObjectMeta::default()
            },
            status: Some(status),
            ..Default::default()
        }
    }
}
