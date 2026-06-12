//! Prometheus/OpenMetrics instrumentation shared across SynDB services.
//!
//! Built on top of the current crate, which supplies the
//! histogram bucket presets and the `MetricsRegistry` wrapper.

#![warn(missing_docs)]

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, LazyLock};

use crate::{
    MetricsRegistry, QueryMetrics, bytes_histogram, job_duration_histogram, latency_histogram,
    unix_timestamp_seconds,
};
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;

/// Labels for HTTP request metrics.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct HttpLabels {
    /// HTTP method.
    pub method: String,
    /// Matched route or handler label.
    pub route: String,
    /// HTTP status code as a string.
    pub status: String,
}

/// Labels for Arrow Flight request metrics.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct FlightLabels {
    /// Logical Flight operation name.
    pub operation: String,
    /// Operation status bucket.
    pub status: String,
}

/// Labels for per-table Arrow Flight counters.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct TableLabels {
    /// Logical Flight operation name.
    pub operation: String,
    /// Table identifier.
    pub table: String,
}

/// Labels that track only an outcome string.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct OutcomeLabels {
    /// Outcome bucket.
    pub outcome: String,
}

/// Labels for job metrics.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct JobLabels {
    /// Job kind.
    pub kind: String,
    /// Job state or terminal status.
    pub status: String,
}

/// Labels for provenance-write metrics.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct ProvenanceLabels {
    /// Provenance activity kind.
    pub activity: String,
    /// Source subsystem.
    pub source: String,
    /// Success/failure bucket.
    pub status: String,
}

/// Labels for dependency-probe metrics.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct DependencyLabels {
    /// Dependency name.
    pub dependency: String,
}

/// Labels for cached cluster-health counts.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct ClusterHealthLabels {
    /// Health-status bucket.
    pub status: String,
}

/// Labels for shard-capacity forecast utilization gauges.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct ShardCapacityLabels {
    /// Forecast shard label. Currently `all` because the forecast is
    /// cluster-wide until per-shard row accounting is added.
    pub shard: String,
    /// Capacity table-family bucket.
    pub table_family: String,
}

/// Labels for build-info gauges.
#[derive(Clone, Debug, Eq, Hash, PartialEq, EncodeLabelSet)]
pub struct BuildInfoLabels {
    /// Service name.
    pub service: String,
    /// Build or release version.
    pub version: String,
}

/// Lazily initialized metric registry plus all SynDB metric families.
#[derive(Debug)]
pub struct SynDbMetrics {
    registry: MetricsRegistry,
    http_requests: Family<HttpLabels, Counter>,
    http_duration_seconds: Family<HttpLabels, Histogram>,
    http_rate_limited_total: Counter,
    http_timeouts_total: Counter,
    flight_requests: Family<FlightLabels, Counter>,
    flight_duration_seconds: Family<FlightLabels, Histogram>,
    flight_active_streams: Family<FlightLabels, Gauge>,
    flight_rows_total: Family<TableLabels, Counter>,
    flight_bytes_total: Family<TableLabels, Counter>,
    flight_failures_total: Family<OutcomeLabels, Counter>,
    federation_delegations_total: Family<OutcomeLabels, Counter>,
    federation_delegation_duration_seconds: Family<OutcomeLabels, Histogram>,
    federation_remote_rows_total: Counter,
    federation_contributing_clusters_total: Counter,
    cluster_health: Family<ClusterHealthLabels, Gauge>,
    job_events_total: Family<JobLabels, Counter>,
    job_state: Family<JobLabels, Gauge>,
    job_duration_seconds: Family<JobLabels, Histogram>,
    job_result_bytes: Family<JobLabels, Histogram>,
    query_metrics: Arc<QueryMetrics>,
    provenance_writes_total: Family<ProvenanceLabels, Counter>,
    provenance_last_success_unixtime: Gauge,
    dependency_up: Family<DependencyLabels, Gauge>,
    dependency_probe_duration_seconds: Family<DependencyLabels, Gauge<f64, AtomicU64>>,
    dependency_last_success_unixtime: Family<DependencyLabels, Gauge>,
    shard_capacity_utilization_ratio: Family<ShardCapacityLabels, Gauge<f64, AtomicU64>>,
    build_info: Family<BuildInfoLabels, Gauge>,
}

/// Global singleton containing the shared SynDB metrics registry.
pub static METRICS: LazyLock<SynDbMetrics> = LazyLock::new(SynDbMetrics::new);

