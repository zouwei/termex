use termex_lib::team::sync::{merge_decision, MergeDecision};
use termex_lib::team::types::{
    SharedServerConfig, TeamJson, TeamMemberEntry, TeamSettings,
};

#[test]
fn test_merge_decision_remote_newer() {
    let decision = merge_decision(
        Some("2026-04-09T10:00:00Z"),
        "2026-04-10T10:00:00Z",
    );
    assert_eq!(decision, MergeDecision::ImportRemote);
}

#[test]
fn test_merge_decision_local_newer() {
    let decision = merge_decision(
        Some("2026-04-10T10:00:00Z"),
        "2026-04-09T10:00:00Z",
    );
    assert_eq!(decision, MergeDecision::ExportLocal);
}

#[test]
fn test_merge_decision_equal() {
    let decision = merge_decision(
        Some("2026-04-10T10:00:00Z"),
        "2026-04-10T10:00:00Z",
    );
    assert_eq!(decision, MergeDecision::NoAction);
}

#[test]
fn test_merge_decision_no_local() {
    let decision = merge_decision(None, "2026-04-10T10:00:00Z");
    assert_eq!(decision, MergeDecision::ImportRemote);
}

#[test]
fn test_team_json_serialize_roundtrip() {
    let team = TeamJson {
        version: 1,
        name: "DevOps Alpha".to_string(),
        salt: "a1b2c3d4e5f67890a1b2c3d4e5f67890".to_string(),
        verify: "base64-verify-token".to_string(),
        members: vec![
            TeamMemberEntry {
                username: "alice".to_string(),
                role: "admin".to_string(),
                joined_at: "2026-04-09T10:00:00Z".to_string(),
                device_id: "abc123".to_string(),
            },
            TeamMemberEntry {
                username: "bob".to_string(),
                role: "member".to_string(),
                joined_at: "2026-04-09T10:05:00Z".to_string(),
                device_id: "def456".to_string(),
            },
        ],
        settings: TeamSettings::default(),
    };

    let json = serde_json::to_string_pretty(&team).unwrap();
    let parsed: TeamJson = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "DevOps Alpha");
    assert_eq!(parsed.members.len(), 2);
    assert_eq!(parsed.members[0].role, "admin");
    assert!(parsed.settings.allow_member_push);
}

#[test]
fn test_shared_server_config_serialize_roundtrip() {
    let config = SharedServerConfig {
        id: "srv-001".to_string(),
        name: "prod-web-01".to_string(),
        host: "10.0.1.100".to_string(),
        port: 22,
        username: "deploy".to_string(),
        auth_type: "password".to_string(),
        password_enc: Some("base64-encrypted-pw".to_string()),
        passphrase_enc: None,
        group_id: Some("group-prod".to_string()),
        tags: "production,web".to_string(),
        startup_cmd: None,
        encoding: "UTF-8".to_string(),
        auto_record: false,
        shared_by: "alice".to_string(),
        shared_at: "2026-04-09T10:30:00Z".to_string(),
        updated_at: "2026-04-09T10:30:00Z".to_string(),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: SharedServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "prod-web-01");
    assert_eq!(parsed.port, 22);
    assert!(parsed.password_enc.is_some());
    assert!(parsed.passphrase_enc.is_none());
}

#[test]
fn test_shared_server_config_minimal() {
    // Minimal JSON — optional fields missing
    let json = r#"{
        "id": "srv-002",
        "name": "test",
        "host": "127.0.0.1",
        "port": 22,
        "username": "root",
        "auth_type": "key",
        "password_enc": null,
        "passphrase_enc": null,
        "group_id": null,
        "tags": "",
        "startup_cmd": null,
        "encoding": "UTF-8",
        "auto_record": false,
        "shared_by": "bob",
        "shared_at": "2026-04-10T08:00:00Z",
        "updated_at": "2026-04-10T08:00:00Z"
    }"#;

    let parsed: SharedServerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.auth_type, "key");
    assert!(parsed.password_enc.is_none());
}
