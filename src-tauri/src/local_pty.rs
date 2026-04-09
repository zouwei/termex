use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem, MasterPty, Child};
use tauri::{AppHandle, Emitter};

/// A running local PTY session.
struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
}

/// Registry of active local PTY sessions.
pub struct PtyRegistry {
    sessions: Mutex<HashMap<String, PtySession>>,
}

impl PtyRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Closes all active PTY sessions. Called on app shutdown.
    pub fn close_all(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        for (_, mut session) in sessions.drain() {
            let _ = session.child.kill();
        }
    }
}

/// Opens a local PTY shell and starts streaming data to the frontend.
#[tauri::command]
pub fn local_pty_open(
    state: tauri::State<'_, PtyRegistry>,
    app: AppHandle,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let pty_system = NativePtySystem::default();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut cmd = CommandBuilder::new_default_prog();
    // Set TERM so programs know they're in a capable terminal
    cmd.env("TERM", "xterm-256color");

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave); // Close slave end — child owns it now

    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let session = PtySession {
        master: pair.master,
        child,
        writer,
    };

    state.sessions.lock().unwrap().insert(session_id.clone(), session);

    // Spawn reader thread: PTY output → Tauri event
    // Short delay to ensure frontend event listener is registered before first emit
    let sid = session_id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let data_event = format!("ssh://data/{sid}");
                    let _ = app.emit(&data_event, buf[..n].to_vec());
                }
            }
        }
        // PTY closed — notify frontend
        let status_event = format!("ssh://status/{sid}");
        let _ = app.emit(
            &status_event,
            serde_json::json!({ "status": "disconnected", "message": "terminal closed" }),
        );
    });

    Ok(())
}

/// Writes user input to a local PTY.
#[tauri::command]
pub fn local_pty_write(
    state: tauri::State<'_, PtyRegistry>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get_mut(&session_id)
        .ok_or("PTY session not found")?;
    session.writer.write_all(&data).map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())
}

/// Resizes a local PTY.
#[tauri::command]
pub fn local_pty_resize(
    state: tauri::State<'_, PtyRegistry>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get(&session_id)
        .ok_or("PTY session not found")?;
    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

/// Closes a local PTY session.
#[tauri::command]
pub fn local_pty_close(
    state: tauri::State<'_, PtyRegistry>,
    session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(mut session) = sessions.remove(&session_id) {
        let _ = session.child.kill();
    }
    Ok(())
}
