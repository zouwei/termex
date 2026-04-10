//! Integration tests for the SSH config parser.

use std::io::Write;
use tempfile::NamedTempFile;
use termex_lib::ssh::config_parser::{parse_ssh_config, pattern_matches, is_non_interactive_host};

fn write_config(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

// ── Host Parsing ──

#[test]
fn test_parse_basic_host() {
    let f = write_config(
        "Host myserver\n  HostName 10.0.1.1\n  Port 2222\n  User admin\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);

    let entry = &result.entries[0];
    assert_eq!(entry.host_alias, "myserver");
    assert_eq!(entry.hostname, "10.0.1.1");
    assert_eq!(entry.port, 2222);
    assert_eq!(entry.user, "admin");
    assert!(!entry.is_wildcard);
}

#[test]
fn test_parse_identity_file() {
    let f = write_config(
        "Host keyhost\n  HostName 10.0.1.2\n  IdentityFile ~/.ssh/id_rsa\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);

    let identity = result.entries[0].identity_file.as_deref().unwrap();
    // Tilde should be resolved — the path must NOT start with "~/"
    assert!(
        !identity.starts_with("~/"),
        "tilde was not resolved: {identity}"
    );
    assert!(
        identity.ends_with(".ssh/id_rsa"),
        "unexpected identity path: {identity}"
    );
}

#[test]
fn test_parse_wildcard_merge() {
    let f = write_config(
        "Host *\n  User default\n\nHost myhost\n  HostName 10.0.1.1\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();

    // Wildcard entries are filtered out after merging; only concrete hosts remain
    let concrete: Vec<_> = result
        .entries
        .iter()
        .filter(|e| !e.is_wildcard)
        .collect();
    assert_eq!(concrete.len(), 1);

    let entry = concrete[0];
    assert_eq!(entry.host_alias, "myhost");
    assert_eq!(entry.hostname, "10.0.1.1");
    assert_eq!(entry.user, "default", "wildcard User should merge into concrete host");
}

#[test]
fn test_parse_multiple_hosts() {
    let f = write_config(
        "Host alpha\n  HostName 10.0.0.1\n\n\
         Host beta\n  HostName 10.0.0.2\n\n\
         Host gamma\n  HostName 10.0.0.3\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 3);

    let aliases: Vec<&str> = result.entries.iter().map(|e| e.host_alias.as_str()).collect();
    assert!(aliases.contains(&"alpha"));
    assert!(aliases.contains(&"beta"));
    assert!(aliases.contains(&"gamma"));
}

#[test]
fn test_parse_proxy_jump() {
    let f = write_config(
        "Host target\n  HostName 10.0.1.5\n  ProxyJump bastion1,bastion2\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(
        result.entries[0].proxy_jump.as_deref(),
        Some("bastion1,bastion2")
    );
}

#[test]
fn test_parse_proxy_command() {
    let f = write_config(
        "Host jumped\n  HostName 10.0.1.6\n  ProxyCommand ssh -W %h:%p jump\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(
        result.entries[0].proxy_command.as_deref(),
        Some("ssh -W %h:%p jump")
    );
}

#[test]
fn test_parse_comments() {
    let f = write_config(
        "# This is a comment\n\n\
         Host commented\n  # inline comment\n  HostName 10.0.1.7\n\n\
         # trailing comment\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].hostname, "10.0.1.7");
}

// ── Pattern Matching ──

#[test]
fn test_pattern_star() {
    assert!(pattern_matches("*.example.com", "web.example.com"));
    assert!(pattern_matches("*.example.com", "api.example.com"));
    assert!(!pattern_matches("*.example.com", "example.com"));
}

#[test]
fn test_pattern_question() {
    assert!(pattern_matches("host?", "host1"));
    assert!(pattern_matches("host?", "hostA"));
    assert!(!pattern_matches("host?", "host12"), "? should match exactly one character");
}

#[test]
fn test_pattern_negation() {
    assert!(!pattern_matches("!*.internal", "web.internal"));
    assert!(pattern_matches("!*.internal", "web.public"));
}

// ── Match Directive Warning ──

#[test]
fn test_match_directive_warning() {
    let f = write_config(
        "Host normal\n  HostName 10.0.1.1\n\n\
         Match host *.example.com\n  User matchuser\n\n\
         Host after\n  HostName 10.0.1.2\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();

    // Should NOT crash — Match is skipped with a warning
    let has_warning = result
        .errors
        .iter()
        .any(|e| e.message.contains("Match directive skipped"));
    assert!(has_warning, "expected a warning about Match directive, got: {:?}", result.errors);

    // The concrete hosts should still parse successfully
    let aliases: Vec<&str> = result.entries.iter().map(|e| e.host_alias.as_str()).collect();
    assert!(aliases.contains(&"normal"));
    assert!(aliases.contains(&"after"));
}

// ── Non-Interactive Host Detection ──

#[test]
fn test_github_detected_as_non_interactive() {
    let f = write_config(
        "Host github.com\n  HostName github.com\n  User git\n  IdentityFile ~/.ssh/id_ed25519\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert!(result.entries[0].is_non_interactive, "github.com should be non-interactive");
}

#[test]
fn test_gitlab_detected_as_non_interactive() {
    let f = write_config(
        "Host gitlab.com\n  HostName gitlab.com\n  User git\n  IdentityFile ~/.ssh/id_rsa\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert!(result.entries[0].is_non_interactive);
}

#[test]
fn test_normal_server_not_non_interactive() {
    let f = write_config(
        "Host myserver\n  HostName 10.0.1.1\n  User admin\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert!(!result.entries[0].is_non_interactive, "normal server should NOT be non-interactive");
}

#[test]
fn test_github_alias_detected_as_non_interactive() {
    let f = write_config(
        "Host github-personal\n  HostName github.com\n  User git\n  IdentityFile ~/.ssh/personal\n",
    );
    let result = parse_ssh_config(f.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert!(result.entries[0].is_non_interactive, "alias pointing to github.com should be non-interactive");
}
