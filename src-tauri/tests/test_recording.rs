use termex_lib::recording::asciicast::{AsciicastEvent, AsciicastFile, AsciicastHeader};
use termex_lib::recording::recorder::RecorderRegistry;

// ── Asciicast Tests ──

#[test]
fn test_header_serialize() {
    let header = AsciicastHeader::new(80, 24, Some("test".into()));
    let json = serde_json::to_string(&header).unwrap();
    assert!(json.contains("\"version\":2"));
    assert!(json.contains("\"width\":80"));
    assert!(json.contains("\"title\":\"test\""));
}

#[test]
fn test_event_serialize() {
    let event = AsciicastEvent::output(1.5, "hello world");
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(json, "[1.5,\"o\",\"hello world\"]");
}

#[test]
fn test_input_event_serialize() {
    let event = AsciicastEvent::input(2.0, "ls\r");
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(json, "[2.0,\"i\",\"ls\\r\"]");
}

#[test]
fn test_parse_roundtrip() {
    let file = AsciicastFile {
        header: AsciicastHeader {
            version: 2,
            width: 80,
            height: 24,
            timestamp: Some(1700000000),
            title: Some("test session".into()),
            env: None,
        },
        events: vec![
            AsciicastEvent::output(0.0, "$ "),
            AsciicastEvent::input(0.5, "ls\r"),
            AsciicastEvent::output(0.6, "file1.txt  file2.txt\r\n$ "),
        ],
    };
    let serialized = file.serialize().unwrap();
    let parsed = AsciicastFile::parse(&serialized).unwrap();
    assert_eq!(parsed.header.version, 2);
    assert_eq!(parsed.header.width, 80);
    assert_eq!(parsed.events.len(), 3);
    assert!((parsed.duration() - 0.6).abs() < 0.001);
}

#[test]
fn test_duration() {
    let file = AsciicastFile {
        header: AsciicastHeader::new(80, 24, None),
        events: vec![
            AsciicastEvent::output(0.0, "start"),
            AsciicastEvent::output(5.5, "end"),
        ],
    };
    assert!((file.duration() - 5.5).abs() < 0.001);
}

#[test]
fn test_duration_empty() {
    let file = AsciicastFile {
        header: AsciicastHeader::new(80, 24, None),
        events: vec![],
    };
    assert_eq!(file.duration(), 0.0);
}

// ── Recorder Tests ──

#[tokio::test]
async fn test_recorder_lifecycle() {
    let registry = RecorderRegistry::new();
    assert!(!registry.is_recording("test-session").await);

    let (rec_id, _path) = registry
        .start("test-session", "server-1", "Test Server", 80, 24, Some("test".into()), false, 50)
        .await
        .unwrap();
    assert!(registry.is_recording("test-session").await);
    assert!(!rec_id.is_empty());

    registry.record_output("test-session", "$ ").await;
    registry.record_input("test-session", "ls\r").await;
    registry.record_output("test-session", "file.txt\r\n").await;

    let (stopped_id, result_path) = registry.stop("test-session").await.unwrap();
    assert_eq!(stopped_id, rec_id);
    assert!(result_path.exists());

    let content = std::fs::read_to_string(&result_path).unwrap();
    let file = AsciicastFile::parse(&content).unwrap();
    assert_eq!(file.events.len(), 3);
    assert_eq!(file.header.width, 80);

    let _ = std::fs::remove_file(result_path);
}

