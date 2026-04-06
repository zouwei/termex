use termex_lib::ssh::proxy_command::substitute_vars;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ── Variable Substitution Tests ──

#[test]
fn test_substitute_hostname() {
    let result = substitute_vars("ssh -W %h:%p bastion", "example.com", 22, None);
    assert_eq!(result, "ssh -W example.com:22 bastion");
}

#[test]
fn test_substitute_all_vars() {
    let result = substitute_vars(
        "cloudflared access ssh --hostname %h --port %p --user %r",
        "server.internal",
        2222,
        Some("admin"),
    );
    assert_eq!(
        result,
        "cloudflared access ssh --hostname server.internal --port 2222 --user admin"
    );
}

#[test]
fn test_substitute_percent_escape() {
    let result = substitute_vars("echo %%h is %h", "host", 22, None);
    assert_eq!(result, "echo %h is host");
}

#[test]
fn test_substitute_no_vars() {
    let result = substitute_vars("cat /dev/null", "host", 22, None);
    assert_eq!(result, "cat /dev/null");
}

#[test]
fn test_substitute_unknown_var_passthrough() {
    let result = substitute_vars("echo %z %h", "host", 22, None);
    assert_eq!(result, "echo %z host");
}

#[test]
fn test_substitute_username_none() {
    let result = substitute_vars("ssh %r@%h", "host", 22, None);
    assert_eq!(result, "ssh %r@host");
}

#[test]
fn test_substitute_trailing_percent() {
    let result = substitute_vars("echo %", "host", 22, None);
    assert_eq!(result, "echo %");
}

#[test]
fn test_substitute_empty_command() {
    let result = substitute_vars("", "host", 22, None);
    assert_eq!(result, "");
}

// ── Cloudflare Tunnel specific substitution ──

#[test]
fn test_substitute_cloudflared_typical() {
    let result = substitute_vars(
        "cloudflared access ssh --hostname %h",
        "ssh.mycompany.com",
        22,
        Some("deploy"),
    );
    assert_eq!(result, "cloudflared access ssh --hostname ssh.mycompany.com");
}

#[test]
fn test_substitute_cloudflared_with_service_token() {
    // Users may chain env vars or flags — shell features via sh -c
    let result = substitute_vars(
        "cloudflared access ssh --hostname %h --id $CF_ACCESS_CLIENT_ID --secret $CF_ACCESS_CLIENT_SECRET",
        "internal.example.com",
        22,
        None,
    );
    assert_eq!(
        result,
        "cloudflared access ssh --hostname internal.example.com --id $CF_ACCESS_CLIENT_ID --secret $CF_ACCESS_CLIENT_SECRET"
    );
}

// ── ProxyType Round-trip Tests ──

use termex_lib::ssh::proxy::ProxyType;

#[test]
fn test_proxy_type_command_roundtrip() {
    let pt = ProxyType::from_str("command").unwrap();
    assert_eq!(pt, ProxyType::Command);
    assert_eq!(pt.as_str(), "command");
}

#[test]
fn test_proxy_type_unknown_returns_none() {
    assert!(ProxyType::from_str("unknown").is_none());
}

// ── connect_command Error Tests ──

#[tokio::test]
async fn test_connect_command_empty_string() {
    let result =
        termex_lib::ssh::proxy_command::connect_command("", "host", 22, None).await;
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("empty"), "Expected 'empty' in error: {msg}");
        }
        Ok(_) => panic!("Expected error for empty command"),
    }
}

#[tokio::test]
async fn test_connect_command_whitespace_only() {
    let result =
        termex_lib::ssh::proxy_command::connect_command("   ", "host", 22, None).await;
    assert!(result.is_err());
}

// ── CommandStream I/O Tests ──
// These tests use real child processes to verify the bidirectional stream works.

