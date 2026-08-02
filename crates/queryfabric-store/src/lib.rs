//! S3-compatible object store with presigned URLs, over OpenDAL.
//!
//! Wraps an [`opendal::Operator`] with the operations a sovereign data
//! platform needs: presigned upload/download URL generation and direct
//! put/get for small payloads (e.g. export bundles from
//! `queryfabric-portability`). The backend is chosen by OpenDAL
//! configuration — AWS S3, MinIO, and Garage all work unchanged (enable the
//! `s3` cargo feature); the `memory` feature provides an in-process backend
//! for tests and the demonstrator host.
//!
//! Presigning is an S3-protocol capability: in-process backends (memory, fs)
//! do not support it. [`ObjectStore`] checks the operator's capability and
//! returns [`StoreError::PresignUnsupported`] instead of panicking, so a
//! misconfigured backend fails loudly and clearly.

use std::time::Duration;

use opendal::Operator;

/// Default presigned-URL expiration (2 hours).
pub const DEFAULT_PRESIGN_TTL: Duration = Duration::from_secs(7200);

/// Error surface for object-store operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The object path was empty.
    #[error("object path must not be empty")]
    EmptyPath,
    /// The configured backend cannot presign URLs.
    #[error(
        "backend '{scheme}' does not support presigned URLs; use an S3-compatible \
         backend (AWS S3, MinIO, Garage)"
    )]
    PresignUnsupported {
        /// OpenDAL scheme name of the backend.
        scheme: String,
    },
    /// The underlying OpenDAL operation failed.
    #[error("object store operation failed on '{path}': {source}")]
    Backend {
        /// Object path the operation targeted.
        path: String,
        /// OpenDAL error.
        #[source]
        source: opendal::Error,
    },
    /// The backend configuration was rejected before any operation ran.
    #[error("object store configuration rejected: {source}")]
    Configuration {
        /// OpenDAL error.
        #[source]
        source: opendal::Error,
    },
}

/// Configuration for any S3-compatible backend (AWS S3, MinIO, Garage).
#[cfg(feature = "s3")]
#[derive(Clone)]
pub struct S3Config {
    /// Bucket holding all objects.
    pub bucket: String,
    /// Endpoint URL; `None` uses the backend's default (AWS).
    pub endpoint: Option<String>,
    /// Region; many S3-compatible stores accept any value here.
    pub region: Option<String>,
    /// Access key id.
    pub access_key_id: String,
    /// Secret access key.
    pub secret_access_key: String,
    /// Path prefix all objects live under, when set.
    pub root: Option<String>,
}

#[cfg(feature = "s3")]
impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"<redacted>")
            .field("root", &self.root)
            .finish()
    }
}

/// Object storage over any OpenDAL backend.
#[derive(Debug, Clone)]
pub struct ObjectStore {
    op: Operator,
}

impl ObjectStore {
    /// Wrap a configured OpenDAL operator.
    #[must_use]
    pub fn new(op: Operator) -> Self {
        Self { op }
    }

    /// An in-process store for tests and demos. Not durable.
    #[cfg(feature = "memory")]
    #[must_use]
    pub fn memory() -> Self {
        let op = Operator::new(opendal::services::Memory::default())
            .expect("memory backend has no fallible configuration");
        Self::new(op)
    }