impl SynDbMetrics {
    fn new() -> Self {
        let registry = MetricsRegistry::with_prefix("syndb");
        let http_requests = Family::<HttpLabels, Counter>::default();
        let http_duration_seconds =
            Family::<HttpLabels, Histogram>::new_with_constructor(latency_histogram);
        let http_rate_limited_total = Counter::default();
        let http_timeouts_total = Counter::default();
        let flight_requests = Family::<FlightLabels, Counter>::default();
        let flight_duration_seconds =
            Family::<FlightLabels, Histogram>::new_with_constructor(latency_histogram);
        let flight_active_streams = Family::<FlightLabels, Gauge>::default();
        let flight_rows_total = Family::<TableLabels, Counter>::default();
        let flight_bytes_total = Family::<TableLabels, Counter>::default();
        let flight_failures_total = Family::<OutcomeLabels, Counter>::default();
        let federation_delegations_total = Family::<OutcomeLabels, Counter>::default();
        let federation_delegation_duration_seconds =
            Family::<OutcomeLabels, Histogram>::new_with_constructor(latency_histogram);
        let federation_remote_rows_total = Counter::default();
        let federation_contributing_clusters_total = Counter::default();
        let cluster_health = Family::<ClusterHealthLabels, Gauge>::default();
        let job_events_total = Family::<JobLabels, Counter>::default();
        let job_state = Family::<JobLabels, Gauge>::default();
        let job_duration_seconds =
            Family::<JobLabels, Histogram>::new_with_constructor(job_duration_histogram);
        let job_result_bytes =
            Family::<JobLabels, Histogram>::new_with_constructor(bytes_histogram);
        let query_metrics = QueryMetrics::register(&registry);
        let provenance_writes_total = Family::<ProvenanceLabels, Counter>::default();
        let provenance_last_success_unixtime = Gauge::default();
        let dependency_up = Family::<DependencyLabels, Gauge>::default();
        let dependency_probe_duration_seconds =
            Family::<DependencyLabels, Gauge<f64, AtomicU64>>::default();
        let dependency_last_success_unixtime = Family::<DependencyLabels, Gauge>::default();
        let shard_capacity_utilization_ratio =
            Family::<ShardCapacityLabels, Gauge<f64, AtomicU64>>::default();
        let build_info = Family::<BuildInfoLabels, Gauge>::default();

        registry.register(
            "http_requests_total",
            "HTTP requests handled by the API.",
            http_requests.clone(),
        );
        registry.register(
            "http_request_duration_seconds",
            "HTTP request duration.",
            http_duration_seconds.clone(),
        );
        registry.register(
            "http_rate_limited_total",
            "HTTP requests rejected by the per-IP rate limiter.",
            http_rate_limited_total.clone(),
        );
        registry.register(
            "http_timeouts_total",
            "HTTP requests that returned a timeout response.",
            http_timeouts_total.clone(),
        );
        registry.register(
            "flight_requests_total",
            "Arrow Flight requests handled by operation and status.",
            flight_requests.clone(),
        );
        registry.register(
            "flight_request_duration_seconds",
            "Arrow Flight request duration by operation and status.",
            flight_duration_seconds.clone(),
        );
        registry.register(
            "flight_active_streams",
            "Active Arrow Flight streams by operation.",
            flight_active_streams.clone(),
        );
        registry.register(
            "flight_rows_total",
            "Rows moved through Arrow Flight by operation and table.",
            flight_rows_total.clone(),
        );
        registry.register(
            "flight_bytes_total",
            "Approximate bytes moved through Arrow Flight by operation and table.",
            flight_bytes_total.clone(),
        );
        registry.register(
            "flight_failures_total",
            "Arrow Flight data-plane failures by coarse outcome.",
            flight_failures_total.clone(),
        );
        registry.register(
            "federation_delegations_total",
            "Federation delegation attempts by outcome.",
            federation_delegations_total.clone(),
        );
        registry.register(
            "federation_delegation_duration_seconds",
            "Federation delegation duration by outcome.",
            federation_delegation_duration_seconds.clone(),
        );
        registry.register(
            "federation_remote_rows_total",
            "Rows returned by successful federation delegations.",
            federation_remote_rows_total.clone(),
        );
        registry.register(
            "federation_contributing_clusters_total",
            "Total contributing cluster observations across federation reads.",
            federation_contributing_clusters_total.clone(),
        );
        registry.register(
            "federation_cluster_health",
            "Federation cluster health cache counts by status.",
            cluster_health.clone(),
        );
        registry.register(
            "job_events_total",
            "Query job lifecycle events by kind and status.",
            job_events_total.clone(),
        );
        registry.register(
            "job_state",
            "Current query job state gauges by kind and status.",
            job_state.clone(),
        );
        registry.register(
            "job_duration_seconds",
            "Query job duration by kind and final status.",
            job_duration_seconds.clone(),
        );
        registry.register(
            "job_result_bytes",
            "Query job result bytes by kind and final status.",
            job_result_bytes.clone(),
        );
        registry.register(
            "provenance_writes_total",
            "Operational provenance writes by activity, source, and status.",
            provenance_writes_total.clone(),
        );
        registry.register(
            "provenance_last_success_unixtime",
            "Unix timestamp of the latest successful operational provenance write.",
            provenance_last_success_unixtime.clone(),
        );
        registry.register(
            "dependency_up",
            "Cached dependency health probe result.",
            dependency_up.clone(),
        );
        registry.register(
            "dependency_probe_duration_seconds",
            "Cached dependency health probe duration.",
            dependency_probe_duration_seconds.clone(),
        );
        registry.register(
            "dependency_last_success_unixtime",
            "Unix timestamp of latest successful dependency probe.",
            dependency_last_success_unixtime.clone(),
        );
        registry.register(
            "shard_capacity_utilization_ratio",
            "Latest shard-capacity utilization ratio by shard and table family.",
            shard_capacity_utilization_ratio.clone(),
        );
        registry.register(
            "build_info",
            "SynDB service build information.",
            build_info.clone(),
        );

        Self {
            registry,
            http_requests,
            http_duration_seconds,
            http_rate_limited_total,
            http_timeouts_total,
            flight_requests,
            flight_duration_seconds,
            flight_active_streams,
            flight_rows_total,
            flight_bytes_total,
            flight_failures_total,
            federation_delegations_total,
            federation_delegation_duration_seconds,
            federation_remote_rows_total,
            federation_contributing_clusters_total,
            cluster_health,
            job_events_total,
            job_state,
            job_duration_seconds,
            job_result_bytes,
            query_metrics,
            provenance_writes_total,
            provenance_last_success_unixtime,
            dependency_up,
            dependency_probe_duration_seconds,
            dependency_last_success_unixtime,
            shard_capacity_utilization_ratio,
            build_info,
        }
    }