/// Verifies that data written to CommandStream stdin is read back from stdout
/// using `cat` as an echo process.
#[tokio::test]
async fn test_command_stream_cat_echo() {
    let mut stream = termex_lib::ssh::proxy_command::connect_command(
        "cat", "ignored", 0, None,
    )
    .await
    .expect("cat should spawn successfully");

    // Write data to the process stdin
    let payload = b"hello proxy command\n";
    stream.write_all(payload).await.expect("write should succeed");
    stream.flush().await.expect("flush should succeed");

    // Read echoed data from process stdout
    let mut buf = vec![0u8; 128];
    let n = stream.read(&mut buf).await.expect("read should succeed");
    assert_eq!(&buf[..n], payload, "cat should echo back exactly what was written");
}

/// Verifies variable substitution is applied before spawning.
/// Uses `echo` to print the substituted command args to stdout.
#[tokio::test]
async fn test_command_stream_var_substitution_in_output() {
    let mut stream = termex_lib::ssh::proxy_command::connect_command(
        "echo %h:%p:%r", "myhost", 8022, Some("testuser"),
    )
    .await
    .expect("echo should spawn successfully");

    let mut buf = vec![0u8; 256];
    let n = stream.read(&mut buf).await.expect("read should succeed");
    let output = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(output.trim(), "myhost:8022:testuser");
}

/// Verifies that multiple write/read cycles work (bidirectional streaming).
#[tokio::test]
async fn test_command_stream_multiple_roundtrips() {
    let mut stream = termex_lib::ssh::proxy_command::connect_command(
        "cat", "ignored", 0, None,
    )
    .await
    .expect("cat should spawn");

    for i in 0..5 {
        let msg = format!("roundtrip-{}\n", i);
        stream.write_all(msg.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();

        let mut buf = vec![0u8; 64];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(
            String::from_utf8_lossy(&buf[..n]),
            msg,
            "roundtrip {} failed",
            i
        );
    }
}

/// Verifies that binary data (non-UTF8) passes through correctly.
/// SSH protocol sends binary data, so this is essential.
#[tokio::test]
async fn test_command_stream_binary_data() {
    let mut stream = termex_lib::ssh::proxy_command::connect_command(
        "cat", "ignored", 0, None,
    )
    .await
    .expect("cat should spawn");

    // Binary payload: 256 bytes covering 0x00..0xFF
    let payload: Vec<u8> = (0..=255).collect();
    stream.write_all(&payload).await.unwrap();
    stream.flush().await.unwrap();

    // Read back the exact number of bytes we wrote (avoid read_to_end which waits for EOF)
    let mut received = vec![0u8; 256];
    let mut total = 0;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while total < 256 {
        let remaining = std::time::Duration::from_secs(5)
            .checked_sub(deadline.elapsed())
            .unwrap_or_default();
        match tokio::time::timeout(remaining, stream.read(&mut received[total..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => total += n,
            Ok(Err(e)) => panic!("read error: {}", e),
            Err(_) => panic!("timeout reading binary data, got {} of 256 bytes", total),
        }
    }
    assert_eq!(&received[..total], &payload[..], "binary data should pass through unmodified");
}

/// Verifies the child process is killed when CommandStream is dropped.
#[tokio::test]
async fn test_command_stream_drop_kills_child() {
    use std::process::Command as StdCommand;

    // Spawn a sleep process that would run for 999 seconds
    let stream = termex_lib::ssh::proxy_command::connect_command(
        "sleep 999", "ignored", 0, None,
    )
    .await
    .expect("sleep should spawn");

    // Find the sleep process PID via pgrep (macOS/Linux)
    let before = StdCommand::new("pgrep")
        .args(["-f", "sleep 999"])
        .output();

    // Drop the stream — should kill the child
    drop(stream);

    // Give the OS a moment to reap
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Check that the sleep process is gone
    let after = StdCommand::new("pgrep")
        .args(["-f", "sleep 999"])
        .output();

    if let (Ok(before_out), Ok(after_out)) = (before, after) {
        let before_pids = String::from_utf8_lossy(&before_out.stdout);
        let after_pids = String::from_utf8_lossy(&after_out.stdout);
        // Before drop, process should exist; after drop, it should be gone
        assert!(
            !before_pids.trim().is_empty(),
            "sleep process should exist before drop"
        );
        assert!(
            after_pids.trim().is_empty(),
            "sleep process should be killed after drop, but found: {}",
            after_pids.trim()
        );
    }
    // If pgrep is not available, skip assertion (test still verifies no panic on drop)
}

/// Verifies that connect_command works with a nc-style TCP relay command.
/// This simulates the same flow as `cloudflared access ssh --hostname %h`.
///
/// Requires: nc (netcat) installed, port 22 listening on localhost.
/// Skips gracefully if prerequisites are not met.
#[tokio::test]
async fn test_command_stream_nc_tcp_relay() {
    // Check if nc is available
    let nc_check = std::process::Command::new("which").arg("nc").output();
    if nc_check.map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("SKIP: nc not found, skipping TCP relay test");
        return;
    }

    // Check if something is listening on localhost:22 (SSH)
    let port_check = std::process::Command::new("nc")
        .args(["-z", "-w1", "127.0.0.1", "22"])
        .output();
    if port_check.map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("SKIP: nothing listening on localhost:22, skipping TCP relay test");
        return;
    }

    // Connect through nc as ProxyCommand (equivalent to `ssh -o ProxyCommand='nc %h %p'`)
    let mut stream = termex_lib::ssh::proxy_command::connect_command(
        "nc %h %p", "127.0.0.1", 22, None,
    )
    .await
    .expect("nc should spawn and connect to localhost:22");

    // SSH servers send a banner on connect (e.g., "SSH-2.0-OpenSSH_9.6\r\n")
    let mut buf = vec![0u8; 256];
    let read_result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        stream.read(&mut buf),
    )
    .await;

    match read_result {
        Ok(Ok(n)) if n > 0 => {
            let banner = String::from_utf8_lossy(&buf[..n]);
            assert!(
                banner.starts_with("SSH-"),
                "Expected SSH banner, got: {}",
                banner.trim()
            );
            eprintln!("SSH banner via nc ProxyCommand: {}", banner.trim());
        }
        Ok(Ok(_)) => panic!("nc connected but read 0 bytes"),
        Ok(Err(e)) => panic!("nc read error: {}", e),
        Err(_) => panic!("timeout reading SSH banner via nc"),
    }
}

