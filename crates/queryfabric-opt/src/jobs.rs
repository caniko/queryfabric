//! Resource and timeout sizing for isolated query execution.
//!
//! Pure math over a [`EstimatedCost`]: the host applies the results to its
//! own `IsolatedJobSpec` / worker orchestration. Floors are caller-supplied
//! because they encode backend bootstrap costs the planner cannot know.

use std::time::Duration;

use queryfabric_catalog::EstimatedCost;

const MI: u64 = 1024 * 1024;

/// Render a byte count as a Kubernetes-style mebibyte quantity (`"8192Mi"`),
/// clamped to `floor_bytes` from below and rounded up to a whole MiB.
pub fn memory_quantity(bytes: u64, floor_bytes: u64) -> String {
    let rounded_mib = bytes.max(floor_bytes).div_ceil(MI);
    format!("{rounded_mib}Mi")
}

/// Memory quantity for an isolated worker sized from a cost estimate.
pub fn memory_quantity_from_estimate(estimated: &EstimatedCost, floor_bytes: u64) -> String {
    memory_quantity(estimated.memory_bytes, floor_bytes)
}

/// Timeout for an isolated worker: the estimator's wallclock estimate, but
/// never below `floor` (worker bootstrap + readiness detection headroom).
pub fn timeout_from_estimate(estimated: &EstimatedCost, floor: Duration) -> Duration {
    Duration::from_millis(estimated.wallclock_estimate_ms).max(floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GI: u64 = 1024 * MI;

    #[test]
    fn memory_quantity_applies_floor_and_rounds_up_to_mib() {
        assert_eq!(memory_quantity(0, 8 * GI), "8192Mi");
        assert_eq!(memory_quantity(8 * GI + 1, 8 * GI), "8193Mi");
        assert_eq!(memory_quantity(16 * GI, 8 * GI), "16384Mi");
    }

    #[test]
    fn timeout_uses_estimate_only_above_floor() {
        let floor = Duration::from_secs(600);
        let short = EstimatedCost {
            wallclock_estimate_ms: 1_000,
            ..EstimatedCost::default()
        };
        let long = EstimatedCost {
            wallclock_estimate_ms: 3_600_000,
            ..EstimatedCost::default()
        };
        assert_eq!(timeout_from_estimate(&short, floor), floor);
        assert_eq!(
            timeout_from_estimate(&long, floor),
            Duration::from_secs(3_600)
        );
    }
}
