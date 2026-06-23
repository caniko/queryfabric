use k8s_openapi::api::batch::v1::Job;
use kube::api::DeleteParams;
use kube::{Api, Error};
use tokio_util::sync::CancellationToken;

pub(crate) use queryfabric::spawn_traced;

/// Delete parameters that remove isolated Jobs immediately in the background.
pub fn background_delete_params() -> DeleteParams {
    DeleteParams {
        grace_period_seconds: Some(0),
        ..DeleteParams::background()
    }
}

/// Spawn a cancellation watcher that deletes the Job when the token fires.
pub fn spawn_cancel_delete_task(api: Api<Job>, job_name: String, cancel: CancellationToken) {
    spawn_traced("cancel-delete-job", async move {
        cancel.cancelled().await;
        if let Err(error) = delete_job_background(&api, &job_name).await {
            tracing::warn!(job_name, %error, "failed to delete isolated execution Job after cancel");
        }
    });
}

/// Delete an isolated worker Job using background propagation.
pub async fn delete_job_background(api: &Api<Job>, job_name: &str) -> Result<(), Error> {
    match api.delete(job_name, &background_delete_params()).await {
        Ok(_) => Ok(()),
        Err(Error::Api(response)) if response.code == 404 => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::time::Duration;

    use http::{Method, Response};
    use k8s_openapi::api::batch::v1::Job;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Status;
    use kube::api::PropagationPolicy;
    use kube::client::Body;
    use kube::{Api, Client};
    use tokio_util::sync::CancellationToken;
    use tower_test::mock;

    use super::{background_delete_params, spawn_cancel_delete_task};

    #[test]
    fn cancel_deletion_uses_background_propagation() {
        let params = background_delete_params();
        assert_eq!(params.grace_period_seconds, Some(0));
        assert_eq!(
            params.propagation_policy,
            Some(PropagationPolicy::Background)
        );
    }

    #[tokio::test]
    async fn cancellation_triggers_job_deletion_against_mock_client() {
        let (mock_service, handle) = mock::pair();
        let client = Client::new(mock_service, "syndb");
        let jobs: Api<Job> = Api::namespaced(client, "syndb");
        let cancel = CancellationToken::new();

        spawn_cancel_delete_task(jobs, "qf-isolated-test".to_owned(), cancel.clone());
        cancel.cancel();

        let mut handle = pin!(handle);
        let observed = tokio::time::timeout(Duration::from_secs(2), handle.next_request())
            .await
            .expect("delete request should be sent within 2s")
            .expect("mock service should receive request");
        let (request, send) = observed;
        assert_eq!(request.method(), Method::DELETE);
        assert_eq!(
            request.uri().path(),
            "/apis/batch/v1/namespaces/syndb/jobs/qf-isolated-test"
        );
        assert_eq!(request.uri().query(), Some(""));
        send.send_response(
            Response::builder()
                .body(Body::from(
                    serde_json::to_vec(&Status {
                        status: Some("Success".to_owned()),
                        ..Status::default()
                    })
                    .expect("status json"),
                ))
                .expect("response"),
        );
    }
}
