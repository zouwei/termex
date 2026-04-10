use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A single parsed SSH config host entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigEntry {
    /// The Host alias pattern (e.g. "myserver", "*.example.com").
    pub host_alias: String,
    /// Resolved HostName (falls back to host_alias if not set).
    pub hostname: String,
    /// Port number (default 22).
    pub port: u16,
    /// Username (default: current system user).
    pub user: String,
    /// Path to identity file (~ resolved).
    pub identity_file: Option<String>,
    /// ProxyJump value.
    pub proxy_jump: Option<String>,
    /// ProxyCommand value.
    pub proxy_command: Option<String>,
    /// Whether this entry comes from a wildcard Host pattern.
    pub is_wildcard: bool,
    /// Whether this entry is a non-interactive host (e.g. github.com, gitlab.com).
    pub is_non_interactive: bool,
    /// All other options as raw key-value pairs.
    pub raw_options: HashMap<String, String>,
}

/// Result of parsing an SSH config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub entries: Vec<SshConfigEntry>,
    pub errors: Vec<ParseError>,
}

/// A warning or error encountered during parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Parses an SSH config file at the given path.
/// Supports Host, HostName, Port, User, IdentityFile, ProxyJump, ProxyCommand,
/// Include (recursive with glob), and wildcard merging.
pub fn parse_ssh_config(path: &Path) -> Result<ParseResult, String> {
    let mut state = ParserState::new();
    parse_file(path, &mut state, 0)?;
    state.flush_current_block();

    // Merge wildcard defaults into concrete hosts
    merge_wildcards(&mut state);

    // Filter out pure wildcard entries and set defaults
    let default_user = get_default_user();
    let entries: Vec<SshConfigEntry> = state
        .blocks
        .into_iter()
        .map(|mut b| {
            if b.user.is_empty() {
                b.user = default_user.clone();
            }
            if b.hostname.is_empty() {
                b.hostname = b.host_alias.clone();
            }
            b.is_non_interactive = is_non_interactive_host(&b);
            b
        })
        .collect();

    Ok(ParseResult {
        entries,
        errors: state.errors,
    })
}

// ── Internal parser state ──

struct ParserState {
    blocks: Vec<SshConfigEntry>,
    current: Option<SshConfigEntry>,
    errors: Vec<ParseError>,
    /// Tracks visited files to prevent Include cycles.
    visited: std::collections::HashSet<PathBuf>,
}

impl ParserState {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current: None,
            errors: Vec::new(),
            visited: std::collections::HashSet::new(),
        }
    }

    fn flush_current_block(&mut self) {
        if let Some(block) = self.current.take() {
            self.blocks.push(block);
        }
    }

    fn new_block(&mut self, host_alias: &str) {
        self.flush_current_block();
        self.current = Some(SshConfigEntry {
            host_alias: host_alias.to_string(),
            hostname: String::new(),
            port: 22,
            user: String::new(),
            identity_file: None,
            proxy_jump: None,
            proxy_command: None,
            is_wildcard: host_alias.contains('*') || host_alias.contains('?'),
            is_non_interactive: false,
            raw_options: HashMap::new(),
        });
    }

    fn set_option(&mut self, key: &str, value: &str) {
        if let Some(ref mut block) = self.current {
            let lower = key.to_lowercase();
            match lower.as_str() {
                "hostname" => block.hostname = value.to_string(),
                "port" => {
                    if let Ok(p) = value.parse::<u16>() {
                        block.port = p;
                    }
                }
                "user" => block.user = value.to_string(),
                "identityfile" => block.identity_file = Some(resolve_tilde(value)),
                "proxyjump" => block.proxy_jump = Some(value.to_string()),
                "proxycommand" => block.proxy_command = Some(value.to_string()),
                _ => {
                    block.raw_options.insert(key.to_string(), value.to_string());
                }
            }
        } else {
            // Options before any Host block → treat as global defaults (Host *)
            self.new_block("*");
            self.set_option(key, value);
        }
    }
}

/// Recursively parses a single SSH config file.
fn parse_file(
    path: &Path,
    state: &mut ParserState,
    depth: usize,
) -> Result<(), String> {
    if depth > 10 {
        state.errors.push(ParseError {
            file: path.display().to_string(),
            line: 0,
            message: "Include depth limit exceeded (max 10)".to_string(),
        });
        return Ok(());
    }

    let canonical = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());

    if !state.visited.insert(canonical.clone()) {
        state.errors.push(ParseError {
            file: path.display().to_string(),
            line: 0,
            message: "Circular Include detected, skipping".to_string(),
        });
        return Ok(());
    }

    let content = std::fs::read_to_string(path).map_err(|e| {
        format!("Failed to read {}: {}", path.display(), e)
    })?;

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split into keyword and argument
        let (keyword, argument) = match split_keyword(line) {
            Some(pair) => pair,
            None => continue,
        };

        let kw_lower = keyword.to_lowercase();

        match kw_lower.as_str() {
            "host" => {
                // Host directive can have multiple patterns separated by spaces
                for pattern in argument.split_whitespace() {
                    // Each pattern becomes a separate block
                    state.new_block(pattern);
                }
            }
            "match" => {
                // Match directive: not supported, record warning
                state.flush_current_block();
                state.current = None; // ignore options until next Host
                state.errors.push(ParseError {
                    file: path.display().to_string(),
                    line: line_num + 1,
                    message: format!(
                        "Match directive skipped at line {}, only Host blocks are supported",
                        line_num + 1
                    ),
                });
            }
            "include" => {
                let include_path = resolve_tilde(argument);
                let base_dir = path.parent().unwrap_or(Path::new("/"));
                let resolved = if Path::new(&include_path).is_absolute() {
                    PathBuf::from(&include_path)
                } else {
                    base_dir.join(&include_path)
                };

                // Glob expand
                let pattern_str = resolved.to_string_lossy().to_string();
                match glob::glob(&pattern_str) {
                    Ok(paths) => {
                        for entry in paths.flatten() {
                            if entry.is_file() {
                                parse_file(&entry, state, depth + 1)?;
                            }
                        }
                    }
                    Err(e) => {
                        state.errors.push(ParseError {
                            file: path.display().to_string(),
                            line: line_num + 1,
                            message: format!("Invalid Include glob pattern: {}", e),
                        });
                    }
                }
            }
            _ => {
                state.set_option(&keyword, argument);
            }
        }
    }

    Ok(())
}

