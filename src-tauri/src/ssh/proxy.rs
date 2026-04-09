//! Network proxy support for SSH connections.
//!
//! Provides TCP stream tunneling through SOCKS5, SOCKS4/4a, and HTTP CONNECT proxies.
//! The resulting stream is passed to `russh::client::connect_stream()` for SSH handshake.

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls;

use super::SshError;

/// Combined trait for async bidirectional streams.
/// Required because Rust doesn't allow multiple non-auto traits in `dyn` trait objects.
pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

/// Supported network proxy types.
#[derive(Debug, Clone, PartialEq)]
pub enum ProxyType {
    Socks5,
    Socks4,
    Http,
    /// Tor proxy — delegates to SOCKS5 (Tor exposes a standard SOCKS5 interface).
    Tor,
    /// ProxyCommand — spawns an external process and uses its stdin/stdout as transport.
    Command,
}

impl ProxyType {
    /// Parses from database string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "socks5" => Some(Self::Socks5),
            "socks4" => Some(Self::Socks4),
            "http" => Some(Self::Http),
            "tor" => Some(Self::Tor),
            "command" => Some(Self::Command),
            _ => None,
        }
    }

    /// Converts to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Socks5 => "socks5",
            Self::Socks4 => "socks4",
            Self::Http => "http",
            Self::Tor => "tor",
            Self::Command => "command",
        }
    }
}

/// TLS configuration for HTTPS proxies.
#[derive(Debug, Clone, Default)]
pub struct ProxyTlsConfig {
    pub enabled: bool,
    pub verify: bool,
    pub ca_cert_path: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
}

/// Configuration for a network proxy.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: ProxyTlsConfig,
    /// ProxyCommand string (only used when `proxy_type == Command`).
    pub command: Option<String>,
}

/// Connects to a target host through a network proxy, returning an async stream.
///
/// The returned stream can be passed to `russh::client::connect_stream()` for SSH handshake,
/// keeping proxy logic completely decoupled from SSH protocol.
/// Returns true if the error looks like a SOCKS5 handshake failure that
/// could be caused by the server expecting TLS (e.g., raw bytes on a TLS port).
pub fn is_socks5_tls_retryable(err: &SshError) -> bool {
    let msg = err.to_string();
    // "early eof" = server closed connection on seeing non-TLS bytes
    // "connection reset" = TCP RST from DPI/firewall
    // "unexpected eof" = similar to early eof
    msg.contains("early eof")
        || msg.contains("connection reset")
        || msg.contains("unexpected eof")
        || msg.contains("Connection reset")
}

pub async fn connect_via_proxy(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, SshError> {
    match proxy.proxy_type {
        ProxyType::Socks5 | ProxyType::Tor => {
            if proxy.tls.enabled {
                // TLS explicitly enabled: TCP → TLS → SOCKS5
                let addr = format!("{}:{}", proxy.host, proxy.port);
                let tcp = TcpStream::connect(&addr).await
                    .map_err(|e| SshError::ProxyFailed(format!("TCP connect to proxy {}: {}", addr, e)))?;
                let tls_stream = wrap_stream_with_tls(proxy, Box::new(tcp)).await?;
                socks5_handshake_on_stream(proxy, target_host, target_port, tls_stream).await
            } else {
                // Try plain SOCKS5 first; auto-retry with TLS on failure
                match connect_socks5(proxy, target_host, target_port).await {
                    Ok(stream) => Ok(stream),
                    Err(e) if is_socks5_tls_retryable(&e) => {
                        eprintln!(">>> [PROXY] SOCKS5 plain failed ({e}), retrying with TLS (5s timeout)...");
                        let addr = format!("{}:{}", proxy.host, proxy.port);
                        let retry = async {
                            let tcp = TcpStream::connect(&addr).await
                                .map_err(|e| SshError::ProxyFailed(format!("TCP connect: {}", e)))?;
                            let tls_stream = wrap_stream_with_tls(proxy, Box::new(tcp)).await?;
                            socks5_handshake_on_stream(proxy, target_host, target_port, tls_stream).await
                        };
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(5), retry
                        ).await {
                            Ok(Ok(stream)) => Ok(stream),
                            _ => {
                                eprintln!(">>> [PROXY] TLS retry failed/timed out, returning original error");
                                Err(e)
                            }
                        }
                    }
                    Err(e) => Err(e),
                }
            }
        }
        ProxyType::Socks4 => connect_socks4(proxy, target_host, target_port).await,
        ProxyType::Http => {
            if proxy.tls.enabled {
                connect_https(proxy, target_host, target_port).await
            } else {
                connect_http(proxy, target_host, target_port).await
            }
        }
        ProxyType::Command => {
            let cmd = proxy.command.as_deref()
                .ok_or_else(|| SshError::ProxyFailed("ProxyCommand is empty".into()))?;
            super::proxy_command::connect_command(
                cmd, target_host, target_port, proxy.username.as_deref(),
            ).await
        }
    }
}

