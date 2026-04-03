use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use termex_lib::ssh::socks5::{
    socks5_handshake, socks5_reply_failure, socks5_reply_success,
    REPLY_COMMAND_NOT_SUPPORTED, REPLY_GENERAL_FAILURE, REPLY_SUCCESS,
};

/// Helper: creates a connected pair (client, server) for testing.
async fn create_pair() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (server, _) = listener.accept().await.unwrap();
    (client, server)
}

#[tokio::test]
async fn test_socks5_handshake_ipv4() {
    let (mut client, mut server) = create_pair().await;

    let handle = tokio::spawn(async move { socks5_handshake(&mut server).await });

    // Method negotiation: version=5, 1 method, NO_AUTH=0
    client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();

    // Wait for method reply (2 bytes)
    let mut reply = [0u8; 2];
    tokio::io::AsyncReadExt::read_exact(&mut client, &mut reply)
        .await
        .unwrap();
    assert_eq!(reply, [0x05, 0x00]); // NO AUTH selected

    // CONNECT request: IPv4 127.0.0.1:8080
    client
        .write_all(&[
            0x05, 0x01, 0x00, 0x01, // ver, CONNECT, rsv, IPv4
            127, 0, 0, 1, // addr
            0x1F, 0x90, // port 8080
        ])
        .await
        .unwrap();

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.0, "127.0.0.1");
    assert_eq!(result.1, 8080);
}

#[tokio::test]
async fn test_socks5_handshake_domain() {
    let (mut client, mut server) = create_pair().await;

    let handle = tokio::spawn(async move { socks5_handshake(&mut server).await });

    // Method negotiation
    client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut reply = [0u8; 2];
    tokio::io::AsyncReadExt::read_exact(&mut client, &mut reply)
        .await
        .unwrap();

    // CONNECT request: domain "example.com":443
    let domain = b"example.com";
    let mut req = vec![0x05, 0x01, 0x00, 0x03, domain.len() as u8];
    req.extend_from_slice(domain);
    req.extend_from_slice(&443u16.to_be_bytes());
    client.write_all(&req).await.unwrap();

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.0, "example.com");
    assert_eq!(result.1, 443);
}

#[tokio::test]
async fn test_socks5_handshake_ipv6() {
    let (mut client, mut server) = create_pair().await;

    let handle = tokio::spawn(async move { socks5_handshake(&mut server).await });

    client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut reply = [0u8; 2];
    tokio::io::AsyncReadExt::read_exact(&mut client, &mut reply)
        .await
        .unwrap();

    // CONNECT request: IPv6 ::1:80
    let mut req = vec![0x05, 0x01, 0x00, 0x04];
    req.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]); // ::1
    req.extend_from_slice(&80u16.to_be_bytes());
    client.write_all(&req).await.unwrap();

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.0, "::1");
    assert_eq!(result.1, 80);
}

#[tokio::test]
async fn test_socks5_unsupported_method() {
    let (mut client, mut server) = create_pair().await;

    let handle = tokio::spawn(async move { socks5_handshake(&mut server).await });

    // Only offer USERNAME/PASSWORD (0x02), no NO_AUTH
    client.write_all(&[0x05, 0x01, 0x02]).await.unwrap();

    let result = handle.await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("no acceptable auth method"));
}

#[tokio::test]
async fn test_socks5_unsupported_command() {
    let (mut client, mut server) = create_pair().await;

    let handle = tokio::spawn(async move { socks5_handshake(&mut server).await });

    // Method negotiation OK
    client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut reply = [0u8; 2];
    tokio::io::AsyncReadExt::read_exact(&mut client, &mut reply)
        .await
        .unwrap();

    // BIND command (0x02) instead of CONNECT (0x01)
    client
        .write_all(&[
            0x05, 0x02, 0x00, 0x01, // BIND
            127, 0, 0, 1, 0x00, 0x50,
        ])
        .await
        .unwrap();

    let result = handle.await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unsupported command"));
}

#[tokio::test]
async fn test_socks5_reply_success_format() {
    let (mut client, mut server) = create_pair().await;

    socks5_reply_success(&mut server).await.unwrap();

    let mut buf = [0u8; 10];
    tokio::io::AsyncReadExt::read_exact(&mut client, &mut buf)
        .await
        .unwrap();
    assert_eq!(buf[0], 0x05); // VER
    assert_eq!(buf[1], REPLY_SUCCESS); // REP = success
    assert_eq!(buf[2], 0x00); // RSV
    assert_eq!(buf[3], 0x01); // ATYP = IPv4
}

#[tokio::test]
async fn test_socks5_reply_failure_format() {
    let (mut client, mut server) = create_pair().await;

    socks5_reply_failure(&mut server, REPLY_GENERAL_FAILURE)
        .await
        .unwrap();

    let mut buf = [0u8; 10];
    tokio::io::AsyncReadExt::read_exact(&mut client, &mut buf)
        .await
        .unwrap();
    assert_eq!(buf[0], 0x05);
    assert_eq!(buf[1], REPLY_GENERAL_FAILURE);

    // Test another code
    let (mut client2, mut server2) = create_pair().await;
    socks5_reply_failure(&mut server2, REPLY_COMMAND_NOT_SUPPORTED)
        .await
        .unwrap();
    let mut buf2 = [0u8; 10];
    tokio::io::AsyncReadExt::read_exact(&mut client2, &mut buf2)
        .await
        .unwrap();
    assert_eq!(buf2[1], REPLY_COMMAND_NOT_SUPPORTED);
}
