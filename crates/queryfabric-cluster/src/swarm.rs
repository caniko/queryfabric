//! Bootstrap a piying libp2p swarm.

#![warn(missing_docs)]

use libp2p_identity::PeerId;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Configuration for the libp2p swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    /// Listen address (e.g., `"/ip4/0.0.0.0/udp/4001/quic-v1"`).
    /// If `None`, the OS assigns a random port (suitable for dev/mDNS).
    pub listen_addr: Option<String>,

    /// Multiaddrs of known peer nodes for WAN discovery (optional).
    pub bootstrap_multiaddrs: Vec<String>,

    /// Enable mDNS for local network discovery (dev/LAN mode).
    pub enable_mdns: bool,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            listen_addr: None,
            bootstrap_multiaddrs: Vec::new(),
            enable_mdns: true,
        }
    }
}

/// Bootstrap a piying libp2p swarm with a `role` label for log context.
///
/// In dev mode (no explicit listen address), uses `piying::remote::bootstrap()`.
/// In production (explicit address), uses `piying::remote::bootstrap_on()`.
///
/// # Errors
/// Returns any piying/libp2p bootstrap error from the selected bootstrap
/// helper.
pub fn bootstrap_swarm(
    role: &str,
    config: &SwarmConfig,
) -> Result<PeerId, Box<dyn std::error::Error>> {
    let peer_id = if let Some(addr) = &config.listen_addr {
        info!(addr = %addr, role, "Bootstrapping swarm on explicit address");
        piying::remote::bootstrap_on(addr)?
    } else {
        info!(role, "Bootstrapping swarm with mDNS (dev mode)");
        piying::remote::bootstrap()?
    };

    info!(peer_id = %peer_id, role, "Swarm bootstrapped");
    Ok(peer_id)
}
