use queryfabric_catalog::{
    BackendExecutionLimits, EstimatedCost, QueryTimeoutClass, ResultDeliveryMode,
};
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

/// Pick the runtime mode a cost estimate lands in given the backend's byte
/// thresholds: interactive below `interactive_byte_limit`, batch below
/// `batch_byte_limit`, isolated above.
pub fn runtime_mode_for_estimate(
    estimated: &EstimatedCost,
    limits: &BackendExecutionLimits,
) -> ExecutionRuntimeMode {
    if estimated.memory_bytes > limits.batch_byte_limit {
        ExecutionRuntimeMode::Isolated
    } else if estimated.memory_bytes > limits.interactive_byte_limit {
        ExecutionRuntimeMode::Batch
    } else {
        ExecutionRuntimeMode::Interactive
    }
}

/// Combine the estimate-derived mode with a user-supplied timeout-class hint.
/// Hints only ever escalate: a batch hint promotes interactive work to batch,
/// an export hint forces isolation, and no hint demotes an isolated estimate.
pub fn resolve_runtime_mode(
    estimated: &EstimatedCost,
    limits: &BackendExecutionLimits,
    user_class_hint: Option<QueryTimeoutClass>,
) -> ExecutionRuntimeMode {
    let estimated_mode = runtime_mode_for_estimate(estimated, limits);
    match user_class_hint {
        None | Some(QueryTimeoutClass::Interactive) => estimated_mode,
        Some(QueryTimeoutClass::Batch) => {
            if matches!(
                estimated_mode,
                ExecutionRuntimeMode::Interactive | ExecutionRuntimeMode::Batch
            ) {
                ExecutionRuntimeMode::Batch
            } else {
                estimated_mode
            }
        }
        Some(QueryTimeoutClass::Export) => ExecutionRuntimeMode::Isolated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_runtime_mode_covers_estimate_and_user_hint_matrix() {
        let limits = BackendExecutionLimits {
            interactive_byte_limit: 10,
            batch_byte_limit: 20,
            ..BackendExecutionLimits::default()
        };
        let cases = [
            (5, None, ExecutionRuntimeMode::Interactive),
            (
                5,
                Some(QueryTimeoutClass::Interactive),
                ExecutionRuntimeMode::Interactive,
            ),
            (
                5,
                Some(QueryTimeoutClass::Batch),
                ExecutionRuntimeMode::Batch,
            ),
            (
                5,
                Some(QueryTimeoutClass::Export),
                ExecutionRuntimeMode::Isolated,
            ),
            (15, None, ExecutionRuntimeMode::Batch),
            (
                15,
                Some(QueryTimeoutClass::Interactive),
                ExecutionRuntimeMode::Batch,
            ),
            (
                15,
                Some(QueryTimeoutClass::Batch),
                ExecutionRuntimeMode::Batch,
            ),
            (
                15,
                Some(QueryTimeoutClass::Export),
                ExecutionRuntimeMode::Isolated,
            ),
            (25, None, ExecutionRuntimeMode::Isolated),
            (
                25,
                Some(QueryTimeoutClass::Interactive),
                ExecutionRuntimeMode::Isolated,
            ),
            (
                25,
                Some(QueryTimeoutClass::Batch),
                ExecutionRuntimeMode::Isolated,
            ),
            (
                25,
                Some(QueryTimeoutClass::Export),
                ExecutionRuntimeMode::Isolated,
            ),
        ];

        for (memory_bytes, hint, expected) in cases {
            let actual = resolve_runtime_mode(
                &EstimatedCost {
                    memory_bytes,
                    rows_scanned: 1,
                    partitions_touched: 1,
                    wallclock_estimate_ms: 1,
                },
                &limits,
                hint,
            );
            assert_eq!(actual, expected, "memory={memory_bytes}, hint={hint:?}");
        }
    }
}