/// Merges wildcard Host entries' defaults into concrete host entries.
/// Uses first-match-wins semantics (consistent with OpenSSH).
fn merge_wildcards(state: &mut ParserState) {
    // Separate wildcards from concrete hosts
    let (wildcards, concrete): (Vec<_>, Vec<_>) =
        state.blocks.drain(..).partition(|b| b.is_wildcard);

    let mut merged = Vec::new();
    for mut host in concrete {
        for wc in &wildcards {
            if pattern_matches(&wc.host_alias, &host.host_alias) {
                // Apply defaults (first-match-wins: only fill empty fields)
                if host.hostname.is_empty() && !wc.hostname.is_empty() {
                    host.hostname = wc.hostname.clone();
                }
                if host.port == 22 && wc.port != 22 {
                    host.port = wc.port;
                }
                if host.user.is_empty() && !wc.user.is_empty() {
                    host.user = wc.user.clone();
                }
                if host.identity_file.is_none() && wc.identity_file.is_some() {
                    host.identity_file = wc.identity_file.clone();
                }
                if host.proxy_jump.is_none() && wc.proxy_jump.is_some() {
                    host.proxy_jump = wc.proxy_jump.clone();
                }
                if host.proxy_command.is_none() && wc.proxy_command.is_some() {
                    host.proxy_command = wc.proxy_command.clone();
                }
                // Merge raw_options (wildcard fills gaps)
                for (k, v) in &wc.raw_options {
                    host.raw_options.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        merged.push(host);
    }

    state.blocks = merged;
}

/// Matches an SSH config pattern against a host alias.
/// Supports `*` (any string), `?` (any single char), and `!` prefix (negation).
pub fn pattern_matches(pattern: &str, text: &str) -> bool {
    if let Some(negated) = pattern.strip_prefix('!') {
        return !do_match(negated.as_bytes(), text.as_bytes());
    }
    do_match(pattern.as_bytes(), text.as_bytes())
}

fn do_match(pattern: &[u8], text: &[u8]) -> bool {
    let (plen, tlen) = (pattern.len(), text.len());
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < tlen {
        if pi < plen && (pattern[pi] == b'?' || pattern[pi].to_ascii_lowercase() == text[ti].to_ascii_lowercase()) {
            pi += 1;
            ti += 1;
        } else if pi < plen && pattern[pi] == b'*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < plen && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == plen
}

// ── Utility functions ──

/// Splits a config line into keyword and argument.
/// Handles both `Keyword Value` and `Keyword=Value` forms.
fn split_keyword(line: &str) -> Option<(&str, &str)> {
    // Try = separator first
    if let Some(eq_pos) = line.find('=') {
        let key = line[..eq_pos].trim();
        let val = line[eq_pos + 1..].trim();
        if !key.is_empty() {
            return Some((key, val));
        }
    }
    // Whitespace separator
    let mut parts = line.splitn(2, char::is_whitespace);
    let key = parts.next()?.trim();
    let val = parts.next().unwrap_or("").trim();
    if key.is_empty() {
        return None;
    }
    Some((key, val))
}

/// Resolves `~` at the beginning of a path to the user's home directory.
fn resolve_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// Gets the current system username as a default SSH user.
fn get_default_user() -> String {
    whoami::username()
}

/// Known non-SSH-login hosts (code hosting, CI, etc.).
/// These entries are valid SSH config but not useful as interactive terminals.
const NON_SSH_HOSTS: &[&str] = &[
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "ssh.dev.azure.com",
    "vs-ssh.visualstudio.com",
    "codeberg.org",
    "gitee.com",
    "jihulab.com",
    "source.developers.google.com",
    "heroku.com",
    "ssh.github.com",
];

/// Returns true if this entry is a non-interactive SSH host (e.g. GitHub).
pub fn is_non_interactive_host(entry: &SshConfigEntry) -> bool {
    let hostname = entry.hostname.to_lowercase();
    let alias = entry.host_alias.to_lowercase();

    // Check exact match or subdomain match against known hosts
    for &host in NON_SSH_HOSTS {
        if hostname == host || alias == host {
            return true;
        }
        // Match subdomains like "github.com-personal"
        if hostname.starts_with(&format!("{}.", host)) || hostname.starts_with(&format!("{}-", host)) {
            return true;
        }
        if alias.starts_with(&format!("{}.", host)) || alias.starts_with(&format!("{}-", host)) {
            return true;
        }
    }

    // User "git" with no explicit hostname usually means a code hosting service
    if entry.user == "git" && entry.hostname == entry.host_alias {
        return true;
    }

    false
}
