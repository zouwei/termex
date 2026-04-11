//! Security audit logging.
//!
//! Records security-relevant events (connections, auth attempts, config changes)
//! to the audit_log database table for compliance (ISO 27001).

use crate::storage::Database;

/// Audit event types.
pub enum AuditEvent {
    // Authentication
    MasterPasswordSet,
    MasterPasswordVerified,
    MasterPasswordFailed,
    MasterPasswordChanged,
    MasterPasswordLocked,
    // SSH
    SshConnectAttempt { server_id: String, host: String },
    SshConnectSuccess { server_id: String, session_id: String },
    SshConnectFailed { server_id: String, error: String },
    SshDisconnect { session_id: String },
    SshHostKeyNewTrusted { host: String, fingerprint: String },
    SshHostKeyChanged { host: String },
    // Data
    ServerCreated { server_id: String },
    ServerDeleted { server_id: String },
    ConfigExported,
    ConfigImported,
    // Credential access
    CredentialAccessed { server_id: String, credential_type: String },
    // Team
    TeamCreate { name: String },
    TeamJoin { name: String, username: String },
    TeamLeave { name: String },
    TeamSync { imported: usize, exported: usize },
    TeamMemberRoleChange { target: String, role: String },
    TeamMemberRemove { target: String },
    TeamKeyRotated,
}

impl AuditEvent {
    fn event_type(&self) -> &str {
        match self {
            Self::MasterPasswordSet => "master_password_set",
            Self::MasterPasswordVerified => "master_password_verified",
            Self::MasterPasswordFailed => "master_password_failed",
            Self::MasterPasswordChanged => "master_password_changed",
            Self::MasterPasswordLocked => "master_password_locked",
            Self::SshConnectAttempt { .. } => "ssh_connect_attempt",
            Self::SshConnectSuccess { .. } => "ssh_connect_success",
            Self::SshConnectFailed { .. } => "ssh_connect_failed",
            Self::SshDisconnect { .. } => "ssh_disconnect",
            Self::SshHostKeyNewTrusted { .. } => "ssh_host_key_new_trusted",
            Self::SshHostKeyChanged { .. } => "ssh_host_key_changed",
            Self::TeamCreate { .. } => "team_create",
            Self::TeamJoin { .. } => "team_join",
            Self::TeamLeave { .. } => "team_leave",
            Self::TeamSync { .. } => "team_sync",
            Self::TeamMemberRoleChange { .. } => "team_member_role_change",
            Self::TeamMemberRemove { .. } => "team_member_remove",
            Self::TeamKeyRotated => "team_key_rotated",
            Self::ServerCreated { .. } => "server_created",
            Self::ServerDeleted { .. } => "server_deleted",
            Self::ConfigExported => "config_exported",
            Self::ConfigImported => "config_imported",
            Self::CredentialAccessed { .. } => "credential_accessed",
        }
    }

    fn detail_json(&self) -> Option<String> {
        let val = match self {
            Self::SshConnectAttempt { server_id, host } =>
                serde_json::json!({"server_id": server_id, "host": host}),
            Self::SshConnectSuccess { server_id, session_id } =>
                serde_json::json!({"server_id": server_id, "session_id": session_id}),
            Self::SshConnectFailed { server_id, error } =>
                serde_json::json!({"server_id": server_id, "error": error}),
            Self::SshDisconnect { session_id } =>
                serde_json::json!({"session_id": session_id}),
            Self::SshHostKeyNewTrusted { host, fingerprint } =>
                serde_json::json!({"host": host, "fingerprint": fingerprint}),
            Self::SshHostKeyChanged { host } =>
                serde_json::json!({"host": host}),
            Self::ServerCreated { server_id } =>
                serde_json::json!({"server_id": server_id}),
            Self::ServerDeleted { server_id } =>
                serde_json::json!({"server_id": server_id}),
            Self::CredentialAccessed { server_id, credential_type } =>
                serde_json::json!({"server_id": server_id, "type": credential_type}),
            Self::TeamCreate { name } =>
                serde_json::json!({"name": name}),
            Self::TeamJoin { name, username } =>
                serde_json::json!({"name": name, "username": username}),
            Self::TeamLeave { name } =>
                serde_json::json!({"name": name}),
            Self::TeamSync { imported, exported } =>
                serde_json::json!({"imported": imported, "exported": exported}),
            Self::TeamMemberRoleChange { target, role } =>
                serde_json::json!({"target": target, "role": role}),
            Self::TeamMemberRemove { target } =>
                serde_json::json!({"target": target}),
            Self::TeamKeyRotated => serde_json::json!({}),
            _ => return None,
        };
        Some(val.to_string())
    }
}

/// Logs an audit event to the database.
pub fn log(db: &Database, event: AuditEvent) {
    let event_type = event.event_type().to_string();
    let detail = event.detail_json();
    let now = time::OffsetDateTime::now_utc().to_string();

    let _ = db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO audit_log (timestamp, event_type, detail) VALUES (?1, ?2, ?3)",
            rusqlite::params![now, event_type, detail],
        )
    });
}

/// Cleans up old audit log entries. Called periodically (e.g., on startup).
pub fn cleanup(db: &Database, retention_days: i64) {
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(retention_days);
    let cutoff_str = cutoff.to_string();

    let _ = db.with_conn(|conn| {
        conn.execute(
            "DELETE FROM audit_log WHERE timestamp < ?1",
            rusqlite::params![cutoff_str],
        )
    });
}
