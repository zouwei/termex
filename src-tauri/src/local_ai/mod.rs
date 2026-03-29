pub mod binary_manager;
pub mod downloader;
pub mod health_check;
pub mod storage;

use std::path::PathBuf;
use std::process::Stdio;

/// Represents the current state of the llama-server process.
#[derive(Debug, Clone)]
pub struct LlamaServerState {
    /// The llama-server subprocess, if running.
    pub process_id: Option<u32>,
    /// The allocated port number (8000-9000 range).
    pub port: Option<u16>,
    /// The currently loaded model path.
    pub loaded_model: Option<String>,
}

impl LlamaServerState {
    /// Creates a new, uninitialized LlamaServerState.
    pub fn new() -> Self {
        Self {
            process_id: None,
            port: None,
            loaded_model: None,
        }
    }

    /// Starts the llama-server process with the given model.
    ///
    /// # Arguments
    /// * `binary_path` - Path to the llama-server binary
    /// * `model_path` - Path to the GGUF model file
    ///
    /// # Returns
    /// The allocated port number if successful, or an error message.
    pub async fn start(
        &mut self,
        binary_path: PathBuf,
        model_path: PathBuf,
    ) -> Result<u16, String> {
        eprintln!(">>> [MOD] start() called with model: {}", model_path.display());

        // If same model is already loaded and running, return the existing port
        if let Some(loaded) = &self.loaded_model {
            let current_model = model_path.to_string_lossy().to_string();
            eprintln!(">>> [MOD] Checking if model already loaded:");
            eprintln!(">>> [MOD]   Loaded model: {}", loaded);
            eprintln!(">>> [MOD]   Current model: {}", current_model);
            eprintln!(">>> [MOD]   Models match: {}", loaded == &current_model);
            eprintln!(">>> [MOD]   PID: {:?}", self.process_id);
            let is_running = self.is_running();
            eprintln!(">>> [MOD]   Process running: {}", is_running);

            if loaded == &current_model && is_running {
                eprintln!(">>> [MOD] Same model already running on port {:?}", self.port);
                log::info!("Model already loaded: {}", loaded);
                return self.port.ok_or_else(|| "Port not set but process is running".to_string());
            } else {
                eprintln!(">>> [MOD] Restarting: model mismatch or process not running");
            }
        } else {
            eprintln!(">>> [MOD] No previous model loaded");
        }

        // Different model or process not running, stop the old one
        if self.process_id.is_some() {
            eprintln!(">>> [MOD] Stopping previous process before starting new one");
            let _ = self.stop().await;
            // Give it a moment to fully terminate
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Verify binary and model exist
        if !binary_path.exists() {
            return Err(format!("llama-server binary not found: {}", binary_path.display()));
        }
        if !model_path.exists() {
            return Err(format!("Model file not found: {}", model_path.display()));
        }

        // Ensure binary exists and get its path
        eprintln!(">>> [MOD] Locating llama-server binary...");
        let actual_binary_path = crate::local_ai::binary_manager::ensure_binary_exists(&binary_path)
            .await
            .map(PathBuf::from)?;

        // Find available port (15000-16000 range - less common, less prone to conflicts)
        let port = find_available_port(15000, 16000)
            .map_err(|_| "No available port in range 15000-16000".to_string())?;

        // Start the process with inherited stdio for direct output
        eprintln!(">>> [MOD] Spawning llama-server: {:?}", &actual_binary_path);
        eprintln!(">>> [MOD] Model: {:?}", &model_path);
        eprintln!(">>> [MOD] Port: {}", port);

        let mut child = std::process::Command::new(&actual_binary_path)
            .arg("--model")
            .arg(&model_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--n-gpu-layers")
            .arg("99")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                eprintln!(">>> [MOD] Failed to spawn process: {}", e);
                format!("Failed to spawn llama-server: {}", e)
            })?;

        let pid = child.id();
        eprintln!(">>> [MOD] Process spawned with PID: {}", pid);

        // Update state with the PID
        self.process_id = Some(pid);
        self.port = Some(port);
        self.loaded_model = Some(model_path.to_string_lossy().to_string());

        // Detach the child process (let it run in background)
        // We'll track it via PID only
        drop(child);

        // Give the process a tiny moment to crash if it's going to
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check if process is still alive before waiting for port
        if !self.is_running() {
            eprintln!(">>> [MOD] Process died immediately after spawn! PID {} is not running", pid);
            return Err("llama-server process died immediately after startup. Check the binary file and dependencies.".to_string());
        }

        eprintln!(">>> [MOD] Process is alive, waiting for port {} to listen (this may take up to 120 seconds for model loading)...", port);
        log::info!("Process alive, waiting for llama-server (PID: {}) to start listening on port {}...", pid, port);

        match crate::local_ai::health_check::wait_for_port(port, 120).await {
            Ok(()) => {
                eprintln!(">>> [MOD] llama-server successfully started and listening on port {}", port);
                log::info!("llama-server successfully started and listening on port {}", port);
                Ok(port)
            }
            Err(e) => {
                eprintln!(">>> [MOD] Failed to start llama-server on port {}: {}", port, e);
                log::error!("Failed to start llama-server on port {}: {}", port, e);

                // Check if process is still running
                let is_alive = self.is_running();
                eprintln!(">>> [MOD] Process alive after timeout: {}", is_alive);

                // Try to stop the process
                let _ = self.stop().await;
                Err(format!("llama-server failed to start: {}", e))
            }
        }
    }