    /// Render all registered metrics to OpenMetrics text format.
    ///
    /// # Errors
    /// Returns any text-encoding error reported by the underlying registry.
    pub fn render(&self) -> Result<String, std::fmt::Error> {
        self.registry.render()
    }
}

/// Render all global metrics to OpenMetrics text format.
///
/// # Errors
/// Returns any text-encoding error reported by the underlying registry.
pub fn render() -> Result<String, std::fmt::Error> {
    METRICS.render()
}

/// Return a shared handle to the query-specific metric families.
#[must_use]
pub fn query_metrics() -> Arc<QueryMetrics> {
    Arc::clone(&METRICS.query_metrics)
}

/// Record the currently running service/version pair.
pub fn record_build_info(service: &str, version: &str) {
    METRICS
        .build_info
        .get_or_create(&BuildInfoLabels {
            service: service.to_owned(),
            version: version.to_owned(),
        })
        .set(1);
}

/// Record one HTTP request and its latency.
pub fn record_http_request(method: &str, route: &str, status: u16, duration_seconds: f64) {
    let labels = HttpLabels {
        method: method.to_owned(),
        route: route.to_owned(),
        status: status.to_string(),
    };
    METRICS.http_requests.get_or_create(&labels).inc();
    METRICS
        .http_duration_seconds
        .get_or_create(&labels)
        .observe(duration_seconds);
    if status == 408 {
        METRICS.http_timeouts_total.inc();
    }
}

/// Record one request rejected by the rate limiter.
pub fn record_rate_limited() {
    METRICS.http_rate_limited_total.inc();
}

/// Record one Arrow Flight request and its latency.
pub fn record_flight_request(operation: &str, status: &str, duration_seconds: f64) {
    let labels = FlightLabels {
        operation: operation.to_owned(),
        status: status.to_owned(),
    };
    METRICS.flight_requests.get_or_create(&labels).inc();
    METRICS
        .flight_duration_seconds
        .get_or_create(&labels)
        .observe(duration_seconds);
}

/// Increment the count of active Flight streams for `operation`.
pub fn inc_flight_active(operation: &str) {
    METRICS
        .flight_active_streams
        .get_or_create(&FlightLabels {
            operation: operation.to_owned(),
            status: "active".to_owned(),
        })
        .inc();
}

/// Decrement the count of active Flight streams for `operation`.
pub fn dec_flight_active(operation: &str) {
    METRICS
        .flight_active_streams
        .get_or_create(&FlightLabels {
            operation: operation.to_owned(),
            status: "active".to_owned(),
        })
        .dec();
}

