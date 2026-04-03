//! SOCKS5 server-side protocol handling (RFC 1928 subset).
//!
//! Implements only the CONNECT command with NO AUTH method.
//! Used by dynamic port forwarding (`ssh -D`) to parse browser proxy requests.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// SOCKS5 protocol error.
#[derive(Debug, thiserror::Error)]
pub enum Socks5Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid SOCKS version: {0}")]
    InvalidVersion(u8),
    #[error("no acceptable auth method")]
    NoAcceptableMethod,
    #[error("unsupported command: {0}")]
    UnsupportedCommand(u8),
    #[error("invalid address type: {0}")]
    InvalidAddressType(u8),
}

/// SOCKS5 reply codes.
pub const REPLY_SUCCESS: u8 = 0x00;
pub const REPLY_GENERAL_FAILURE: u8 = 0x01;
pub const REPLY_HOST_UNREACHABLE: u8 = 0x04;
pub const REPLY_COMMAND_NOT_SUPPORTED: u8 = 0x07;

/// Performs the SOCKS5 handshake on an accepted TCP stream.
///
/// 1. Reads method negotiation → replies NO AUTH
/// 2. Reads CONNECT request → parses target address
/// 3. Returns `(host, port)` for the caller to open an SSH channel
///
/// The caller must send the success/failure reply after attempting the connection.
pub async fn socks5_handshake(stream: &mut TcpStream) -> Result<(String, u16), Socks5Error> {
    // ── Phase 1: Method negotiation ──
    let version = stream.read_u8().await?;
    if version != 0x05 {
        return Err(Socks5Error::InvalidVersion(version));
    }

    let nmethods = stream.read_u8().await? as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    // We only support NO AUTH (0x00)
    if !methods.contains(&0x00) {
        // Reply: no acceptable method
        stream.write_all(&[0x05, 0xFF]).await?;
        return Err(Socks5Error::NoAcceptableMethod);
    }

    // Reply: selected NO AUTH
    stream.write_all(&[0x05, 0x00]).await?;

    // ── Phase 2: CONNECT request ──
    let ver = stream.read_u8().await?;
    if ver != 0x05 {
        return Err(Socks5Error::InvalidVersion(ver));
    }

    let cmd = stream.read_u8().await?;
    if cmd != 0x01 {
        // Not CONNECT — send failure and return error
        socks5_reply_failure(stream, REPLY_COMMAND_NOT_SUPPORTED).await?;
        return Err(Socks5Error::UnsupportedCommand(cmd));
    }

    let _rsv = stream.read_u8().await?; // reserved byte
    let atyp = stream.read_u8().await?;

    let host = match atyp {
        0x01 => {
            // IPv4
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            Ipv4Addr::from(addr).to_string()
        }
        0x03 => {
            // Domain name
            let len = stream.read_u8().await? as usize;
            let mut domain = vec![0u8; len];
            stream.read_exact(&mut domain).await?;
            String::from_utf8_lossy(&domain).to_string()
        }
        0x04 => {
            // IPv6
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
            Ipv6Addr::from(addr).to_string()
        }
        _ => {
            socks5_reply_failure(stream, REPLY_GENERAL_FAILURE).await?;
            return Err(Socks5Error::InvalidAddressType(atyp));
        }
    };

    let port = stream.read_u16().await?;

    Ok((host, port))
}

/// Sends a SOCKS5 CONNECT success reply (BND.ADDR = 0.0.0.0:0).
pub async fn socks5_reply_success(stream: &mut TcpStream) -> io::Result<()> {
    // VER  REP  RSV  ATYP  BND.ADDR(4)   BND.PORT(2)
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
}

/// Sends a SOCKS5 CONNECT failure reply with the given error code.
pub async fn socks5_reply_failure(stream: &mut TcpStream, code: u8) -> io::Result<()> {
    stream
        .write_all(&[0x05, code, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
}