    /// Stops the llama-server process if running.
    pub async fn stop(&mut self) -> Result<(), String> {
        if let Some(pid) = self.process_id {
            // Try to kill the process
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("taskkill")
                    .arg("/PID")
                    .arg(pid.to_string())
                    .arg("/F")
                    .output()
                    .map_err(|e| format!("Failed to kill process: {}", e))?;
            }

            #[cfg(not(target_os = "windows"))]
            {
                std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output()
                    .map_err(|e| format!("Failed to kill process: {}", e))?;
            }

            // Small delay to ensure process is killed
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        self.process_id = None;
        self.port = None;
        self.loaded_model = None;

        Ok(())
    }

    /// Checks if the process is still running.
    pub fn is_running(&self) -> bool {
        if let Some(pid) = self.process_id {
            let running = check_process_running(pid);
            eprintln!(">>> [MOD] check_process_running({}) = {}", pid, running);
            running
        } else {
            eprintln!(">>> [MOD] No PID stored, process not running");
            false
        }
    }
}

/// Finds an available TCP port in the given range.
fn find_available_port(start: u16, end: u16) -> Result<u16, String> {
    use std::net::TcpListener;

    for port in start..=end {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            drop(listener);
            return Ok(port);
        }
    }

    Err("No available port found".to_string())
}

/// Checks if a process with the given PID is still running.
#[cfg(target_os = "macos")]
fn check_process_running(pid: u32) -> bool {
    match std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
    {
        Ok(output) => {
            let running = output.status.success();
            eprintln!(">>> [CHECK] kill -0 {} = {}", pid, running);
            if !running {
                eprintln!(">>> [CHECK] kill stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
            running
        }
        Err(e) => {
            eprintln!(">>> [CHECK] kill -0 {} failed: {}", pid, e);
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn check_process_running(pid: u32) -> bool {
    match std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
    {
        Ok(output) => {
            let running = output.status.success();
            eprintln!(">>> [CHECK] kill -0 {} = {}", pid, running);
            if !running {
                eprintln!(">>> [CHECK] kill stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
            running
        }
        Err(e) => {
            eprintln!(">>> [CHECK] kill -0 {} failed: {}", pid, e);
            false
        }
    }
}

#[cfg(target_os = "windows")]
fn check_process_running(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .arg("/FI")
        .arg(format!("PID eq {}", pid))
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}
