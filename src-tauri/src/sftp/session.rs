use russh::client;
use russh_sftp::client::SftpSession;
use serde::Serialize;

use super::SftpError;
use crate::ssh::auth::ClientHandler;

/// A file entry returned by directory listing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub permissions: Option<String>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub mtime: Option<u64>,
}

/// Transfer progress information emitted as events.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub transfer_id: String,
    pub remote_path: String,
    pub transferred: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

/// Wraps an `SftpSession` tied to an SSH connection.
pub struct SftpHandle {
    pub(super) sftp: SftpSession,
}

impl SftpHandle {
    /// Opens an SFTP subsystem on the given SSH handle.
    pub async fn open(handle: &client::Handle<ClientHandler>) -> Result<Self, SftpError> {
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| SftpError::ChannelError(e.to_string()))?;

        channel
            .request_subsystem(false, "sftp")
            .await
            .map_err(|e| SftpError::ChannelError(e.to_string()))?;

        let sftp = SftpSession::new(channel.into_stream()).await?;
        Ok(Self { sftp })
    }

    /// Lists directory contents at the given path.
    pub async fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>, SftpError> {
        let read_dir = self.sftp.read_dir(path).await?;
        let entries = read_dir
            .map(|entry| {
                let meta = entry.metadata();
                let perms = meta.permissions.map(|p| {
                    let fp = russh_sftp::protocol::FilePermissions::from(p);
                    fp.to_string()
                });
                FileEntry {
                    name: entry.file_name(),
                    is_dir: meta.is_dir(),
                    is_symlink: meta.is_symlink(),
                    size: meta.size.unwrap_or(0),
                    permissions: perms,
                    uid: meta.uid,
                    gid: meta.gid,
                    mtime: meta.mtime.map(|t| t as u64),
                }
            })
            .collect();
        Ok(entries)
    }

    /// Creates a directory at the given path.
    pub async fn mkdir(&self, path: &str) -> Result<(), SftpError> {
        self.sftp.create_dir(path).await?;
        Ok(())
    }

    /// Removes a file at the given path.
    pub async fn remove_file(&self, path: &str) -> Result<(), SftpError> {
        self.sftp.remove_file(path).await?;
        Ok(())
    }

    /// Removes a directory at the given path.
    pub async fn remove_dir(&self, path: &str) -> Result<(), SftpError> {
        self.sftp.remove_dir(path).await?;
        Ok(())
    }

    /// Renames a file or directory.
    pub async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), SftpError> {
        self.sftp.rename(old_path, new_path).await?;
        Ok(())
    }

    /// Reads a small file entirely into memory.
    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>, SftpError> {
        let data = self.sftp.read(path).await?;
        Ok(data)
    }

    /// Writes data to a file (creates or truncates).
    pub async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), SftpError> {
        use russh_sftp::protocol::OpenFlags;
        use tokio::io::AsyncWriteExt;

        let mut file = self
            .sftp
            .open_with_flags(
                path,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await?;
        file.write_all(data).await?;
        Ok(())
    }

    /// Downloads a file and emits progress events.
    pub async fn download(
        &self,
        remote_path: &str,
        local_path: &str,
        transfer_id: &str,
        app: &tauri::AppHandle,
    ) -> Result<(), SftpError> {
        use tauri::Emitter;
        use tokio::io::AsyncReadExt;

        let meta = self.sftp.metadata(remote_path).await?;
        let total = meta.size.unwrap_or(0);

        let mut remote_file = self.sftp.open(remote_path).await?;
        let mut local_file = tokio::fs::File::create(local_path).await?;

        let mut transferred: u64 = 0;
        let mut buf = vec![0u8; 32768];
        let event = format!("sftp://progress/{transfer_id}");

        loop {
            let n = remote_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            tokio::io::AsyncWriteExt::write_all(&mut local_file, &buf[..n]).await?;
            transferred += n as u64;

            let _ = app.emit(
                &event,
                TransferProgress {
                    transfer_id: transfer_id.to_string(),
                    remote_path: remote_path.to_string(),
                    transferred,
                    total,
                    done: false,
                    error: None,
                },
            );
        }

        let _ = app.emit(
            &event,
            TransferProgress {
                transfer_id: transfer_id.to_string(),
                remote_path: remote_path.to_string(),
                transferred,
                total,
                done: true,
                error: None,
            },
        );

        Ok(())
    }

    /// Uploads a local file to the remote server and emits progress events.
    pub async fn upload(
        &self,
        local_path: &str,
        remote_path: &str,
        transfer_id: &str,
        app: &tauri::AppHandle,
    ) -> Result<(), SftpError> {
        use russh_sftp::protocol::OpenFlags;
        use tauri::Emitter;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let local_meta = tokio::fs::metadata(local_path).await?;
        let total = local_meta.len();

        let mut local_file = tokio::fs::File::open(local_path).await?;
        let mut remote_file = self
            .sftp
            .open_with_flags(
                remote_path,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await?;

        let mut transferred: u64 = 0;
        let mut buf = vec![0u8; 32768];
        let event = format!("sftp://progress/{transfer_id}");

        loop {
            let n = local_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buf[..n]).await?;
            transferred += n as u64;

            let _ = app.emit(
                &event,
                TransferProgress {
                    transfer_id: transfer_id.to_string(),
                    remote_path: remote_path.to_string(),
                    transferred,
                    total,
                    done: false,
                    error: None,
                },
            );
        }

        let _ = app.emit(
            &event,
            TransferProgress {
                transfer_id: transfer_id.to_string(),
                remote_path: remote_path.to_string(),
                transferred,
                total,
                done: true,
                error: None,
            },
        );

        Ok(())
    }

    /// Gets the canonical (absolute) path.
    pub async fn canonicalize(&self, path: &str) -> Result<String, SftpError> {
        let abs = self.sftp.canonicalize(path).await?;
        Ok(abs)
    }

    /// Changes file permissions (chmod).
    /// TODO: Implement once russh-sftp provides setstat/chmod API
    /// Currently not supported in russh-sftp 2.1
    #[allow(dead_code)]
    pub async fn chmod(&self, _path: &str, _mode: u32) -> Result<(), SftpError> {
        Err(SftpError::Sftp("chmod not yet supported - russh-sftp API limitation".to_string()))
    }

    /// Closes the SFTP session.
    pub async fn close(self) -> Result<(), SftpError> {
        self.sftp.close().await?;
        Ok(())
    }
}