/// Performs a proxy handshake over a pre-established stream (e.g., an SSH direct-tcpip channel).
///
/// Unlike `connect_via_proxy` which creates a new TCP connection to the proxy,
/// this function uses an existing `AsyncStream` as the transport to the proxy server.
/// Used when the proxy is reachable through an SSH tunnel.
pub async fn connect_via_proxy_on_stream(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    stream: Box<dyn AsyncStream>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    eprintln!(
        ">>> [PROXY_ON_STREAM] type={:?} host={}:{} tls_enabled={} target={}:{}",
        proxy.proxy_type, proxy.host, proxy.port, proxy.tls.enabled, target_host, target_port
    );
    // Wrap in TLS if enabled (applies to SOCKS5, HTTP, etc.)
    let stream = if proxy.tls.enabled {
        eprintln!(">>> [PROXY_ON_STREAM] Wrapping stream in TLS...");
        wrap_stream_with_tls(proxy, stream).await?
    } else {
        eprintln!(">>> [PROXY_ON_STREAM] No TLS, using raw stream");
        stream
    };

    match proxy.proxy_type {
        ProxyType::Socks5 | ProxyType::Tor => {
            socks5_handshake_on_stream(proxy, target_host, target_port, stream).await
        }
        ProxyType::Http => {
            http_connect_on_stream(proxy, target_host, target_port, stream).await
        }
        ProxyType::Socks4 => {
            socks4_handshake_on_stream(target_host, target_port, stream).await
        }
        ProxyType::Command => {
            let cmd = proxy.command.as_deref()
                .ok_or_else(|| SshError::ProxyFailed("ProxyCommand is empty".into()))?;
            super::proxy_command::connect_command(
                cmd, target_host, target_port, proxy.username.as_deref(),
            ).await
        }
    }
}

/// SOCKS5 handshake over an existing stream.
async fn socks5_handshake_on_stream(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    mut stream: Box<dyn AsyncStream>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    let has_auth = proxy.username.is_some() && proxy.password.is_some();

    // Method negotiation
    if has_auth {
        stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await
            .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 write methods: {e}")))?;
    } else {
        stream.write_all(&[0x05, 0x01, 0x00]).await
            .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 write methods: {e}")))?;
    }

    let mut resp = [0u8; 2];
    stream.read_exact(&mut resp).await
        .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 read method reply: {e}")))?;
    if resp[0] != 0x05 {
        return Err(SshError::ProxyFailed(format!("SOCKS5 invalid version: {}", resp[0])));
    }

    // Username/password auth (RFC 1929) if selected
    if resp[1] == 0x02 {
        let user = proxy.username.as_deref().unwrap_or("");
        let pass = proxy.password.as_deref().unwrap_or("");
        let mut auth_req = vec![0x01, user.len() as u8];
        auth_req.extend_from_slice(user.as_bytes());
        auth_req.push(pass.len() as u8);
        auth_req.extend_from_slice(pass.as_bytes());
        stream.write_all(&auth_req).await
            .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 write auth: {e}")))?;

        let mut auth_resp = [0u8; 2];
        stream.read_exact(&mut auth_resp).await
            .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 read auth reply: {e}")))?;
        if auth_resp[1] != 0x00 {
            return Err(SshError::ProxyFailed("SOCKS5 authentication failed".into()));
        }
    } else if resp[1] != 0x00 {
        return Err(SshError::ProxyFailed(format!("SOCKS5 no acceptable method: {}", resp[1])));
    }

    // CONNECT request
    let mut connect_req = vec![0x05, 0x01, 0x00, 0x03];
    let host_bytes = target_host.as_bytes();
    connect_req.push(host_bytes.len() as u8);
    connect_req.extend_from_slice(host_bytes);
    connect_req.push((target_port >> 8) as u8);
    connect_req.push(target_port as u8);
    stream.write_all(&connect_req).await
        .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 write connect: {e}")))?;

    // Read CONNECT reply (min 10 bytes for IPv4 response)
    let mut reply = [0u8; 10];
    stream.read_exact(&mut reply).await
        .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 read connect reply: {e}")))?;
    if reply[1] != 0x00 {
        return Err(SshError::ProxyFailed(format!("SOCKS5 CONNECT failed: reply code {}", reply[1])));
    }

    // Handle variable-length BND.ADDR
    match reply[3] {
        0x03 => {
            // Domain: skip len + domain + port (already read 10 bytes, need to read extra)
            let domain_len = reply[4] as usize;
            if domain_len > 5 {
                let extra = domain_len - 5 + 2; // remaining domain bytes + port
                let mut extra_buf = vec![0u8; extra];
                stream.read_exact(&mut extra_buf).await
                    .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 read bind addr: {e}")))?;
            }
        }
        0x04 => {
            // IPv6: need 12 more bytes (16 addr + 2 port - 6 already read)
            let mut extra = [0u8; 12];
            stream.read_exact(&mut extra).await
                .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 read bind addr: {e}")))?;
        }
        _ => {} // IPv4: already fully read in the 10-byte reply
    }

    Ok(stream)
}

