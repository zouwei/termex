use futures_util::StreamExt;
use serde::Serialize;
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

#[derive(Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    received: u64,
    total: u64,
    progress: u32,
}

/// Returns OS and architecture for asset matching.
#[tauri::command]
pub fn get_platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

/// Exits the application.
#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Downloads an update installer to the Downloads folder.
/// Emits `download-progress` events with `{ received, total, progress }`.
/// Returns the full path of the downloaded file.
#[tauri::command]
pub async fn download_update(
    app: tauri::AppHandle,
    url: String,
    filename: String,
) -> Result<String, String> {
    let download_dir = dirs::download_dir()
        .ok_or_else(|| "Cannot resolve Downloads directory".to_string())?;
    let dest_path = download_dir.join(&filename);

    let client = reqwest::Client::builder()
        .user_agent("Termex-Updater/1.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }

    let total = response.content_length().unwrap_or(0);

    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create file: {e}"))?;

    let mut stream = response.bytes_stream();
    let mut received: u64 = 0;
    let mut last_progress: u32 = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Write error: {e}"))?;
        received += chunk.len() as u64;

        if total > 0 {
            let progress = ((received as f64 / total as f64) * 100.0) as u32;
            if progress != last_progress {
                last_progress = progress;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress { received, total, progress },
                );
            }
        }
    }

    file.flush().await.map_err(|e| format!("Flush error: {e}"))?;

    if total > 0 && received != total {
        return Err(format!("Incomplete: {received}/{total} bytes"));
    }

    let full_path = dest_path.to_string_lossy().into_owned();

    // Linux: set executable permission on AppImage
    #[cfg(target_os = "linux")]
    if filename.ends_with(".AppImage") {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755));
    }

    // Open installer with OS default handler
    if let Ok(()) = open::that(&dest_path) {
        let app_handle = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            app_handle.exit(0);
        });
    }

    Ok(full_path)
}
