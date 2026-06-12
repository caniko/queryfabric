use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use futures::{FutureExt, future::join_all};
use queryfabric_contract::{ClusterProbe, Health, NodeId, ProbeResult};
use thespis::Actor;
use thespis::actor::{ActorRef, Spawn};
use thespis::error::Infallible;
use thespis::message::{Context, Message};
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::health::HealthCache;
use crate::registry::{ClusterRefs, ClusterRemoteHandle};

fn spawn_traced(name: &'static str, future: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(async move {
        if AssertUnwindSafe(future).catch_unwind().await.is_err() {
            tracing::error!(task = name, "background task panicked");
        }
    });
}

/// Messages for the generic health monitor actor.
#[derive(Debug, Clone, Copy)]
pub struct CheckAllClusters;

/// Request the cached health for a single cluster id.
#[derive(Debug, Clone)]
pub struct GetHealth<C = NodeId>(pub C);

/// Reset a cluster's circuit breaker to Closed, allowing immediate probing.
#[derive(Debug, Clone)]
pub struct ResetCircuitBreaker<C = NodeId>(pub C);

/// Circuit breaker tuning.
#[derive(Debug, Clone, Copy)]
pub struct CircuitConfig {
    /// Number of consecutive failures required to open the circuit.
    pub open_threshold: u32,
    /// Cooldown period before an open circuit may probe again.
    pub cooldown: Duration,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            open_threshold: 3,
            cooldown: Duration::from_secs(30),
        }
    }
}

/// Per-cluster circuit breaker tracking.
#[derive(Debug, Clone)]
pub struct CircuitState {
    /// Closed = normal, Open = reject, HalfOpen = probing one request.
    pub phase: CircuitPhase,
    /// Running count of consecutive failures (resets on success).
    pub consecutive_failures: u32,
    /// When the circuit transitioned to Open.
    pub opened_at: Option<Instant>,
}

/// Current circuit-breaker state for one cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitPhase {
    /// Normal operation; probes are allowed.
    Closed,
    /// Circuit is open; probes are suppressed until cooldown expires.
    Open,
    /// Circuit is testing recovery with a probe after cooldown.
    HalfOpen,
}

impl CircuitState {
    /// Construct a closed circuit with zero recorded failures.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: CircuitPhase::Closed,
            consecutive_failures: 0,
            opened_at: None,
        }
    }

    /// Should we skip probing this cluster this sweep?
    pub fn should_skip_probe(&self, config: CircuitConfig) -> bool {
        match self.phase {
            CircuitPhase::Closed | CircuitPhase::HalfOpen => false,
            CircuitPhase::Open => match self.opened_at {
                Some(opened) => opened.elapsed() < config.cooldown,
                None => false,
            },
        }
    }

    /// Attempt transition from Open to HalfOpen when cooldown expires.
    pub fn maybe_transition_to_half_open(&mut self, config: CircuitConfig) {
        if self.phase == CircuitPhase::Open
            && let Some(opened) = self.opened_at
            && opened.elapsed() >= config.cooldown
        {
            debug!("Circuit transitioning Open -> HalfOpen");
            self.phase = CircuitPhase::HalfOpen;
        }
    }

    /// Record a successful probe and close the circuit.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.opened_at = None;
        self.phase = CircuitPhase::Closed;
    }

    /// Record a failed probe and open the circuit after the configured threshold.
    pub fn record_failure(&mut self, config: CircuitConfig) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= config.open_threshold {
            self.phase = CircuitPhase::Open;
            self.opened_at = Some(Instant::now());
        }
    }
}

impl Default for CircuitState {
    fn default() -> Self {
        Self::new()
    }
}

/// Arguments for constructing [`HealthMonitorActor`].
pub struct HealthMonitorArgs<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    /// Known remote clusters to probe.
    pub cluster_refs: ClusterRefs<C>,
    /// Interval between health sweeps.
    pub check_interval: Duration,
    /// Shared cache updated with the latest probe results.
    pub health_cache: HealthCache<C>,
    /// Timeout for each individual probe.
    pub probe_timeout: Duration,
    /// Circuit-breaker tuning.
    pub circuit_config: CircuitConfig,
    /// Domain-specific probe implementation.
    pub probe: P,
}

