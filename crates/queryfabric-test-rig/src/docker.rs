//! Utility functions for Docker container operations via bollard.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use bollard::Docker;
use bollard::models::{ContainerCreateBody, HostConfig, NetworkCreateRequest, PortBinding};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, LogsOptionsBuilder,
    RemoveContainerOptionsBuilder, StartContainerOptions, UploadToContainerOptionsBuilder,
    WaitContainerOptions,
};
use futures_util::StreamExt;
use tokio::net::TcpStream;

/// Connect to the container runtime (Docker or rootless Podman).
///
/// Priority: `$DOCKER_HOST` → rootless Podman socket → system Docker socket.
pub fn connect_docker() -> eyre::Result<Docker> {
    // DOCKER_HOST takes precedence (handled by connect_with_local_defaults)
    if std::env::var("DOCKER_HOST").is_ok() {
        return Ok(Docker::connect_with_local_defaults()?);
    }

    // Probe rootless Podman socket
    // SAFETY: `getuid()` is an always-successful, thread-safe POSIX syscall that
    // takes no arguments, returns the caller's real UID, and cannot fail or
    // produce an invalid value, so the call has no preconditions to uphold.
    let uid = unsafe { libc::getuid() };
    let podman_sock = format!("/run/user/{uid}/podman/podman.sock");
    if std::path::Path::new(&podman_sock).exists() {
        return Ok(Docker::connect_with_unix(
            &podman_sock,
            120,
            bollard::API_DEFAULT_VERSION,
        )?);
    }

    // Fall back to system Docker socket
    Ok(Docker::connect_with_local_defaults()?)
}

/// Ensure a Docker network exists, creating it if needed.
pub async fn ensure_network(docker: &Docker, name: &str) -> eyre::Result<()> {
    match docker
        .inspect_network(
            name,
            None::<bollard::query_parameters::InspectNetworkOptions>,
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => {
            docker
                .create_network(NetworkCreateRequest {
                    name: name.to_string(),
                    driver: Some("bridge".to_string()),
                    ..Default::default()
                })
                .await?;
            Ok(())
        }
    }
}

/// Remove a Docker network and all containers attached to it.
pub async fn cleanup_network(docker: &Docker, network_name: &str) -> eyre::Result<()> {
    if let Ok(info) = docker
        .inspect_network(
            network_name,
            None::<bollard::query_parameters::InspectNetworkOptions>,
        )
        .await
        && let Some(containers) = info.containers
    {
        for (id, _) in containers {
            let _ = docker
                .remove_container(
                    &id,
                    Some(RemoveContainerOptionsBuilder::default().force(true).build()),
                )
                .await;
        }
    }
    let _ = docker.remove_network(network_name).await;
    Ok(())
}

