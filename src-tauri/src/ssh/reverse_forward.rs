//! SSH reverse port forwarding for Git Auto Sync notifications.
//!
//! When a remote `git push` completes, the remote script sends
//! `GET /push-done` to `127.0.0.1:{port}` via the SSH reverse tunnel.
//! This module receives the forwarded channel data and emits a Tauri event.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

/// Actions that can be triggered by the remote sync script.
#[derive(Debug)]
pub enum SyncAction {
    PushDone,
}

/// Parses a minimal HTTP request for sync actions.
/// Only matches `GET /push-done` — no full HTTP parser needed.
pub fn parse_sync_request(data: &[u8]) -> Option<SyncAction> {
    let request = String::from_utf8_lossy(data);
    if request.contains("GET /push-done") {
        Some(SyncAction::PushDone)
    } else {
        None
    }
}

/// Minimal HTTP 200 response to send back through the channel.
pub const HTTP_200: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";

/// Entry for one reverse-forwarded address:port pair.
pub struct ReverseForwardEntry {
    pub server_id: String,
    pub tx: mpsc::Sender<Vec<u8>>,
}

/// Registry of active reverse forwards, keyed by `"address:port"`.
pub struct ReverseForwardRegistry {
    entries: HashMap<String, ReverseForwardEntry>,
}

impl ReverseForwardRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        address: &str,
        port: u32,
        server_id: String,
        tx: mpsc::Sender<Vec<u8>>,
    ) {
        let key = format!("{}:{}", address, port);
        self.entries.insert(key, ReverseForwardEntry { server_id, tx });
    }

    pub fn unregister(&mut self, address: &str, port: u32) {
        let key = format!("{}:{}", address, port);
        self.entries.remove(&key);
    }

    pub fn lookup(&self, address: &str, port: u32) -> Option<&ReverseForwardEntry> {
        let key = format!("{}:{}", address, port);
        self.entries.get(&key)
    }

    pub fn clear_for_server(&mut self, server_id: &str) {
        self.entries.retain(|_, v| v.server_id != server_id);
    }
}

/// Global shared registry, wrapped in Arc<RwLock> for async access.
pub type SharedReverseForwardRegistry = Arc<RwLock<ReverseForwardRegistry>>;

/// Creates a new shared registry.
pub fn new_shared_registry() -> SharedReverseForwardRegistry {
    Arc::new(RwLock::new(ReverseForwardRegistry::new()))
}

/// Calculates a unique sync port based on server_id hash (range 19500-19999).
pub fn calculate_sync_port(server_id: &str) -> u32 {
    let hash = server_id
        .as_bytes()
        .iter()
        .fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
    19500 + (hash % 500)
}