    /// A store over any S3-compatible backend.
    #[cfg(feature = "s3")]
    pub fn s3(config: S3Config) -> Result<Self, StoreError> {
        let mut builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key);
        if let Some(endpoint) = &config.endpoint {
            builder = builder.endpoint(endpoint);
        }
        if let Some(region) = &config.region {
            builder = builder.region(region);
        }
        if let Some(root) = &config.root {
            builder = builder.root(root);
        }
        let op = Operator::new(builder).map_err(|source| StoreError::Configuration { source })?;
        Ok(Self::new(op))
    }

    /// The wrapped operator, for operations this facade does not cover.
    #[must_use]
    pub fn operator(&self) -> &Operator {
        &self.op
    }

    fn validated<'a>(&self, path: &'a str) -> Result<&'a str, StoreError> {
        if path.is_empty() {
            return Err(StoreError::EmptyPath);
        }
        Ok(path)
    }

    fn require_presign(&self, write: bool) -> Result<(), StoreError> {
        let capability = self.op.info().capability();
        let supported = if write {
            capability.presign_write
        } else {
            capability.presign_read
        };
        if supported {
            Ok(())
        } else {
            Err(StoreError::PresignUnsupported {
                scheme: self.op.info().scheme().to_string(),
            })
        }
    }

    /// Generate a presigned URL for uploading to `path`.
    pub async fn presigned_upload(&self, path: &str, ttl: Duration) -> Result<String, StoreError> {
        self.validated(path)?;
        self.require_presign(true)?;
        let presigned =
            self.op
                .presign_write(path, ttl)
                .await
                .map_err(|source| StoreError::Backend {
                    path: path.to_owned(),
                    source,
                })?;
        Ok(presigned.uri().to_string())
    }

    /// Generate a presigned URL for downloading `path`.
    pub async fn presigned_download(
        &self,
        path: &str,
        ttl: Duration,
    ) -> Result<String, StoreError> {
        self.validated(path)?;
        self.require_presign(false)?;
        let presigned =
            self.op
                .presign_read(path, ttl)
                .await
                .map_err(|source| StoreError::Backend {
                    path: path.to_owned(),
                    source,
                })?;
        Ok(presigned.uri().to_string())
    }

    /// Write `bytes` to `path`.
    pub async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.validated(path)?;
        self.op
            .write(path, bytes)
            .await
            .map(|_| ())
            .map_err(|source| StoreError::Backend {
                path: path.to_owned(),
                source,
            })
    }

    /// Read the full object at `path`.
    pub async fn get(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        self.validated(path)?;
        let buffer = self
            .op
            .read(path)
            .await
            .map_err(|source| StoreError::Backend {
                path: path.to_owned(),
                source,
            })?;
        Ok(buffer.to_vec())
    }

    /// Delete an object at `path`.
    ///
    /// Cleanup is deliberately explicit: callers decide whether an object is
    /// still referenced by durable state before removing it.  OpenDAL treats
    /// deleting an already absent object as success for the supported stores,
    /// which makes this safe for retryable cleanup paths.
    pub async fn delete(&self, path: &str) -> Result<(), StoreError> {
        self.validated(path)?;
        self.op
            .delete(path)
            .await
            .map_err(|source| StoreError::Backend {
                path: path.to_owned(),
                source,
            })
    }
}

#[cfg(all(test, feature = "memory"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_round_trip_on_memory_backend() {
        let store = ObjectStore::memory();
        store
            .put("results/bundle.json", b"{\"ok\":true}".to_vec())
            .await
            .expect("put");
        let bytes = store.get("results/bundle.json").await.expect("get");
        assert_eq!(bytes, b"{\"ok\":true}");
    }

    #[tokio::test]
    async fn delete_removes_staged_object() {
        let store = ObjectStore::memory();
        store
            .put("imports/staging/object.csv", b"payload".to_vec())
            .await
            .expect("put");
        store
            .delete("imports/staging/object.csv")
            .await
            .expect("delete");
        assert!(matches!(
            store.get("imports/staging/object.csv").await,
            Err(StoreError::Backend { .. })
        ));
    }

    #[tokio::test]
    async fn empty_path_fails_fast() {
        let store = ObjectStore::memory();
        assert!(matches!(
            store.put("", Vec::new()).await,
            Err(StoreError::EmptyPath)
        ));
        assert!(matches!(
            store.presigned_upload("", DEFAULT_PRESIGN_TTL).await,
            Err(StoreError::EmptyPath)
        ));
    }

    #[tokio::test]
    async fn presign_is_capability_gated_not_a_panic() {
        let store = ObjectStore::memory();
        // The in-process memory backend cannot presign; the store must say
        // so clearly instead of panicking or returning a bogus URL.
        let upload = store
            .presigned_upload("results/bundle.json", DEFAULT_PRESIGN_TTL)
            .await;
        match upload {
            Err(StoreError::PresignUnsupported { scheme }) => {
                assert_eq!(scheme, "memory");
            }
            other => panic!("expected PresignUnsupported, got {other:?}"),
        }
        let download = store
            .presigned_download("results/bundle.json", DEFAULT_PRESIGN_TTL)
            .await;
        assert!(matches!(
            download,
            Err(StoreError::PresignUnsupported { .. })
        ));
    }

    #[tokio::test]
    async fn missing_object_surfaces_backend_error_with_path() {
        let store = ObjectStore::memory();
        let err = store.get("absent/object").await.expect_err("missing");
        match err {
            StoreError::Backend { path, .. } => assert_eq!(path, "absent/object"),
            other => panic!("expected Backend error, got {other:?}"),
        }
    }
}