/// Generic actor that periodically probes cluster health through a domain adapter.
pub struct HealthMonitorActor<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    cluster_refs: ClusterRefs<C>,
    health_cache: HealthCache<C>,
    probe_timeout: Duration,
    circuit_config: CircuitConfig,
    circuits: HashMap<C, CircuitState>,
    probe: P,
}

impl<C, P> HealthMonitorActor<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    /// Get a clone of the shared health cache for read access from handlers.
    #[must_use]
    pub fn health_cache(&self) -> HealthCache<C> {
        Arc::clone(&self.health_cache)
    }

    /// Spawn the actor and start its periodic health check loop.
    pub async fn spawn_with_refs(
        cluster_refs: ClusterRefs<C>,
        check_interval: Duration,
        probe: P,
    ) -> (ActorRef<Self>, HealthCache<C>) {
        Self::spawn_with_args(HealthMonitorArgs {
            cluster_refs,
            check_interval,
            health_cache: Arc::new(papaya::HashMap::new()),
            probe_timeout: Duration::from_secs(15),
            circuit_config: CircuitConfig::default(),
            probe,
        })
        .await
    }

    /// Spawn the actor with explicit cache, timeout, and circuit settings.
    pub async fn spawn_with_args(
        args: HealthMonitorArgs<C, P>,
    ) -> (ActorRef<Self>, HealthCache<C>) {
        let tick_interval = args.check_interval;
        let cache = Arc::clone(&args.health_cache);
        let actor_ref = Self::spawn(args);

        let ref_clone = actor_ref.clone();
        spawn_traced("health-monitor-ticker", async move {
            let mut tick = tokio::time::interval(tick_interval);
            loop {
                tick.tick().await;
                if ref_clone.tell(CheckAllClusters).send().await.is_err() {
                    break;
                }
            }
        });

        (actor_ref, cache)
    }
}

impl<C, P> Actor for HealthMonitorActor<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    type Args = HealthMonitorArgs<C, P>;
    type Error = Infallible;

    async fn on_start(args: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        info!(
            "HealthMonitorActor started (interval: {:?}, probe_timeout: {:?})",
            args.check_interval, args.probe_timeout
        );
        Ok(Self {
            cluster_refs: args.cluster_refs,
            health_cache: args.health_cache,
            probe_timeout: args.probe_timeout,
            circuit_config: args.circuit_config,
            circuits: HashMap::new(),
            probe: args.probe,
        })
    }
}