/// Poll a TCP port until it is reachable or the timeout expires.
pub async fn wait_for_port(host: &str, port: u16, timeout: Duration) -> eyre::Result<()> {
    let deadline = Instant::now() + timeout;
    let addr = format!("{host}:{port}");
    while Instant::now() < deadline {
        if TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    eyre::bail!(
        "tcp endpoint {addr} did not become reachable within {timeout:?}; confirm the container started and bound the expected port"
    )
}

/// Start a named container on a given network with the specified config.
///
/// Returns the container ID. The container name is also used as its DNS
/// hostname within the Docker network.
pub async fn start_container(
    docker: &Docker,
    name: &str,
    network: &str,
    image: &str,
    env: Vec<String>,
    cmd: Option<Vec<String>>,
) -> eyre::Result<String> {
    let config = ContainerCreateBody {
        image: Some(image.to_string()),
        env: Some(env),
        cmd,
        host_config: Some(HostConfig {
            network_mode: Some(network.to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default().name(name).build();
    let container = docker.create_container(Some(options), config).await?;
    docker
        .start_container(&container.id, None::<StartContainerOptions>)
        .await?;
    Ok(container.id)
}

/// Start a named container on a given network with explicit host port bindings.
pub async fn start_container_with_ports(
    docker: &Docker,
    name: &str,
    network: &str,
    image: &str,
    env: Vec<String>,
    cmd: Option<Vec<String>>,
    ports: &[(u16, u16)],
) -> eyre::Result<String> {
    let ports: Vec<_> = ports
        .iter()
        .map(|(container_port, host_port)| (*container_port, *host_port, "127.0.0.1"))
        .collect();
    start_container_with_port_bindings(docker, name, network, image, env, cmd, &ports).await
}

/// Start a named container with explicit host port bindings and host IPs.
pub async fn start_container_with_port_bindings(
    docker: &Docker,
    name: &str,
    network: &str,
    image: &str,
    env: Vec<String>,
    cmd: Option<Vec<String>>,
    ports: &[(u16, u16, &str)],
) -> eyre::Result<String> {
    let exposed_ports = if ports.is_empty() {
        None
    } else {
        Some(
            ports
                .iter()
                .map(|(container_port, _, _)| format!("{container_port}/tcp"))
                .collect(),
        )
    };

    let port_bindings = if ports.is_empty() {
        None
    } else {
        let mut bindings = HashMap::new();
        for (container_port, host_port, host_ip) in ports {
            bindings.insert(
                format!("{container_port}/tcp"),
                Some(vec![PortBinding {
                    host_ip: Some((*host_ip).to_owned()),
                    host_port: Some(host_port.to_string()),
                }]),
            );
        }
        Some(bindings)
    };

    let config = ContainerCreateBody {
        image: Some(image.to_string()),
        env: Some(env),
        cmd,
        exposed_ports,
        host_config: Some(HostConfig {
            network_mode: Some(network.to_string()),
            port_bindings,
            ..Default::default()
        }),
        ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default().name(name).build();
    let container = docker.create_container(Some(options), config).await?;
    docker
        .start_container(&container.id, None::<StartContainerOptions>)
        .await?;
    Ok(container.id)
}

/// Run a container to completion and return its stdout.
pub async fn run_container_to_completion(
    docker: &Docker,
    name: &str,
    network: &str,
    image: &str,
    env: Vec<String>,
    cmd: Vec<String>,
) -> eyre::Result<String> {
    let id = start_container(docker, name, network, image, env, Some(cmd)).await?;

    // Wait for exit
    let mut wait_stream = docker.wait_container(&id, None::<WaitContainerOptions>);
    let mut exit_code = 0i64;
    while let Some(result) = wait_stream.next().await {
        match result {
            Ok(response) => {
                exit_code = response.status_code;
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Collect logs
    let mut logs = String::new();
    let log_opts = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .build();
    let mut log_stream = docker.logs(&id, Some(log_opts));
    while let Some(Ok(chunk)) = log_stream.next().await {
        logs.push_str(&chunk.to_string());
    }

    // Remove container
    let _ = docker
        .remove_container(
            &id,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await;

    if exit_code != 0 {
        eyre::bail!(
            "container {name} exited with code {exit_code}; inspect the captured logs for the failing process:\n{logs}"
        );
    }

    Ok(logs)
}

/// Copy a directory out of a container to the host filesystem.
///
/// # Errors
/// Returns any `docker cp` invocation failure.
pub async fn docker_cp_out(
    container_name: &str,
    container_path: &str,
    host_path: &str,
) -> eyre::Result<()> {
    let status = tokio::process::Command::new("docker")
        .args([
            "cp",
            &format!("{container_name}:{container_path}"),
            host_path,
        ])
        .status()
        .await?;
    if !status.success() {
        eyre::bail!(
            "docker cp from {container_name}:{container_path} to {host_path} failed; verify the source path exists in the container and the destination path is writable"
        );
    }
    Ok(())
}

/// Load an OCI tarball into the Docker daemon and return the image ID.
pub async fn docker_load(tarball_path: &str) -> eyre::Result<String> {
    let output = tokio::process::Command::new("docker")
        .args(["load", "-i", tarball_path])
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eyre::bail!(
            "docker load -i {tarball_path} failed with stderr: {stderr}; confirm the tarball is a valid OCI archive"
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let image_ref = stdout
        .lines()
        .find_map(|line| {
            line.strip_prefix("Loaded image: ")
                .or_else(|| line.strip_prefix("Loaded image ID: "))
        })
        .unwrap_or("unknown")
        .trim()
        .to_string();
    Ok(image_ref)
}

/// Pull an image if not already present.
pub async fn ensure_image(docker: &Docker, image: &str) -> eyre::Result<()> {
    if docker.inspect_image(image).await.is_ok() {
        return Ok(());
    }
    let options = CreateImageOptionsBuilder::default()
        .from_image(image)
        .build();
    let mut stream = docker.create_image(Some(options), None, None);
    while let Some(result) = stream.next().await {
        result?;
    }
    Ok(())
}

/// Upload a tar archive into a container at the given path.
pub async fn upload_tar_to_container(
    docker: &Docker,
    container_id: &str,
    path: &str,
    tar_bytes: Vec<u8>,
) -> eyre::Result<()> {
    let options = UploadToContainerOptionsBuilder::default()
        .path(path)
        .build();
    docker
        .upload_to_container(
            container_id,
            Some(options),
            bollard::body_full(tar_bytes.into()),
        )
        .await?;
    Ok(())
}
