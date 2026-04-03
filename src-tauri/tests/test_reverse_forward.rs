use termex_lib::ssh::reverse_forward::{
    calculate_sync_port, parse_sync_request, ReverseForwardRegistry, SyncAction, HTTP_200,
};

// ── parse_sync_request ──

#[test]
fn test_parse_push_done() {
    let req = b"GET /push-done HTTP/1.1\r\nHost: 127.0.0.1:19527\r\n\r\n";
    let action = parse_sync_request(req);
    assert!(matches!(action, Some(SyncAction::PushDone)));
}

#[test]
fn test_parse_push_done_minimal() {
    // curl -s sends a minimal request
    let req = b"GET /push-done HTTP/1.1\r\n\r\n";
    assert!(matches!(parse_sync_request(req), Some(SyncAction::PushDone)));
}

#[test]
fn test_parse_unknown_path() {
    let req = b"GET /other HTTP/1.1\r\n\r\n";
    assert!(parse_sync_request(req).is_none());
}

#[test]
fn test_parse_post_ignored() {
    let req = b"POST /push-done HTTP/1.1\r\n\r\n";
    // We match "GET /push-done" specifically
    assert!(parse_sync_request(req).is_none());
}

#[test]
fn test_parse_empty_data() {
    assert!(parse_sync_request(b"").is_none());
}

#[test]
fn test_parse_partial_data() {
    assert!(parse_sync_request(b"GET /pu").is_none());
}

// ── HTTP_200 response ──

#[test]
fn test_http_200_is_valid() {
    let resp = std::str::from_utf8(HTTP_200).unwrap();
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("Content-Length: 2"));
    assert!(resp.ends_with("OK"));
}

// ── calculate_sync_port ──

#[test]
fn test_sync_port_range() {
    // Port must be in [19500, 19999]
    for id in ["abc", "12345678-abcd-ef01", "", "x".repeat(100).as_str()] {
        let port = calculate_sync_port(id);
        assert!(port >= 19500, "port {} < 19500 for id '{}'", port, id);
        assert!(port <= 19999, "port {} > 19999 for id '{}'", port, id);
    }
}

#[test]
fn test_sync_port_deterministic() {
    let id = "a1b2c3d4-e5f6-7890-abcd-ef0123456789";
    let p1 = calculate_sync_port(id);
    let p2 = calculate_sync_port(id);
    assert_eq!(p1, p2);
}

#[test]
fn test_sync_port_different_ids() {
    let p1 = calculate_sync_port("server-aaa");
    let p2 = calculate_sync_port("server-bbb");
    // Different IDs should (very likely) produce different ports
    // Not guaranteed but extremely unlikely to collide for distinct strings
    assert_ne!(p1, p2);
}

// ── ReverseForwardRegistry ──

#[test]
fn test_registry_register_and_lookup() {
    let mut reg = ReverseForwardRegistry::new();
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    reg.register("127.0.0.1", 19527, "srv-1".into(), tx);

    let entry = reg.lookup("127.0.0.1", 19527);
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().server_id, "srv-1");
}

#[test]
fn test_registry_lookup_missing() {
    let reg = ReverseForwardRegistry::new();
    assert!(reg.lookup("127.0.0.1", 19527).is_none());
}

#[test]
fn test_registry_unregister() {
    let mut reg = ReverseForwardRegistry::new();
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    reg.register("127.0.0.1", 19527, "srv-1".into(), tx);

    reg.unregister("127.0.0.1", 19527);
    assert!(reg.lookup("127.0.0.1", 19527).is_none());
}

#[test]
fn test_registry_clear_for_server() {
    let mut reg = ReverseForwardRegistry::new();
    let (tx1, _rx1) = tokio::sync::mpsc::channel(1);
    let (tx2, _rx2) = tokio::sync::mpsc::channel(1);
    reg.register("127.0.0.1", 19500, "srv-1".into(), tx1);
    reg.register("127.0.0.1", 19501, "srv-2".into(), tx2);

    reg.clear_for_server("srv-1");

    assert!(reg.lookup("127.0.0.1", 19500).is_none());
    assert!(reg.lookup("127.0.0.1", 19501).is_some());
}

#[test]
fn test_registry_overwrite() {
    let mut reg = ReverseForwardRegistry::new();
    let (tx1, _rx1) = tokio::sync::mpsc::channel(1);
    let (tx2, _rx2) = tokio::sync::mpsc::channel(1);
    reg.register("127.0.0.1", 19527, "srv-old".into(), tx1);
    reg.register("127.0.0.1", 19527, "srv-new".into(), tx2);

    let entry = reg.lookup("127.0.0.1", 19527).unwrap();
    assert_eq!(entry.server_id, "srv-new");
}