/// Streams a file from one SFTP server to another in 32KB chunks.
/// Data relays through local memory (Server A → Termex → Server B).
pub async fn transfer_between(
    src: &SftpHandle,
    src_path: &str,
    dst: &SftpHandle,
    dst_path: &str,
    transfer_id: &str,
    app: &tauri::AppHandle,
) -> Result<(), SftpError> {
    use russh_sftp::protocol::OpenFlags;
    use tauri::Emitter;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let meta = src.sftp.metadata(src_path).await?;
    let total = meta.size.unwrap_or(0);

    let mut src_file = src.sftp.open(src_path).await?;
    let mut dst_file = dst
        .sftp
        .open_with_flags(
            dst_path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await?;

    let mut transferred: u64 = 0;
    let mut buf = vec![0u8; 32768];
    let event = format!("sftp://progress/{transfer_id}");

    loop {
        let n = src_file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        dst_file.write_all(&buf[..n]).await?;
        transferred += n as u64;

        let _ = app.emit(
            &event,
            TransferProgress {
                transfer_id: transfer_id.to_string(),
                remote_path: src_path.to_string(),
                transferred,
                total,
                done: false,
                error: None,
            },
        );
    }

    let _ = app.emit(
        &event,
        TransferProgress {
            transfer_id: transfer_id.to_string(),
            remote_path: src_path.to_string(),
            transferred,
            total,
            done: true,
            error: None,
        },
    );

    Ok(())
}