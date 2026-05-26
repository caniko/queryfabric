use queryfabric_catalog::ResultDeliveryMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionRuntimeMode {
    Interactive,
    Batch,
    Isolated,
    Federated,
}

impl From<ResultDeliveryMode> for ExecutionRuntimeMode {
    fn from(mode: ResultDeliveryMode) -> Self {
        match mode {
            ResultDeliveryMode::InteractiveStream => Self::Interactive,
            ResultDeliveryMode::PagedResult => Self::Batch,
            ResultDeliveryMode::AsyncMaterializedExport => Self::Isolated,
            ResultDeliveryMode::RejectedOverBudget => Self::Interactive,
        }
    }
}
