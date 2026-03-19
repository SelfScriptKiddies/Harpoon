use std::net::SocketAddr;

use crate::error::HarpoonError;

/// Create a UDP socket bound to the client's address with IP_TRANSPARENT.
/// This makes the upstream see the original client IP as the source.
/// Requires CAP_NET_ADMIN.
pub fn create_transparent_upstream_socket(
    client_addr: SocketAddr,
    target_addr: SocketAddr,
) -> Result<std::net::UdpSocket, HarpoonError> {
    use socket2::{Domain, Protocol, Socket, Type};

    let domain = if client_addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| HarpoonError::TransparentSocket(format!("socket creation: {e}")))?;

    // IP_TRANSPARENT allows binding to non-local addresses
    set_ip_transparent(&socket)?;

    // IP_FREEBIND allows binding to addresses not yet assigned
    set_ip_freebind(&socket)?;

    socket.set_reuse_address(true).map_err(|e| {
        HarpoonError::TransparentSocket(format!("SO_REUSEADDR: {e}"))
    })?;

    socket.set_nonblocking(true).map_err(|e| {
        HarpoonError::TransparentSocket(format!("set_nonblocking: {e}"))
    })?;

    // Bind to the client's address — upstream will see this as the source
    let bind_addr: socket2::SockAddr = client_addr.into();
    socket.bind(&bind_addr).map_err(|e| {
        HarpoonError::TransparentSocket(format!(
            "bind to {client_addr} (requires CAP_NET_ADMIN): {e}"
        ))
    })?;

    // Connect to upstream
    let target: socket2::SockAddr = target_addr.into();
    socket.connect(&target).map_err(|e| {
        HarpoonError::TransparentSocket(format!("connect to {target_addr}: {e}"))
    })?;

    Ok(socket.into())
}

fn set_ip_transparent(socket: &socket2::Socket) -> Result<(), HarpoonError> {
    use std::os::unix::io::AsRawFd;

    let fd = socket.as_raw_fd();
    let val: libc::c_int = 1;
    let ret = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_IP,
            libc::IP_TRANSPARENT,
            &val as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if ret != 0 {
        return Err(HarpoonError::TransparentSocket(format!(
            "IP_TRANSPARENT: {} (requires CAP_NET_ADMIN)",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn set_ip_freebind(socket: &socket2::Socket) -> Result<(), HarpoonError> {
    use std::os::unix::io::AsRawFd;

    let fd = socket.as_raw_fd();
    let val: libc::c_int = 1;
    let ret = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_IP,
            libc::IP_FREEBIND,
            &val as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if ret != 0 {
        return Err(HarpoonError::TransparentSocket(format!(
            "IP_FREEBIND: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}
