use std::time::Duration;

use async_trait::async_trait;
use queryfabric_ir::BoundQuery;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{DriverError, RecordBatchStream};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageAccessMode {
    ReplicatedReadOnly,
    SnapshotClone {
        source_pvc: String,
        snapshot_class: String,
    },
    ObjectStore {
        uri: String,
        format: ObjectStoreFormat,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectStoreFormat {
    Parquet,
    Arrow,
    Csv,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedJobSpec {
    pub query: BoundQuery,
    pub storage: StorageAccessMode,
    pub resources: ResourceRequest,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRequest {
    pub cpu_request: String,
    pub memory_request: String,
    pub cpu_limit: String,
    pub memory_limit: String,
}

#[async_trait]
pub trait IsolatedExecutionDriver: Send + Sync {
    async fn spawn(
        &self,
        spec: IsolatedJobSpec,
        cancel: CancellationToken,
    ) -> Result<RecordBatchStream, DriverError>;
}
