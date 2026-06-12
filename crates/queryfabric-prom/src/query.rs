//! Per-query metrics and labels.

#![warn(missing_docs)]

use std::fmt::Write;
use std::sync::Arc;

use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue, LabelValueEncoder};
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};

use crate::MetricsRegistry;

/// Query wall-clock latency histogram.
///
/// Buckets start at 1 ms and grow by 2.5x for 17 buckets, covering
/// interactive and isolated query durations on the same axis.
#[must_use]
pub fn query_duration_histogram() -> Histogram {
    Histogram::new(exponential_buckets(0.001, 2.5, 17))
}

/// Query peak-memory histogram.
///
/// Buckets start at 1 KiB and grow by 4x for 13 buckets, covering up to
/// roughly 64 GiB.
#[must_use]
pub fn query_memory_histogram() -> Histogram {
    Histogram::new(exponential_buckets(1024.0, 4.0, 13))
}

/// Query rows-read histogram.
///
/// Buckets start at 1 row and grow by 10x for 10 buckets, covering up to
/// 1 billion rows.
#[must_use]
pub fn query_rows_histogram() -> Histogram {
    Histogram::new(exponential_buckets(1.0, 10.0, 10))
}

/// High-level query execution plan classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanKind {
    /// Query executed directly on the local node.
    Direct,
    /// Query fan-out across multiple remote nodes.
    ScatterGather,
    /// Query executed in an isolated context.
    Isolated,
}

impl PlanKind {
    /// Return the stable Prometheus label value for this plan kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::ScatterGather => "scatter_gather",
            Self::Isolated => "isolated",
        }
    }
}

impl EncodeLabelValue for PlanKind {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        encoder.write_str(self.as_str())
    }
}

/// High-level query outcome classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// Query completed successfully.
    Ok,
    /// Query failed.
    Error,
    /// Query was cancelled before completion.
    Cancelled,
}

impl Outcome {
    /// Return the stable Prometheus label value for this outcome.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }
}

impl EncodeLabelValue for Outcome {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        encoder.write_str(self.as_str())
    }
}

/// Labels shared by the query metric families.
#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct QueryLabels {
    /// Query execution-plan class.
    pub plan_kind: PlanKind,
    /// Query completion outcome.
    pub outcome: Outcome,
}

/// Registered query metric families.
#[derive(Debug)]
pub struct QueryMetrics {
    /// Query wall-clock durations.
    pub duration: Family<QueryLabels, Histogram>,
    /// Query peak-memory observations.
    pub memory: Family<QueryLabels, Histogram>,
    /// Query rows-read observations.
    pub rows: Family<QueryLabels, Histogram>,
}

impl QueryMetrics {
    /// Register the query metric families into `registry`.
    #[must_use]
    pub fn register(registry: &MetricsRegistry) -> Arc<Self> {
        let duration =
            Family::<QueryLabels, Histogram>::new_with_constructor(query_duration_histogram);
        let memory = Family::<QueryLabels, Histogram>::new_with_constructor(query_memory_histogram);
        let rows = Family::<QueryLabels, Histogram>::new_with_constructor(query_rows_histogram);

        registry.register(
            "query_duration_seconds",
            "Query wall-clock by plan kind and outcome.",
            duration.clone(),
        );
        registry.register(
            "query_memory_bytes",
            "Query peak memory by plan kind and outcome.",
            memory.clone(),
        );
        registry.register(
            "query_rows_read",
            "Rows scanned/read by plan kind and outcome.",
            rows.clone(),
        );

        Arc::new(Self {
            duration,
            memory,
            rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_histogram_buckets(
        constructor: fn() -> Histogram,
        metric_name: &str,
        buckets: &[&str],
    ) {
        let registry = MetricsRegistry::with_prefix("test");
        let histogram = constructor();
        registry.register(metric_name, "test histogram", histogram.clone());
        histogram.observe(f64::MAX);

        let text = registry.render().expect("render metrics");
        for bucket in buckets {
            assert!(
                text.contains(&format!("{metric_name}_bucket{{le=\"{bucket}\"}} 0")),
                "missing bucket {bucket} in:\n{text}"
            );
        }
        assert!(
            text.contains(&format!("{metric_name}_bucket{{le=\"+Inf\"}} 1")),
            "missing +Inf bucket in:\n{text}"
        );
    }

    #[test]
    fn query_histogram_bucket_boundaries_serialize_as_expected() {
        assert_histogram_buckets(
            query_duration_histogram,
            "query_duration_seconds",
            &[
                "0.001",
                "0.0025",
                "0.00625",
                "0.015625",
                "0.0390625",
                "0.09765625",
                "0.244140625",
                "0.6103515625",
                "1.52587890625",
                "3.814697265625",
                "9.5367431640625",
                "23.84185791015625",
                "59.604644775390628",
                "149.01161193847657",
                "372.5290298461914",
                "931.3225746154785",
                "2328.3064365386965",
            ],
        );
        assert_histogram_buckets(
            query_memory_histogram,
            "query_memory_bytes",
            &[
                "1024.0",
                "4096.0",
                "16384.0",
                "65536.0",
                "262144.0",
                "1048576.0",
                "4194304.0",
                "16777216.0",
                "67108864.0",
                "268435456.0",
                "1073741824.0",
                "4294967296.0",
                "17179869184.0",
            ],
        );
        assert_histogram_buckets(
            query_rows_histogram,
            "query_rows_read",
            &[
                "1.0",
                "10.0",
                "100.0",
                "1000.0",
                "10000.0",
                "100000.0",
                "1000000.0",
                "10000000.0",
                "100000000.0",
                "1000000000.0",
            ],
        );
    }

    #[test]
    fn query_labels_encode_as_lowercase_prometheus_values() {
        let registry = MetricsRegistry::with_prefix("test");
        let query_metrics = QueryMetrics::register(&registry);
        let labels = QueryLabels {
            plan_kind: PlanKind::ScatterGather,
            outcome: Outcome::Cancelled,
        };

        query_metrics.duration.get_or_create(&labels).observe(1.0);

        let text = registry.render().expect("render metrics");
        assert!(
            text.contains("test_query_duration_seconds_count{plan_kind=\"scatter_gather\",outcome=\"cancelled\"} 1"),
            "unexpected labels in:\n{text}"
        );
    }

    #[test]
    fn registered_query_metrics_render_help_and_type_without_observations() {
        let registry = MetricsRegistry::with_prefix("syndb");
        let _query_metrics = QueryMetrics::register(&registry);

        let text = registry.render().expect("render metrics");
        assert!(text.contains("# HELP syndb_query_duration_seconds"));
        assert!(text.contains("# TYPE syndb_query_duration_seconds histogram"));
        assert!(text.contains("# HELP syndb_query_memory_bytes"));
        assert!(text.contains("# TYPE syndb_query_memory_bytes histogram"));
        assert!(text.contains("# HELP syndb_query_rows_read"));
        assert!(text.contains("# TYPE syndb_query_rows_read histogram"));
        assert!(!text.contains("syndb_query_duration_seconds_count{"));
    }
}
