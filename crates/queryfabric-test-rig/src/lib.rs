//! Docker / rootless-Podman primitives for integration test harnesses.
//!
//! Provides connection bring-up, network and container lifecycle, image
//! ensure / load, and registry push helpers. All functions are agnostic to
//! the application using them — service-specific orchestration belongs at
//! the call site.

#![warn(missing_docs)]

mod docker;
mod publish;
mod rig;

pub use bollard::Docker;
pub use docker::{
    cleanup_network, connect_docker, docker_cp_out, docker_load, ensure_image, ensure_network,
    run_container_to_completion, start_container, start_container_with_port_bindings,
    start_container_with_ports, upload_tar_to_container, wait_for_port,
};
pub use publish::tag_and_push_image;
pub use rig::{
    ClickHouseService, MeilisearchService, MinioService, PostgresService, TestRig, TestRigBuilder,
};
