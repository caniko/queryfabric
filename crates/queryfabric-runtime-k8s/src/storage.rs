#[cfg(feature = "snapshot-clone")]
use k8s_openapi::api::core::v1::PersistentVolumeClaimVolumeSource;
use k8s_openapi::api::core::v1::{EnvVar, Volume, VolumeMount};
use queryfabric::{DriverError, ObjectStoreFormat, StorageAccessMode};

#[cfg(not(feature = "snapshot-clone"))]
use crate::error::spawn_message;

/// Materialized storage mounts and environment variables for worker pods.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StorageArtifacts {
    /// Environment variables required by the worker.
    pub env: Vec<EnvVar>,
    /// Volumes mounted into the worker pod.
    pub volumes: Vec<Volume>,
    /// Volume mounts applied to the worker container.
    pub mounts: Vec<VolumeMount>,
}

/// Resolve worker storage configuration for the selected access mode.
pub fn storage_artifacts(storage: &StorageAccessMode) -> Result<StorageArtifacts, DriverError> {
    match storage {
        StorageAccessMode::ReplicatedReadOnly => Ok(StorageArtifacts::default()),
        StorageAccessMode::ObjectStore { uri, format } => Ok(object_store_artifacts(uri, *format)),
        StorageAccessMode::SnapshotClone {
            source_pvc,
            snapshot_class,
        } => snapshot_clone_artifacts(source_pvc, snapshot_class),
    }
}

fn object_store_artifacts(uri: &str, format: ObjectStoreFormat) -> StorageArtifacts {
    StorageArtifacts {
        env: vec![
            env("SYNDB_STORAGE_MODE", "object-store"),
            env("SYNDB_OBJECT_STORE_URI", uri),
            env("SYNDB_OBJECT_STORE_FORMAT", object_store_format(format)),
        ],
        ..StorageArtifacts::default()
    }
}

#[cfg(not(feature = "snapshot-clone"))]
fn snapshot_clone_artifacts(
    _source_pvc: &str,
    _snapshot_class: &str,
) -> Result<StorageArtifacts, DriverError> {
    Err(spawn_message(
        "SnapshotClone not enabled; install with feature `snapshot-clone`",
    ))
}

#[cfg(feature = "snapshot-clone")]
fn snapshot_clone_artifacts(
    source_pvc: &str,
    snapshot_class: &str,
) -> Result<StorageArtifacts, DriverError> {
    let pvc_name = snapshot_clone_pvc_name(source_pvc, snapshot_class);
    Ok(StorageArtifacts {
        env: vec![
            env("SYNDB_STORAGE_MODE", "snapshot-clone"),
            env("SYNDB_SOURCE_PVC", source_pvc),
            env("SYNDB_VOLUME_SNAPSHOT_CLASS", snapshot_class),
            env("SYNDB_SNAPSHOT_CLONE_PVC", &pvc_name),
        ],
        volumes: vec![Volume {
            name: "clickhouse-data".to_owned(),
            persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                claim_name: pvc_name,
                read_only: Some(true),
            }),
            ..Volume::default()
        }],
        mounts: vec![VolumeMount {
            name: "clickhouse-data".to_owned(),
            mount_path: "/var/lib/clickhouse".to_owned(),
            read_only: Some(true),
            ..VolumeMount::default()
        }],
    })
}

#[cfg(feature = "snapshot-clone")]
fn snapshot_clone_pvc_name(source_pvc: &str, snapshot_class: &str) -> String {
    format!(
        "syndb-snapshot-{}-{}",
        sanitize_dns_label(source_pvc),
        sanitize_dns_label(snapshot_class)
    )
}

fn object_store_format(format: ObjectStoreFormat) -> &'static str {
    match format {
        ObjectStoreFormat::Parquet => "parquet",
        ObjectStoreFormat::Arrow => "arrow",
        ObjectStoreFormat::Csv => "csv",
    }
}

/// Construct a Kubernetes environment variable.
pub(crate) fn env(name: &str, value: &str) -> EnvVar {
    EnvVar {
        name: name.to_owned(),
        value: Some(value.to_owned()),
        ..EnvVar::default()
    }
}

#[cfg(feature = "snapshot-clone")]
fn sanitize_dns_label(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "volume".to_owned()
    } else {
        trimmed.chars().take(45).collect()
    }
}

#[cfg(test)]
mod tests {
    use queryfabric::{ObjectStoreFormat, StorageAccessMode};

    use super::storage_artifacts;

    #[test]
    fn replicated_read_only_has_no_extra_storage() {
        let artifacts = storage_artifacts(&StorageAccessMode::ReplicatedReadOnly).expect("storage");
        assert!(artifacts.env.is_empty());
        assert!(artifacts.volumes.is_empty());
        assert!(artifacts.mounts.is_empty());
    }

    #[test]
    fn object_store_uses_endpoint_env_without_secret_values() {
        let artifacts = storage_artifacts(&StorageAccessMode::ObjectStore {
            uri: "s3://syndb-search/results".to_owned(),
            format: ObjectStoreFormat::Parquet,
        })
        .expect("storage");
        assert!(artifacts.volumes.is_empty());
        assert!(artifacts.mounts.is_empty());
        assert_eq!(artifacts.env[0].value.as_deref(), Some("object-store"));
        assert_eq!(
            artifacts.env[1].value.as_deref(),
            Some("s3://syndb-search/results")
        );
        assert_eq!(artifacts.env[2].value.as_deref(), Some("parquet"));
    }

    #[cfg(not(feature = "snapshot-clone"))]
    #[test]
    fn snapshot_clone_is_gated_without_feature() {
        let error = storage_artifacts(&StorageAccessMode::SnapshotClone {
            source_pvc: "clickhouse-data".to_owned(),
            snapshot_class: "csi-snap".to_owned(),
        })
        .expect_err("feature should be disabled");
        assert!(error.to_string().contains("SnapshotClone not enabled"));
    }

    #[cfg(feature = "snapshot-clone")]
    #[test]
    fn snapshot_clone_mounts_read_only_pvc() {
        let artifacts = storage_artifacts(&StorageAccessMode::SnapshotClone {
            source_pvc: "ClickHouse_Data_0".to_owned(),
            snapshot_class: "csi-snap".to_owned(),
        })
        .expect("storage");
        assert_eq!(artifacts.volumes.len(), 1);
        assert_eq!(artifacts.mounts.len(), 1);
        assert_eq!(artifacts.mounts[0].mount_path, "/var/lib/clickhouse");
        assert_eq!(artifacts.mounts[0].read_only, Some(true));
    }
}