/// Simulates the cloudflared flow by verifying the full connect_via_proxy path
/// with ProxyType::Command. This tests the integration between proxy.rs dispatcher
/// and proxy_command.rs.
#[tokio::test]
async fn test_proxy_config_command_dispatch() {
    use termex_lib::ssh::proxy::{ProxyConfig, ProxyTlsConfig, ProxyType, connect_via_proxy};

    let config = ProxyConfig {
        proxy_type: ProxyType::Command,
        host: String::new(),
        port: 0,
        username: None,
        password: None,
        tls: ProxyTlsConfig::default(),
        command: Some("echo hello".to_string()),
    };

    // This tests the full dispatch: connect_via_proxy → ProxyType::Command → connect_command
    let mut stream = connect_via_proxy(&config, "ignored", 0)
        .await
        .expect("Command proxy dispatch should work");

    let mut buf = vec![0u8; 64];
    let n = stream.read(&mut buf).await.expect("should read echo output");
    let output = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(output.trim(), "hello");
}

/// Tests that connect_via_proxy returns error when command is None.
#[tokio::test]
async fn test_proxy_config_command_missing() {
    use termex_lib::ssh::proxy::{ProxyConfig, ProxyTlsConfig, ProxyType, connect_via_proxy};

    let config = ProxyConfig {
        proxy_type: ProxyType::Command,
        host: String::new(),
        port: 0,
        username: None,
        password: None,
        tls: ProxyTlsConfig::default(),
        command: None,
    };

    let result = connect_via_proxy(&config, "host", 22).await;
    assert!(result.is_err());
    let err = result.err().unwrap().to_string();
    assert!(err.contains("empty"), "Expected 'empty' in error: {err}");
}
