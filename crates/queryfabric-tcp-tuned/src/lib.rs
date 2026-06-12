//! TCP listener with performance-tuned socket options.
//!
//! Provides a single helper, [`tuned_tcp_listener`], that creates a
//! `std::net::TcpListener` with `TCP_NODELAY`, `SO_REUSEADDR`, non-blocking
//! mode, and 2 MB send/recv buffers — settings appropriate for high-throughput
//! streaming servers such as Arrow Flight or HTTP/2.

#![warn(missing_docs)]

use std::net::SocketAddr;

use socket2::{Domain, Protocol, Socket, Type};

/// Create a TCP listener with performance-tuned socket options.
///
/// Sets `TCP_NODELAY`, `SO_REUSEADDR`, non-blocking mode, and 2 MB send/recv
/// buffers. Backlog is fixed at 1024.
///
/// # Errors
/// Returns any socket creation, option-setting, bind, or listen error
/// reported by the OS.
pub fn tuned_tcp_listener(addr: &SocketAddr) -> std::io::Result<std::net::TcpListener> {
    let domain = if addr.is_ipv6() {
        Domain::IPV6
    } else {
        Domain::IPV4
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.set_tcp_nodelay(true)?;
    socket.set_send_buffer_size(2 * 1024 * 1024)?;
    socket.set_recv_buffer_size(2 * 1024 * 1024)?;
    socket.bind(&(*addr).into())?;
    socket.listen(1024)?;
    Ok(socket.into())
}