impl<C, P> Message<CheckAllClusters> for HealthMonitorActor<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    type Reply = ();

    async fn handle(
        &mut self,
        _msg: CheckAllClusters,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let entries: Vec<_> = {
            let guard = self.cluster_refs.guard();
            self.cluster_refs
                .iter(&guard)
                .map(|(id, handle)| (id.clone(), handle.clone()))
                .collect()
        };

        if entries.is_empty() {
            return;
        }

        let mut to_probe = Vec::with_capacity(entries.len());
        let mut skipped = Vec::new();
        for (cluster_id, handle) in entries {
            let circuit = self.circuits.entry(cluster_id.clone()).or_default();

            circuit.maybe_transition_to_half_open(self.circuit_config);

            if circuit.should_skip_probe(self.circuit_config) {
                debug!(
                    cluster = %handle.cluster_name,
                    consecutive_failures = circuit.consecutive_failures,
                    "Circuit open - skipping probe"
                );
                skipped.push((cluster_id, handle));
            } else {
                to_probe.push((cluster_id, handle));
            }
        }

        let probe_timeout = self.probe_timeout;
        let probe = self.probe.clone();
        let futures: Vec<_> = to_probe
            .into_iter()
            .map(|(cluster_id, handle)| {
                let probe = probe.clone();
                async move {
                    let cluster_name = handle.cluster_name.clone();
                    let result = match tokio::time::timeout(
                        probe_timeout,
                        probe.probe(cluster_id.clone(), handle),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => {
                            warn!(
                                cluster = %cluster_name,
                                timeout_ms = probe_timeout.as_millis() as u64,
                                "Health probe timed out"
                            );
                            ProbeResult {
                                health: Health::Unreachable,
                                output: P::Output::default(),
                            }
                        }
                    };
                    (cluster_id, cluster_name, result)
                }
            })
            .collect();

        let results = join_all(futures).await;
        let mut successful_outputs = Vec::new();
        {
            let guard = self.health_cache.guard();

            for (cluster_id, cluster_name, result) in results {
                self.health_cache
                    .insert(cluster_id.clone(), result.health, &guard);

                let circuit = self.circuits.entry(cluster_id).or_default();

                if result.health == Health::Unreachable {
                    circuit.record_failure(self.circuit_config);
                    warn!(
                        cluster = %cluster_name,
                        consecutive_failures = circuit.consecutive_failures,
                        circuit = ?circuit.phase,
                        "Cluster unreachable during health check"
                    );
                } else {
                    if circuit.phase != CircuitPhase::Closed {
                        info!(cluster = %cluster_name, "Circuit closed - cluster recovered");
                    }
                    circuit.record_success();
                    successful_outputs.push(result.output);
                }
            }

            for (cluster_id, handle) in skipped {
                self.health_cache
                    .insert(cluster_id, Health::Unreachable, &guard);
                debug!(
                    cluster = %handle.cluster_name,
                    "Kept Unreachable (circuit open)"
                );
            }
        }

        if !successful_outputs.is_empty() {
            self.probe.on_successful_sweep(successful_outputs).await;
        }
    }
}

impl<C, P> Message<GetHealth<C>> for HealthMonitorActor<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    type Reply = Option<Health>;

    async fn handle(
        &mut self,
        msg: GetHealth<C>,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let guard = self.health_cache.guard();
        self.health_cache.get(&msg.0, &guard).cloned()
    }
}

impl<C, P> Message<ResetCircuitBreaker<C>> for HealthMonitorActor<C, P>
where
    C: Clone + Eq + Hash + Send + Sync + 'static,
    P: ClusterProbe<C, ClusterRemoteHandle<C>>,
{
    type Reply = ();

    async fn handle(
        &mut self,
        msg: ResetCircuitBreaker<C>,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        if let Some(circuit) = self.circuits.get_mut(&msg.0)
            && circuit.phase != CircuitPhase::Closed
        {
            info!(
                old_phase = ?circuit.phase,
                "Circuit breaker reset on cluster registration"
            );
            *circuit = CircuitState::new();
        }
        let guard = self.health_cache.guard();
        self.health_cache.remove(&msg.0, &guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_starts_closed() {
        let circuit = CircuitState::new();
        assert_eq!(circuit.phase, CircuitPhase::Closed);
        assert_eq!(circuit.consecutive_failures, 0);
        assert!(circuit.opened_at.is_none());
    }

    #[test]
    fn failure_threshold_opens_circuit() {
        let config = CircuitConfig {
            open_threshold: 2,
            cooldown: Duration::from_secs(30),
        };
        let mut circuit = CircuitState::new();
        circuit.record_failure(config);
        assert_eq!(circuit.phase, CircuitPhase::Closed);
        circuit.record_failure(config);
        assert_eq!(circuit.phase, CircuitPhase::Open);
        assert!(circuit.opened_at.is_some());
    }

    #[test]
    fn success_closes_circuit() {
        let config = CircuitConfig {
            open_threshold: 1,
            cooldown: Duration::from_secs(30),
        };
        let mut circuit = CircuitState::new();
        circuit.record_failure(config);
        circuit.record_success();
        assert_eq!(circuit.phase, CircuitPhase::Closed);
        assert_eq!(circuit.consecutive_failures, 0);
        assert!(circuit.opened_at.is_none());
    }
}