/// HTTP CONNECT handshake over an existing stream.
async fn http_connect_on_stream(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    mut stream: Box<dyn AsyncStream>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    let target = format!("{}:{}", target_host, target_port);
    let mut request = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n", target, target);

    if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
        let credentials = format!("{}:{}", user, pass);
        let encoded = base64_encode(credentials.as_bytes());
        request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
    }
    request.push_str("\r\n");

    stream.write_all(request.as_bytes()).await
        .map_err(|e| SshError::ProxyFailed(format!("HTTP CONNECT write: {e}")))?;

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await
        .map_err(|e| SshError::ProxyFailed(format!("HTTP CONNECT read: {e}")))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.contains("200") {
        let first_line = response.lines().next().unwrap_or("(empty)");
        return Err(SshError::ProxyFailed(format!("HTTP CONNECT rejected: {first_line}")));
    }

    Ok(stream)
}

/// SOCKS4/4a handshake over an existing stream.
async fn socks4_handshake_on_stream(
    target_host: &str,
    target_port: u16,
    mut stream: Box<dyn AsyncStream>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    // SOCKS4a: VER(1) + CMD(1) + PORT(2) + IP(4: 0.0.0.x) + USERID(1: null) + DOMAIN + null
    let mut req = vec![0x04, 0x01];
    req.push((target_port >> 8) as u8);
    req.push(target_port as u8);
    // SOCKS4a: use 0.0.0.x to signal domain follows
    req.extend_from_slice(&[0, 0, 0, 1]);
    req.push(0x00); // empty user ID
    req.extend_from_slice(target_host.as_bytes());
    req.push(0x00); // null terminator

    stream.write_all(&req).await
        .map_err(|e| SshError::ProxyFailed(format!("SOCKS4 write: {e}")))?;

    let mut reply = [0u8; 8];
    stream.read_exact(&mut reply).await
        .map_err(|e| SshError::ProxyFailed(format!("SOCKS4 read reply: {e}")))?;

    if reply[1] != 0x5A {
        return Err(SshError::ProxyFailed(format!("SOCKS4 rejected: status {:#04x}", reply[1])));
    }

    Ok(stream)
}

/// SOCKS5 proxy connection (RFC 1928).
/// Supports no-auth and username/password authentication.
async fn connect_socks5(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, SshError> {
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
    let target = (target_host.to_string(), target_port);

    let stream = if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
        tokio_socks::tcp::Socks5Stream::connect_with_password(
            proxy_addr.as_str(),
            target,
            user,
            pass,
        )
        .await
        .map_err(|e| SshError::ProxyFailed(format!("SOCKS5 auth: {}", e)))?
    } else {
        tokio_socks::tcp::Socks5Stream::connect(proxy_addr.as_str(), target)
            .await
            .map_err(|e| SshError::ProxyFailed(format!("SOCKS5: {}", e)))?
    };

    Ok(Box::new(stream))
}

/// SOCKS4/4a proxy connection.
/// SOCKS4a sends the domain name to the proxy for DNS resolution.
async fn connect_socks4(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, SshError> {
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);

    // SOCKS4a: domain name resolution on proxy side
    let stream = tokio_socks::tcp::Socks4Stream::connect(
        proxy_addr.as_str(),
        (target_host.to_string(), target_port),
    )
    .await
    .map_err(|e| SshError::ProxyFailed(format!("SOCKS4: {}", e)))?;

    Ok(Box::new(stream))
}

