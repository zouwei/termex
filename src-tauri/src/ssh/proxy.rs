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
}

impl ProxyType {
    /// Parses from database string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "socks5" => Some(Self::Socks5),
            "socks4" => Some(Self::Socks4),
            "http" => Some(Self::Http),
            "tor" => Some(Self::Tor),
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
}

/// Connects to a target host through a network proxy, returning an async stream.
///
/// The returned stream can be passed to `russh::client::connect_stream()` for SSH handshake,
/// keeping proxy logic completely decoupled from SSH protocol.
pub async fn connect_via_proxy(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, SshError> {
    match proxy.proxy_type {
        ProxyType::Socks5 | ProxyType::Tor => connect_socks5(proxy, target_host, target_port).await,
        ProxyType::Socks4 => connect_socks4(proxy, target_host, target_port).await,
        ProxyType::Http => {
            if proxy.tls.enabled {
                connect_https(proxy, target_host, target_port).await
            } else {
                connect_http(proxy, target_host, target_port).await
            }
        }
    }
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

    let mut tls_config_builder = rustls::ClientConfig::builder()
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
