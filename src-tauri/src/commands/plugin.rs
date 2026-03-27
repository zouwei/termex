use tauri::State;

use crate::plugin::registry::InstalledPlugin;
use crate::state::AppState;

/// Lists all installed plugins.
#[tauri::command]
pub fn plugin_list(state: State<'_, AppState>) -> Result<Vec<InstalledPlugin>, String> {
    let registry = state.plugin_registry.read().expect("plugin lock poisoned");
    Ok(registry.list())
}

/// Installs a plugin from a directory path.
#[tauri::command]
pub fn plugin_install(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<InstalledPlugin, String> {
    let mut registry = state
        .plugin_registry
        .write()
        .expect("plugin lock poisoned");
    registry.install(&source_path).map_err(|e| e.to_string())
}

/// Uninstalls a plugin.
#[tauri::command]
pub fn plugin_uninstall(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let mut registry = state
        .plugin_registry
        .write()
        .expect("plugin lock poisoned");
    registry.uninstall(&plugin_id).map_err(|e| e.to_string())
}

/// Enables a plugin.
#[tauri::command]
pub fn plugin_enable(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let mut registry = state
        .plugin_registry
        .write()
        .expect("plugin lock poisoned");
    registry.enable(&plugin_id).map_err(|e| e.to_string())
}

/// Disables a plugin.
#[tauri::command]
pub fn plugin_disable(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let mut registry = state
        .plugin_registry
        .write()
        .expect("plugin lock poisoned");
    registry.disable(&plugin_id).map_err(|e| e.to_string())
}