/// HTTP CONNECT proxy tunnel.
/// Sends `CONNECT host:port HTTP/1.1` and expects a `200` response.
async fn connect_http(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, SshError> {
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
    let mut stream = TcpStream::connect(&proxy_addr)
        .await
        .map_err(|e| SshError::ProxyFailed(format!("HTTP proxy connect: {}", e)))?;

    // Build CONNECT request
    let target = format!("{}:{}", target_host, target_port);
    let mut request = format!(
        "CONNECT {} HTTP/1.1\r\nHost: {}\r\n",
        target, target,
    );

    // Add Basic Auth if credentials provided
    if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
        let credentials = format!("{}:{}", user, pass);
        let encoded = base64_encode(credentials.as_bytes());
        request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
    }

    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| SshError::ProxyFailed(format!("HTTP CONNECT write: {}", e)))?;

    // Read response (look for "HTTP/1.x 200")
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| SshError::ProxyFailed(format!("HTTP CONNECT read: {}", e)))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.contains("200") {
        let first_line = response.lines().next().unwrap_or("(empty)");
        return Err(SshError::ProxyFailed(format!(
            "HTTP CONNECT rejected: {}",
            first_line
        )));
    }

    Ok(Box::new(stream))
}

/// HTTPS CONNECT proxy tunnel with TLS + optional client certificate (mTLS).
///
/// Flow: TcpStream → TLS handshake (with optional client cert) → HTTP CONNECT → tunnel
async fn connect_https(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, SshError> {
    use rustls::pki_types::ServerName;
    use rustls_pemfile::{certs, private_key};
    use std::io::BufReader;
    use tokio_rustls::TlsConnector;

    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
    let tcp_stream = TcpStream::connect(&proxy_addr)
        .await
        .map_err(|e| SshError::ProxyFailed(format!("HTTPS proxy TCP connect: {}", e)))?;

    // Build TLS config
    let mut root_store = rustls::RootCertStore::empty();

    // Load custom CA cert if provided, otherwise use system roots
    if let Some(ca_path) = &proxy.tls.ca_cert_path {
        let ca_data = std::fs::read(ca_path)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to read CA cert: {}", e)))?;
        let mut reader = BufReader::new(ca_data.as_slice());
        let ca_certs = certs(&mut reader).filter_map(|c| c.ok()).collect::<Vec<_>>();
        for cert in ca_certs {
            root_store.add(cert)
                .map_err(|e| SshError::ProxyFailed(format!("Invalid CA cert: {}", e)))?;
        }
    } else {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let tls_config_builder = rustls::ClientConfig::builder()
        .with_root_certificates(root_store);

    // Load client certificate + key for mTLS
    let tls_config = if let (Some(cert_path), Some(key_path)) = (&proxy.tls.client_cert_path, &proxy.tls.client_key_path) {
        let cert_data = std::fs::read(cert_path)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to read client cert: {}", e)))?;
        let mut cert_reader = BufReader::new(cert_data.as_slice());
        let client_certs = certs(&mut cert_reader).filter_map(|c| c.ok()).collect::<Vec<_>>();

        let key_data = std::fs::read(key_path)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to read client key: {}", e)))?;
        let mut key_reader = BufReader::new(key_data.as_slice());
        let client_key = private_key(&mut key_reader)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to parse client key: {}", e)))?
            .ok_or_else(|| SshError::ProxyFailed("No private key found in file".into()))?;

        tls_config_builder.with_client_auth_cert(client_certs, client_key)
            .map_err(|e| SshError::ProxyFailed(format!("Client cert config error: {}", e)))?
    } else {
        tls_config_builder.with_no_client_auth()
    };

    // Disable server cert verification if requested
    let tls_config = if !proxy.tls.verify {
        let mut cfg = tls_config;
        cfg.dangerous().set_certificate_verifier(Arc::new(NoVerifier));
        cfg
    } else {
        tls_config
    };

    let server_name = ServerName::try_from(proxy.host.clone())
        .map_err(|_| SshError::ProxyFailed(format!("Invalid proxy hostname for TLS: {}", proxy.host)))?;

    let connector = TlsConnector::from(Arc::new(tls_config));
    let mut tls_stream = connector.connect(server_name, tcp_stream)
        .await
        .map_err(|e| SshError::ProxyFailed(format!("TLS handshake with proxy: {}", e)))?;

    // Send HTTP CONNECT over TLS
    let target = format!("{}:{}", target_host, target_port);
    let mut request = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n", target, target);

    if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
        let credentials = format!("{}:{}", user, pass);
        let encoded = base64_encode(credentials.as_bytes());
        request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
    }
    request.push_str("\r\n");

    tls_stream.write_all(request.as_bytes()).await
        .map_err(|e| SshError::ProxyFailed(format!("HTTPS CONNECT write: {}", e)))?;

    let mut buf = vec![0u8; 4096];
    let n = tls_stream.read(&mut buf).await
        .map_err(|e| SshError::ProxyFailed(format!("HTTPS CONNECT read: {}", e)))?;

    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.contains("200") {
        let first_line = response.lines().next().unwrap_or("(empty)");
        return Err(SshError::ProxyFailed(format!("HTTPS CONNECT rejected: {}", first_line)));
    }

    Ok(Box::new(tls_stream))
}

/// Wraps an existing async stream in TLS using the proxy's TLS configuration.
///
/// Used to add TLS on top of any transport (direct TCP, SSH direct-tcpip channel, etc.)
/// before performing a SOCKS5 or HTTP CONNECT proxy handshake.
pub async fn wrap_stream_with_tls(
    proxy: &ProxyConfig,
    stream: Box<dyn AsyncStream>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    use rustls::pki_types::ServerName;
    use rustls_pemfile::{certs, private_key};
    use std::io::BufReader;
    use tokio_rustls::TlsConnector;

    let mut root_store = rustls::RootCertStore::empty();

    // Filter empty paths (DB may store "" instead of NULL)
    let ca_path = proxy.tls.ca_cert_path.as_deref().filter(|p| !p.is_empty());
    let cert_path = proxy.tls.client_cert_path.as_deref().filter(|p| !p.is_empty());
    let key_path = proxy.tls.client_key_path.as_deref().filter(|p| !p.is_empty());

    if let Some(ca_path) = ca_path {
        let ca_data = std::fs::read(ca_path)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to read CA cert: {}", e)))?;
        let mut reader = BufReader::new(ca_data.as_slice());
        let ca_certs = certs(&mut reader).filter_map(|c| c.ok()).collect::<Vec<_>>();
        for cert in ca_certs {
            root_store.add(cert)
                .map_err(|e| SshError::ProxyFailed(format!("Invalid CA cert: {}", e)))?;
        }
    } else {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let tls_config_builder = rustls::ClientConfig::builder()
        .with_root_certificates(root_store);

    let tls_config = if let (Some(cert_path), Some(key_path)) = (cert_path, key_path) {
        let cert_data = std::fs::read(cert_path)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to read client cert: {}", e)))?;
        let mut cert_reader = BufReader::new(cert_data.as_slice());
        let client_certs = certs(&mut cert_reader).filter_map(|c| c.ok()).collect::<Vec<_>>();

        let key_data = std::fs::read(key_path)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to read client key: {}", e)))?;
        let mut key_reader = BufReader::new(key_data.as_slice());
        let client_key = private_key(&mut key_reader)
            .map_err(|e| SshError::ProxyFailed(format!("Failed to parse client key: {}", e)))?
            .ok_or_else(|| SshError::ProxyFailed("No private key found in file".into()))?;

        tls_config_builder.with_client_auth_cert(client_certs, client_key)
            .map_err(|e| SshError::ProxyFailed(format!("Client cert config error: {}", e)))?
    } else {
        tls_config_builder.with_no_client_auth()
    };

    let tls_config = if !proxy.tls.verify {
        let mut cfg = tls_config;
        cfg.dangerous().set_certificate_verifier(Arc::new(NoVerifier));
        cfg
    } else {
        tls_config
    };

    let server_name = ServerName::try_from(proxy.host.clone())
        .map_err(|_| SshError::ProxyFailed(format!("Invalid proxy hostname for TLS: {}", proxy.host)))?;

    let connector = TlsConnector::from(Arc::new(tls_config));
    let tls_stream = connector.connect(server_name, stream)
        .await
        .map_err(|e| SshError::ProxyFailed(format!("TLS handshake with proxy: {}", e)))?;

    Ok(Box::new(tls_stream))
}

/// Dummy certificate verifier that accepts any server certificate.
/// Used when `tls_verify` is disabled (e.g., self-signed proxy certs in dev/test).
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Simple Base64 encoder (avoids adding a dependency for this single use).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((n >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(n & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}
