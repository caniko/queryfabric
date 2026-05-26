use std::pin::Pin;

use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use futures::Stream;
use queryfabric_ir::BoundQuery;
use tokio_util::sync::CancellationToken;

use crate::{ExecutionRuntimeMode, RuntimeError};

pub type RecordBatchStream = Pin<Box<dyn Stream<Item = Result<RecordBatch, RuntimeError>> + Send>>;

#[async_trait]
pub trait ExecutionRuntime: Send + Sync {
    async fn execute(
        &self,
        plan: BoundQuery,
        mode: ExecutionRuntimeMode,
        cancel: CancellationToken,
    ) -> Result<RecordBatchStream, RuntimeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InteractiveRuntime;

#[async_trait]
impl ExecutionRuntime for InteractiveRuntime {
    async fn execute(
        &self,
        _plan: BoundQuery,
        _mode: ExecutionRuntimeMode,
        _cancel: CancellationToken,
    ) -> Result<RecordBatchStream, RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }
}
