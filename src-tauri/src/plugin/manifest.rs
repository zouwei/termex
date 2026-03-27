use serde::{Deserialize, Serialize};

/// Plugin manifest (plugin.json) schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Unique plugin identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Plugin version (semver).
    pub version: String,
    /// Short description.
    pub description: String,
    /// Plugin author.
    pub author: Option<String>,
    /// Homepage URL.
    pub homepage: Option<String>,
    /// Required permissions.
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    /// Entry point file (relative to plugin directory).
    pub entry: String,
    /// Minimum Termex version required.
    pub min_termex_version: Option<String>,
}

/// Plugin permission types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    /// Read terminal output data.
    TerminalRead,
    /// Write to terminal input.
    TerminalWrite,
    /// Access server connection info (host, port, username).
    ServerInfo,
    /// Make network requests.
    Network,
    /// Read/write plugin-specific storage.
    Storage,
    /// Access clipboard.
    Clipboard,
    /// Show notifications.
    Notification,
}

impl PluginManifest {
    /// Parses a manifest from JSON string.
    pub fn parse(json: &str) -> Result<Self, super::PluginError> {
        let manifest: Self = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates the manifest fields.
    fn validate(&self) -> Result<(), super::PluginError> {
        if self.id.is_empty() {
            return Err(super::PluginError::InvalidManifest("id is required".into()));
        }
        if self.name.is_empty() {
            return Err(super::PluginError::InvalidManifest(
                "name is required".into(),
            ));
        }
        if self.entry.is_empty() {
            return Err(super::PluginError::InvalidManifest(
                "entry is required".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_manifest() {
        let json = r#"{
            "id": "snippet-manager",
            "name": "Snippet Manager",
            "version": "1.0.0",
            "description": "Manage command snippets",
            "author": "Termex",
            "permissions": ["terminal_write", "storage"],
            "entry": "index.js"
        }"#;
        let manifest = PluginManifest::parse(json).unwrap();
        assert_eq!(manifest.id, "snippet-manager");
        assert_eq!(manifest.permissions.len(), 2);
        assert!(manifest.permissions.contains(&PluginPermission::TerminalWrite));
        assert!(manifest.permissions.contains(&PluginPermission::Storage));
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "version": "0.1.0",
            "description": "Test plugin",
            "entry": "main.js"
        }"#;
        let manifest = PluginManifest::parse(json).unwrap();
        assert_eq!(manifest.id, "test");
        assert!(manifest.permissions.is_empty());
    }

    #[test]
    fn test_parse_missing_id() {
        let json = r#"{
            "id": "",
            "name": "Test",
            "version": "0.1.0",
            "description": "Test",
            "entry": "main.js"
        }"#;
        assert!(PluginManifest::parse(json).is_err());
    }

    #[test]
    fn test_permission_serialize() {
        let perm = PluginPermission::TerminalRead;
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(json, "\"terminal_read\"");
    }
}
