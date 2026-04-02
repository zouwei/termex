use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri::menu::CheckMenuItem;

/// Stores CheckMenuItem references so commands can update them.
/// `Menu::get()` doesn't search submenus, so we keep direct refs.
pub struct MenuCheckItems(pub Mutex<HashMap<String, CheckMenuItem<tauri::Wry>>>);

/// Updates a CheckMenuItem's checked state from the frontend.
/// Used to sync View menu checkmarks with panel visibility.
#[tauri::command]
pub fn set_menu_checked(app: AppHandle, id: String, checked: bool) -> Result<(), String> {
    if let Some(state) = app.try_state::<MenuCheckItems>() {
        let items = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(item) = items.get(&id) {
            item.set_checked(checked).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
