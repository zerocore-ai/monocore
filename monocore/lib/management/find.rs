use tokio::net::TcpListener;

use crate::{MonocoreError, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Find the next available port starting from the provided port number
pub async fn find_available_port(host: &str, start_port: u16) -> MonocoreResult<u16> {
    const MAX_PORT_ATTEMPTS: u16 = 100;
    let end_port = start_port + MAX_PORT_ATTEMPTS - 1;

    for port in start_port..=end_port {
        match TcpListener::bind((host, port)).await {
            Ok(_) => return Ok(port),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => return Err(MonocoreError::Io(e)),
        }
    }

    Err(MonocoreError::NoAvailablePorts {
        host: host.to_string(),
        start: start_port,
        end: end_port,
    })
}
