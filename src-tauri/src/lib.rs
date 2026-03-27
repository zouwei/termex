pub mod ai;
mod commands;
pub mod crypto;
pub mod plugin;
pub mod recording;
pub mod sftp;
pub mod ssh;
pub mod storage;
mod state;

use state::AppState;

/// Initializes and runs the Tauri application.
pub fn run() {
    // MVP: no master password — database is unencrypted.
    // When user sets a master password, it encrypts credential fields via AES-256-GCM.
    let app_state = AppState::new(None).expect("failed to initialize database");

    tauri::Builder::default()
        .manage(app_state)
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
            commands::server::server_touch,
            commands::server::server_reorder,
            commands::server::group_list,
            commands::server::group_create,
            commands::server::group_update,
            commands::server::group_delete,
            commands::server::group_reorder,
            // SSH
            commands::ssh::ssh_connect,
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
            commands::ai::ai_provider_test,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Termex");
}