/// Add `rows` to the per-table Flight row counter.
pub fn add_flight_rows(operation: &str, table: &str, rows: u64) {
    METRICS
        .flight_rows_total
        .get_or_create(&TableLabels {
            operation: operation.to_owned(),
            table: table.to_owned(),
        })
        .inc_by(rows);
}

/// Add `bytes` to the per-table Flight byte counter.
pub fn add_flight_bytes(operation: &str, table: &str, bytes: u64) {
    METRICS
        .flight_bytes_total
        .get_or_create(&TableLabels {
            operation: operation.to_owned(),
            table: table.to_owned(),
        })
        .inc_by(bytes);
}

/// Record one coarse-grained Flight failure outcome.
pub fn record_flight_failure(outcome: &str) {
    METRICS
        .flight_failures_total
        .get_or_create(&OutcomeLabels {
            outcome: outcome.to_owned(),
        })
        .inc();
}

/// Record a federation delegation attempt and its latency.
pub fn record_federation_delegation(outcome: &str, duration_seconds: f64) {
    let labels = OutcomeLabels {
        outcome: outcome.to_owned(),
    };
    METRICS
        .federation_delegations_total
        .get_or_create(&labels)
        .inc();
    METRICS
        .federation_delegation_duration_seconds
        .get_or_create(&labels)
        .observe(duration_seconds);
}

/// Add `rows` to the successful federation remote-row counter.
pub fn add_federation_remote_rows(rows: u64) {
    METRICS.federation_remote_rows_total.inc_by(rows);
}

/// Add `count` to the federation contributing-clusters counter.
pub fn add_federation_contributing_clusters(count: u64) {
    METRICS.federation_contributing_clusters_total.inc_by(count);
}

/// Set the cached count for a particular cluster-health status bucket.
pub fn set_cluster_health(status: &str, count: i64) {
    METRICS
        .cluster_health
        .get_or_create(&ClusterHealthLabels {
            status: status.to_owned(),
        })
        .set(count);
}

/// Record one job lifecycle event.
pub fn record_job_event(kind: &str, status: &str) {
    METRICS
        .job_events_total
        .get_or_create(&JobLabels {
            kind: kind.to_owned(),
            status: status.to_owned(),
        })
        .inc();
}

/// Set the current job-state gauge for a kind/status bucket.
pub fn set_job_state(kind: &str, status: &str, count: i64) {
    METRICS
        .job_state
        .get_or_create(&JobLabels {
            kind: kind.to_owned(),
            status: status.to_owned(),
        })
        .set(count);
}

/// Observe one completed job duration.
pub fn observe_job_duration(kind: &str, status: &str, duration_seconds: f64) {
    METRICS
        .job_duration_seconds
        .get_or_create(&JobLabels {
            kind: kind.to_owned(),
            status: status.to_owned(),
        })
        .observe(duration_seconds);
}

/// Observe the output size for one completed job.
pub fn observe_job_result_bytes(kind: &str, status: &str, bytes: u64) {
    METRICS
        .job_result_bytes
        .get_or_create(&JobLabels {
            kind: kind.to_owned(),
            status: status.to_owned(),
        })
        .observe(bytes as f64);
}

/// Record one provenance write and update the success timestamp on success.
pub fn record_provenance_write(activity: &str, source: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    METRICS
        .provenance_writes_total
        .get_or_create(&ProvenanceLabels {
            activity: activity.to_owned(),
            source: source.to_owned(),
            status: status.to_owned(),
        })
        .inc();
    if success {
        METRICS
            .provenance_last_success_unixtime
            .set(unix_timestamp_seconds() as i64);
    }
}

/// Update cached dependency probe state, latency, and last-success timestamp.
pub fn set_dependency_probe(dependency: &str, up: bool, duration_seconds: f64) {
    let labels = DependencyLabels {
        dependency: dependency.to_owned(),
    };
    METRICS
        .dependency_up
        .get_or_create(&labels)
        .set(if up { 1 } else { 0 });
    METRICS
        .dependency_probe_duration_seconds
        .get_or_create(&labels)
        .set(duration_seconds);
    if up {
        METRICS
            .dependency_last_success_unixtime
            .get_or_create(&labels)
            .set(unix_timestamp_seconds() as i64);
    }
}

/// Set a cached capacity utilization ratio from the daily forecast.
pub fn set_shard_capacity_utilization_ratio(
    shard: &str,
    table_family: &str,
    utilization_ratio: f64,
) {
    METRICS
        .shard_capacity_utilization_ratio
        .get_or_create(&ShardCapacityLabels {
            shard: shard.to_owned(),
            table_family: table_family.to_owned(),
        })
        .set(utilization_ratio);
}