#[tokio::test]
async fn test_recorder_returns_recording_id() {
    let registry = RecorderRegistry::new();
    let (rec_id, _) = registry
        .start("sid-1", "srv-1", "Server 1", 80, 24, None, true, 50)
        .await
        .unwrap();

    let fetched_id = registry.get_recording_id("sid-1").await;
    assert_eq!(fetched_id, Some(rec_id.clone()));

    let (stopped_id, path) = registry.stop("sid-1").await.unwrap();
    assert_eq!(stopped_id, rec_id);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn test_recorder_size_limit() {
    let registry = RecorderRegistry::new();
    // 1 MB limit
    let (_, _) = registry
        .start("sid-limit", "srv-1", "Server", 80, 24, None, false, 1)
        .await
        .unwrap();

    // Write data until limit is exceeded
    let chunk = "x".repeat(100_000); // 100 KB per chunk
    for _ in 0..9 {
        let within = registry.record_output("sid-limit", &chunk).await;
        assert!(within);
    }

    // 10th chunk pushes past 1MB
    let within = registry.record_output("sid-limit", &chunk).await;
    assert!(within); // 1000KB, still under 1MB (1048576 bytes)

    // Write more to exceed
    let big_chunk = "y".repeat(200_000);
    let within = registry.record_output("sid-limit", &big_chunk).await;
    assert!(!within); // Now over 1MB

    let (_, path) = registry.stop("sid-limit").await.unwrap();
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn test_recorder_stop_nonexistent() {
    let registry = RecorderRegistry::new();
    let result = registry.stop("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_record_output_no_session() {
    let registry = RecorderRegistry::new();
    // Should not panic, returns true (no limit issue since not recording)
    let result = registry.record_output("nonexistent", "data").await;
    assert!(result);
}

#[tokio::test]
async fn test_recorder_directory_structure() {
    let registry = RecorderRegistry::new();
    let (_, path) = registry
        .start("sid-dir", "my-server-id", "MyServer", 80, 24, None, false, 50)
        .await
        .unwrap();

    // Path should contain server_id and date
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("my-server-id"), "path should contain server_id: {}", path_str);
    assert!(path_str.ends_with(".cast"), "path should end with .cast: {}", path_str);

    let (_, result_path) = registry.stop("sid-dir").await.unwrap();
    let _ = std::fs::remove_file(result_path);
}

// ── DB CRUD Tests (using in-memory database) ──

#[test]
fn test_recording_db_crud() {
    use termex_lib::storage::recording::*;

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE recordings (
            id TEXT PRIMARY KEY, session_id TEXT NOT NULL, server_id TEXT NOT NULL,
            server_name TEXT NOT NULL, file_path TEXT NOT NULL, file_size INTEGER DEFAULT 0,
            duration_ms INTEGER DEFAULT 0, cols INTEGER NOT NULL, rows INTEGER NOT NULL,
            event_count INTEGER DEFAULT 0, summary TEXT, auto_recorded INTEGER DEFAULT 0,
            started_at TEXT NOT NULL, ended_at TEXT, created_at TEXT NOT NULL
        )"
    ).unwrap();

    let meta = RecordingMeta {
        id: "rec-1".to_string(),
        session_id: "sid-1".to_string(),
        server_id: "srv-1".to_string(),
        server_name: "Server One".to_string(),
        file_path: "/tmp/test.cast".to_string(),
        file_size: 0,
        duration_ms: 0,
        cols: 80,
        rows: 24,
        event_count: 0,
        summary: None,
        auto_recorded: false,
        started_at: "2026-04-10T10:00:00Z".to_string(),
        ended_at: None,
        created_at: "2026-04-10T10:00:00Z".to_string(),
    };

    // Insert
    insert(&conn, &meta).unwrap();

    // Get
    let fetched = get(&conn, "rec-1").unwrap().unwrap();
    assert_eq!(fetched.server_name, "Server One");
    assert_eq!(fetched.cols, 80);
    assert!(!fetched.auto_recorded);

    // Update on stop
    update_on_stop(&conn, "rec-1", 1024, 5000, 42, "2026-04-10T10:05:00Z").unwrap();
    let updated = get(&conn, "rec-1").unwrap().unwrap();
    assert_eq!(updated.file_size, 1024);
    assert_eq!(updated.duration_ms, 5000);
    assert_eq!(updated.event_count, 42);
    assert_eq!(updated.ended_at, Some("2026-04-10T10:05:00Z".to_string()));

    // Update summary
    update_summary(&conn, "rec-1", r#"{"overview":"test"}"#).unwrap();
    let with_summary = get(&conn, "rec-1").unwrap().unwrap();
    assert!(with_summary.summary.unwrap().contains("test"));

    // List all
    let all = list_all(&conn, 10, 0).unwrap();
    assert_eq!(all.len(), 1);

    // List by server
    let by_server = list_by_server(&conn, "srv-1", 10, 0).unwrap();
    assert_eq!(by_server.len(), 1);
    let empty = list_by_server(&conn, "srv-nonexistent", 10, 0).unwrap();
    assert_eq!(empty.len(), 0);

    // Find active by session
    // rec-1 now has ended_at set, so should not be found
    let active = find_active_by_session(&conn, "sid-1").unwrap();
    assert!(active.is_none());

    // Insert another without ended_at
    let meta2 = RecordingMeta {
        id: "rec-2".to_string(),
        session_id: "sid-1".to_string(),
        server_id: "srv-1".to_string(),
        server_name: "Server One".to_string(),
        file_path: "/tmp/test2.cast".to_string(),
        file_size: 0,
        duration_ms: 0,
        cols: 80,
        rows: 24,
        event_count: 0,
        summary: None,
        auto_recorded: true,
        started_at: "2026-04-10T11:00:00Z".to_string(),
        ended_at: None,
        created_at: "2026-04-10T11:00:00Z".to_string(),
    };
    insert(&conn, &meta2).unwrap();
    let active = find_active_by_session(&conn, "sid-1").unwrap();
    assert_eq!(active, Some("rec-2".to_string()));

    // Delete
    delete(&conn, "rec-1").unwrap();
    assert!(get(&conn, "rec-1").unwrap().is_none());
    assert_eq!(list_all(&conn, 10, 0).unwrap().len(), 1);

    // Get nonexistent
    assert!(get(&conn, "nonexistent").unwrap().is_none());
}

#[test]
fn test_recording_cleanup_expired() {
    use termex_lib::storage::recording::*;

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE recordings (
            id TEXT PRIMARY KEY, session_id TEXT NOT NULL, server_id TEXT NOT NULL,
            server_name TEXT NOT NULL, file_path TEXT NOT NULL, file_size INTEGER DEFAULT 0,
            duration_ms INTEGER DEFAULT 0, cols INTEGER NOT NULL, rows INTEGER NOT NULL,
            event_count INTEGER DEFAULT 0, summary TEXT, auto_recorded INTEGER DEFAULT 0,
            started_at TEXT NOT NULL, ended_at TEXT, created_at TEXT NOT NULL
        )"
    ).unwrap();

    // Insert an old recording (200 days ago)
    let meta = RecordingMeta {
        id: "old-rec".to_string(),
        session_id: "sid".to_string(),
        server_id: "srv".to_string(),
        server_name: "Old".to_string(),
        file_path: "/tmp/nonexistent.cast".to_string(),
        file_size: 100,
        duration_ms: 1000,
        cols: 80,
        rows: 24,
        event_count: 10,
        summary: None,
        auto_recorded: false,
        started_at: "2025-01-01T00:00:00Z".to_string(),
        ended_at: Some("2025-01-01T00:01:00Z".to_string()),
        created_at: "2025-01-01T00:00:00Z".to_string(),
    };
    insert(&conn, &meta).unwrap();

    // Insert a recent recording
    let meta2 = RecordingMeta {
        id: "new-rec".to_string(),
        session_id: "sid".to_string(),
        server_id: "srv".to_string(),
        server_name: "New".to_string(),
        file_path: "/tmp/nonexistent2.cast".to_string(),
        file_size: 200,
        duration_ms: 2000,
        cols: 80,
        rows: 24,
        event_count: 20,
        summary: None,
        auto_recorded: false,
        started_at: "2026-04-10T00:00:00Z".to_string(),
        ended_at: Some("2026-04-10T00:02:00Z".to_string()),
        created_at: "2026-04-10T00:00:00Z".to_string(),
    };
    insert(&conn, &meta2).unwrap();

    // Cleanup with 90 day retention
    let paths = cleanup_expired(&conn, 90).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], "/tmp/nonexistent.cast");

    // Only the recent one remains
    assert_eq!(list_all(&conn, 10, 0).unwrap().len(), 1);
    assert!(get(&conn, "new-rec").unwrap().is_some());
    assert!(get(&conn, "old-rec").unwrap().is_none());
}

// ── Migration V13 Test ──

#[test]
fn test_migration_v13_creates_recordings_table() {
    use termex_lib::storage::migrations::run_migrations;

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='recordings'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1, "recordings table should exist after migration");

    // Verify indexes exist
    let idx_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_recordings_%'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(idx_count, 2, "two recording indexes should exist");
}
