//! Tor proxy detection commands.

use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Result of Tor service detection.
#[derive(serde::Serialize)]
pub struct TorStatus {
    pub running: bool,
    pub port: u16,
}

/// Detects a running Tor service on localhost.
///
/// Checks port 9050 (system Tor daemon) then 9150 (Tor Browser).
/// Uses a 1-second TCP connect timeout per port.
#[tauri::command]
pub async fn tor_detect() -> Result<TorStatus, String> {
    for port in [9050u16, 9150] {
        let addr = format!("127.0.0.1:{}", port);
        if let Ok(Ok(_)) = timeout(Duration::from_secs(1), TcpStream::connect(&addr)).await {
            return Ok(TorStatus {
                running: true,
                port,
            });
        }
    }
    Ok(TorStatus {
        running: false,
        port: 9050,
    })
}
