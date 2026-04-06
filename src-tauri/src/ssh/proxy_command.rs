//! ProxyCommand support for SSH connections.
//!
//! Spawns an external process (e.g. `cloudflared access ssh --hostname %h`) and wraps
//! its stdin/stdout as an async bidirectional stream for use with `russh::client::connect_stream()`.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use super::proxy::AsyncStream;
use super::SshError;

/// Bidirectional async stream wrapping a child process's stdin/stdout.
///
/// Implements `AsyncRead` (from stdout) and `AsyncWrite` (to stdin),
/// satisfying the `AsyncStream` trait for use with `russh::client::connect_stream()`.
/// The child process is killed when this stream is dropped.
pub struct CommandStream {
    stdout: ChildStdout,
    stdin: ChildStdin,
    child: Child,
}

impl AsyncRead for CommandStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for CommandStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

impl Drop for CommandStream {
    fn drop(&mut self) {
        // Non-blocking kill; tolerates "already exited".
        let _ = self.child.start_kill();
    }
}

/// Substitutes OpenSSH-style variables in a ProxyCommand string.
///
/// Supported variables:
/// - `%h` — target hostname
/// - `%p` — target port
/// - `%r` — remote username (left as `%r` if not provided)
/// - `%%` — literal `%`
///
/// Unknown `%X` sequences are left as-is (matches OpenSSH behavior).
pub fn substitute_vars(
    command: &str,
    host: &str,
    port: u16,
    username: Option<&str>,
) -> String {
    let mut result = String::with_capacity(command.len());
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.peek() {
                Some('h') => {
                    chars.next();
                    result.push_str(host);
                }
                Some('p') => {
                    chars.next();
                    result.push_str(&port.to_string());
                }
                Some('r') => {
                    chars.next();
                    if let Some(user) = username {
                        result.push_str(user);
                    } else {
                        result.push_str("%r");
                    }
                }
                Some('%') => {
                    chars.next();
                    result.push('%');
                }
                _ => {
                    // Unknown variable — leave as-is
                    result.push('%');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Spawns a ProxyCommand process and returns its stdin/stdout as an `AsyncStream`.
///
/// The command string is passed to `sh -c` (Unix) or `cmd /C` (Windows)
/// so that shell features (pipes, environment variables) work correctly.
///
/// # Errors
///
/// Returns `SshError::ProxyFailed` if:
/// - The command string is empty
/// - The process fails to spawn (command not found, permission denied)
/// - stdin or stdout cannot be captured
pub async fn connect_command(
    command: &str,
    target_host: &str,
    target_port: u16,
    username: Option<&str>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    if command.trim().is_empty() {
        return Err(SshError::ProxyFailed("ProxyCommand is empty".into()));
    }

    let substituted = substitute_vars(command, target_host, target_port, username);

    #[cfg(unix)]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", &substituted]);
        c
    };

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", &substituted]);
        c
    };

    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        let detail = match e.kind() {
            io::ErrorKind::NotFound => "command not found (sh)".to_string(),
            io::ErrorKind::PermissionDenied => format!("permission denied: {}", substituted),
            _ => e.to_string(),
        };
        SshError::ProxyFailed(format!("ProxyCommand spawn failed: {}", detail))
    })?;

    let stdin = child.stdin.take().ok_or_else(|| {
        SshError::ProxyFailed("Failed to capture ProxyCommand stdin".into())
    })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        SshError::ProxyFailed("Failed to capture ProxyCommand stdout".into())
    })?;

    Ok(Box::new(CommandStream {
        stdout,
        stdin,
        child,
    }))
}
