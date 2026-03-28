pub mod ai;
mod commands;
pub mod crypto;
pub mod keychain;
pub mod plugin;
pub mod recording;
pub mod sftp;
pub mod ssh;
pub mod storage;
mod state;

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::Emitter;

use state::AppState;

/// Builds the native application menu.
fn build_menu(app: &tauri::App) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // App submenu (macOS "Termex" menu)
    let settings = MenuItemBuilder::with_id("settings", "Settings...")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let app_menu = SubmenuBuilder::new(app, "Termex")
        .about(None)
        .separator()
        .item(&settings)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    // File submenu
    let new_connection = MenuItemBuilder::with_id("new_connection", "New Connection")
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let new_group = MenuItemBuilder::with_id("new_group", "New Group")
        .build(app)?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&new_connection)
        .item(&new_group)
        .separator()
        .close_window()
        .build()?;

    // Edit submenu
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // View submenu
    let toggle_sidebar = MenuItemBuilder::with_id("toggle_sidebar", "Sidebar")
        .accelerator("CmdOrCtrl+\\")
        .build(app)?;
    let toggle_ai = MenuItemBuilder::with_id("toggle_ai", "AI Panel")
        .accelerator("CmdOrCtrl+Shift+I")
        .build(app)?;
    let toggle_sftp = MenuItemBuilder::with_id("toggle_sftp", "SFTP Panel")
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&toggle_sidebar)
        .item(&toggle_ai)
        .item(&toggle_sftp)
        .build()?;

    // Window submenu
    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .fullscreen()
        .build()?;

    // Help submenu
    let check_update = MenuItemBuilder::with_id("check_update", "Check for Updates...")
        .build(app)?;
    let privacy_policy = MenuItemBuilder::with_id("privacy_policy", "Privacy Policy")
        .build(app)?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&check_update)
        .separator()
        .item(&privacy_policy)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()?;

    Ok(menu)
}

/// Initializes and runs the Tauri application.
pub fn run() {
    // MVP: no master password — database is unencrypted.
    // When user sets a master password, it encrypts credential fields via AES-256-GCM.
    let app_state = AppState::new(None).expect("failed to initialize database");

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            let menu = build_menu(app)?;
            app.set_menu(menu)?;

            app.on_menu_event(move |app_handle, event| {
                match event.id().as_ref() {
                    "settings" => {
                        let _ = app_handle.emit("menu://settings", ());
                    }
                    "new_connection" => {
                        let _ = app_handle.emit("menu://new-connection", ());
                    }
                    "new_group" => {
                        let _ = app_handle.emit("menu://new-group", ());
                    }
                    "toggle_sidebar" => {
                        let _ = app_handle.emit("menu://toggle-sidebar", ());
                    }
                    "toggle_ai" => {
                        let _ = app_handle.emit("menu://toggle-ai", ());
                    }
                    "toggle_sftp" => {
                        let _ = app_handle.emit("menu://toggle-sftp", ());
                    }
                    "check_update" => {
                        let _ = app_handle.emit("menu://check-update", ());
                    }
                    "privacy_policy" => {
                        let _ = app_handle.emit("menu://privacy-policy", ());
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Master password
            commands::crypto::master_password_exists,
            commands::crypto::master_password_set,
            commands::crypto::master_password_verify,
            commands::crypto::master_password_change,
            commands::crypto::master_password_lock,
            // Server & group management
            commands::server::server_list,
            commands::server::server_create,
            commands::server::server_update,
            commands::server::server_delete,
            commands::server::server_get_credentials,
            commands::server::server_touch,
            commands::server::server_reorder,
            commands::server::group_list,
            commands::server::group_create,
            commands::server::group_update,
            commands::server::group_delete,
            commands::server::group_reorder,
            // SSH
            commands::ssh::ssh_connect,
            commands::ssh::ssh_test,
            commands::ssh::ssh_disconnect,
            commands::ssh::ssh_write,
            commands::ssh::ssh_resize,
            // Port Forwarding
            commands::port_forward::port_forward_list,
            commands::port_forward::port_forward_save,
            commands::port_forward::port_forward_delete,
            commands::port_forward::port_forward_start,
            commands::port_forward::port_forward_stop,
            // Settings
            commands::settings::settings_get,
            commands::settings::settings_get_all,
            commands::settings::settings_set,
            commands::settings::settings_delete,
            // Config Export/Import
            commands::config::config_export,
            commands::config::config_import,
            // AI
            commands::ai::ai_provider_list,
            commands::ai::ai_provider_add,
            commands::ai::ai_provider_update,
            commands::ai::ai_provider_delete,
            commands::ai::ai_provider_set_default,
            commands::ai::ai_check_danger,
            commands::ai::ai_explain_command,
            commands::ai::ai_nl2cmd,
            commands::ai::ai_provider_get_key,
            commands::ai::ai_provider_test,
            commands::ai::ai_provider_test_direct,
            // Recording
            commands::recording::recording_start,
            commands::recording::recording_stop,
            commands::recording::recording_is_active,
            commands::recording::recording_list,
            commands::recording::recording_read,
            commands::recording::recording_delete,
            // Plugins
            commands::plugin::plugin_list,
            commands::plugin::plugin_install,
            commands::plugin::plugin_uninstall,
            commands::plugin::plugin_enable,
            commands::plugin::plugin_disable,
            // SFTP
            commands::sftp::sftp_open,
            commands::sftp::sftp_close,
            commands::sftp::sftp_list_dir,
            commands::sftp::sftp_mkdir,
            commands::sftp::sftp_delete,
            commands::sftp::sftp_rename,
            commands::sftp::sftp_read_file,
            commands::sftp::sftp_write_file,
            commands::sftp::sftp_download,
            commands::sftp::sftp_upload,
            commands::sftp::sftp_canonicalize,
            // Local filesystem
            commands::local_fs::local_home_dir,
            commands::local_fs::local_list_dir,
            commands::local_fs::security_status,
            commands::local_fs::open_url,
            // Update
            commands::update::get_platform_info,
            commands::update::download_update,
            commands::update::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Termex");
}
