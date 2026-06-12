//! Tag-and-push helpers for Docker registries.

use bollard::Docker;
use bollard::auth::DockerCredentials;
use bollard::query_parameters::{PushImageOptionsBuilder, TagImageOptionsBuilder};
use futures_util::StreamExt;

/// Tag a local image as `{remote_repo}:{version}` and `{remote_repo}:latest`,
/// then push both to the registry concurrently.
pub async fn tag_and_push_image(
    docker: &Docker,
    local_image: &str,
    remote_repo: &str,
    version: &str,
    credentials: &DockerCredentials,
) -> eyre::Result<Vec<String>> {
    let tag_v = TagImageOptionsBuilder::default()
        .repo(remote_repo)
        .tag(version)
        .build();
    docker.tag_image(local_image, Some(tag_v)).await?;

    let tag_l = TagImageOptionsBuilder::default()
        .repo(remote_repo)
        .tag("latest")
        .build();
    docker.tag_image(local_image, Some(tag_l)).await?;

    let versioned = format!("{remote_repo}:{version}");
    let latest = format!("{remote_repo}:latest");
    tokio::try_join!(
        drain_push(docker, remote_repo, version, credentials),
        drain_push(docker, remote_repo, "latest", credentials),
    )?;

    Ok(vec![
        format!("Published {versioned}"),
        format!("Published {latest}"),
    ])
}

async fn drain_push(
    docker: &Docker,
    repo: &str,
    tag: &str,
    credentials: &DockerCredentials,
) -> eyre::Result<()> {
    let options = PushImageOptionsBuilder::default().tag(tag).build();
    let mut stream = docker.push_image(repo, Some(options), Some(credentials.clone()));
    while let Some(result) = stream.next().await {
        match result {
            Ok(info) => {
                if let Some(detail) = info.error_detail {
                    eyre::bail!(
                        "Push error for {repo}:{tag}: {}",
                        detail.message.unwrap_or_default()
                    );
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
