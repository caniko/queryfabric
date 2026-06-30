//! Reusable Prometheus instrumentation helpers.
//!
//! Provides preset histogram bucket configurations for the three common
//! shapes (latency, job duration, byte sizes), a unix-timestamp helper for
//! `_unixtime` gauges, and a thin [`MetricsRegistry`] wrapper that owns a
//! `Mutex<Registry>` and renders to OpenMetrics text.

#![warn(missing_docs)]

use std::sync::Mutex;

use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;

/// QueryFabric service-level metric families and recording helpers.
pub mod metrics;
/// Query-planning metric families and labels.
pub mod query;

pub use metrics::*;
pub use query::*;

/// Histogram for sub-second latencies.
///
/// 18 exponential buckets starting at 1 ms with factor 2 — covers up to
/// ~131 seconds. Suitable for HTTP / RPC / Flight request durations.
pub fn latency_histogram() -> Histogram {
    Histogram::new(exponential_buckets(0.001, 2.0, 18))
}

/// Histogram for batch job durations.
///
/// 20 exponential buckets starting at 100 ms with factor 2 — covers up to
/// ~29 hours. Suitable for query / graph / export job lifetimes.
pub fn job_duration_histogram() -> Histogram {
    Histogram::new(exponential_buckets(0.1, 2.0, 20))
}

/// Histogram for response/result byte sizes.
///
/// 24 exponential buckets starting at 1 KiB with factor 2 — covers up to
/// ~16 TiB. Suitable for HTTP response bodies, parquet artefacts, etc.
pub fn bytes_histogram() -> Histogram {
    Histogram::new(exponential_buckets(1024.0, 2.0, 24))
}

/// Current Unix epoch in seconds, or 0 if the system clock is before 1970.
///
/// Convenient for `*_last_success_unixtime` gauges.
pub fn unix_timestamp_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn unix_timestamp_seconds_i64() -> i64 {
    i64::try_from(unix_timestamp_seconds()).unwrap_or(i64::MAX)
}

/// Thin wrapper over `prometheus_client::registry::Registry` that owns a
/// `Mutex` and exposes a `render()` method.
///
/// Construct with [`MetricsRegistry::with_prefix`] and call
/// [`MetricsRegistry::register`] for each metric family. The registry can
/// be rendered concurrently from any thread.
#[derive(Debug)]
pub struct MetricsRegistry {
    registry: Mutex<Registry>,
}

impl MetricsRegistry {
    /// Create a registry whose metric names are prefixed with `prefix`.
    #[must_use]
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            registry: Mutex::new(Registry::with_prefix(prefix)),
        }
    }

    /// Register a metric family under `name` with `help` text.
    ///
    /// The metric must be `Clone` (the registry stores its own copy);
    /// callers retain a clone for live updates. This mirrors the upstream
    /// `Registry::register` signature.
    pub fn register<M>(&self, name: &str, help: &str, metric: M)
    where
        M: prometheus_client::registry::Metric + Clone,
    {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register(name, help, metric);
    }

    /// Render the registry to OpenMetrics text format.
    ///
    /// # Errors
    /// Returns any text-encoding error reported by the OpenMetrics encoder.
    pub fn render(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();
        let registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        encode(&mut out, &registry)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_timestamp_is_recent() {
        let now = unix_timestamp_seconds();
        // Sanity: after 2020 (epoch sec), before year 2200.
        assert!(now > 1_577_836_800);
        assert!(now < 7_258_118_400);
    }

    #[test]
    fn registry_renders_empty() {
        let reg = MetricsRegistry::with_prefix("test");
        let text = reg.render().unwrap();
        assert!(text.contains("# EOF"));
    }
}
