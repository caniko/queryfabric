#![allow(missing_docs)]
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Configuration for polling-based readiness probes.
/// Configuration for polling-based readiness probes.
#[derive(Debug, Clone)]
pub struct WaitConfig {
    /// Delay between successive connection attempts.
    pub poll_interval: Duration,
    /// Maximum number of attempts before giving up.
    pub max_attempts: usize,
}

impl Default for WaitConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
            max_attempts: 30,
        }
    }
}

/// Wait until a TCP port is accepting connections.
pub fn wait_for_tcp_port<A: ToSocketAddrs>(addr: A, config: &WaitConfig) -> Result<(), String> {
    let addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {e}"))?
        .next()
        .ok_or_else(|| "No address resolved".to_owned())?;

    for attempt in 1..=config.max_attempts {
        match TcpStream::connect_timeout(&addr, config.poll_interval) {
            Ok(_) => return Ok(()),
            Err(ref e)
                if e.kind() == ErrorKind::ConnectionRefused || e.kind() == ErrorKind::TimedOut =>
            {
                if attempt == config.max_attempts {
                    return Err(format!(
                        "Timed out waiting for TCP port {addr} after {attempt} attempts"
                    ));
                }
                std::thread::sleep(config.poll_interval);
            }
            Err(e) => return Err(format!("TCP connect error: {e}")),
        }
    }
    Ok(())
}
