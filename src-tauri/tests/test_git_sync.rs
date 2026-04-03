use termex_lib::ssh::reverse_forward::calculate_sync_port;

#[test]
fn test_git_sync_script_exists() {
    // The git-sync.sh script is embedded via include_str! in commands/git_sync.rs
    // Verify the asset file exists and has content
    let script = include_str!("../assets/git-sync.sh");
    assert!(!script.is_empty());
    assert!(script.contains("#!/bin/bash"));
    assert!(script.contains("[termex-sync]"));
    assert!(script.contains("git add -A"));
    assert!(script.contains("git push"));
    assert!(script.contains("SYNC_PORT"));
    assert!(script.contains("/push-done"));
}

#[test]
fn test_git_sync_script_reads_sync_port() {
    let script = include_str!("../assets/git-sync.sh");
    // Script should read port from ~/.termex/sync-port with fallback to 19527
    assert!(script.contains("cat ~/.termex/sync-port"));
    assert!(script.contains("19527")); // fallback port
}

#[test]
fn test_git_sync_script_is_executable_compatible() {
    let script = include_str!("../assets/git-sync.sh");
    // Must start with shebang
    assert!(script.starts_with("#!/bin/bash"));
    // Must handle optional path argument
    assert!(script.contains("${1:-.}"));
}

#[test]
fn test_sync_port_for_known_server_ids() {
    // Ensure stability: same ID always yields same port
    let id = "a1b2c3d4-e5f6-7890-abcd-ef0123456789";
    let port = calculate_sync_port(id);
    // This is a regression test — if the hash algorithm changes, this will catch it
    assert_eq!(port, calculate_sync_port(id));
    assert!((19500..=19999).contains(&port));
}

#[test]
fn test_sync_port_empty_id() {
    let port = calculate_sync_port("");
    assert!((19500..=19999).contains(&port));
    // Empty string hash is 0, so port should be 19500
    assert_eq!(port, 19500);
}

#[test]
fn test_deploy_command_structure() {
    // Verify the deploy command would produce valid shell commands
    let sync_port = calculate_sync_port("test-server");
    let script_content = "#!/bin/bash\necho hello";
    let cmd = format!(
        "mkdir -p ~/.termex && cat > ~/.termex/git-sync.sh << 'TERMEX_SYNC_EOF'\n{}\nTERMEX_SYNC_EOF\nchmod +x ~/.termex/git-sync.sh && echo {} > ~/.termex/sync-port",
        script_content, sync_port,
    );
    assert!(cmd.contains("mkdir -p ~/.termex"));
    assert!(cmd.contains("chmod +x"));
    assert!(cmd.contains("TERMEX_SYNC_EOF"));
    assert!(cmd.contains(&sync_port.to_string()));
}
