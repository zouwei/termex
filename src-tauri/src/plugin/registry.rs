use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;

use super::manifest::PluginManifest;
use super::PluginError;

/// State of an installed plugin.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    Enabled,
    Disabled,
}

/// An installed plugin with its metadata and state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub state: PluginState,
    pub install_path: String,
}

/// Manages installed plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, InstalledPlugin>,
    plugins_dir: PathBuf,
}

impl PluginRegistry {
    /// Creates an empty registry (fallback when disk access fails).
    pub fn new_empty() -> Self {
        Self {
            plugins: HashMap::new(),
            plugins_dir: PathBuf::from("/tmp/termex-plugins"),
        }
    }

    /// Creates a new registry at the default plugins directory.
    pub fn new() -> Result<Self, PluginError> {
        let plugins_dir = plugins_dir()?;
        std::fs::create_dir_all(&plugins_dir)?;

        let mut registry = Self {
            plugins: HashMap::new(),
            plugins_dir,
        };

        registry.scan()?;
        Ok(registry)
    }

    /// Scans the plugins directory for installed plugins.
    fn scan(&mut self) -> Result<(), PluginError> {
        self.plugins.clear();

        if !self.plugins_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.json");
            if !manifest_path.exists() {
                continue;
            }

            let json = std::fs::read_to_string(&manifest_path)?;
            match PluginManifest::parse(&json) {
                Ok(manifest) => {
                    let id = manifest.id.clone();
                    self.plugins.insert(
                        id,
                        InstalledPlugin {
                            manifest,
                            state: PluginState::Enabled,
                            install_path: path.to_string_lossy().to_string(),
                        },
                    );
                }
                Err(_) => continue,
            }
        }

        Ok(())
    }

    /// Lists all installed plugins.
    pub fn list(&self) -> Vec<InstalledPlugin> {
        self.plugins.values().cloned().collect()
    }

    /// Installs a plugin from a directory path.
    pub fn install(&mut self, source_path: &str) -> Result<InstalledPlugin, PluginError> {
        let manifest_path = PathBuf::from(source_path).join("plugin.json");
        let json = std::fs::read_to_string(&manifest_path)?;
        let manifest = PluginManifest::parse(&json)?;

        if self.plugins.contains_key(&manifest.id) {
            return Err(PluginError::AlreadyInstalled(manifest.id));
        }

        let dest = self.plugins_dir.join(&manifest.id);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        copy_dir_recursive(source_path, &dest)?;

        let plugin = InstalledPlugin {
            manifest: manifest.clone(),
            state: PluginState::Enabled,
            install_path: dest.to_string_lossy().to_string(),
        };

        self.plugins.insert(manifest.id, plugin.clone());
        Ok(plugin)
    }

    /// Uninstalls a plugin by ID.
    pub fn uninstall(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .remove(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        let path = PathBuf::from(&plugin.install_path);
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }

        Ok(())
    }

    /// Enables a plugin.
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        plugin.state = PluginState::Enabled;
        Ok(())
    }

    /// Disables a plugin.
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        plugin.state = PluginState::Disabled;
        Ok(())
    }
}

/// Returns the plugins directory path.
fn plugins_dir() -> Result<PathBuf, PluginError> {
    let data_dir = dirs::data_dir().ok_or_else(|| {
        PluginError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no data directory",
        ))
    })?;
    Ok(data_dir.join("termex").join("plugins"))
}

/// Recursively copies a directory.
fn copy_dir_recursive(src: &str, dest: &PathBuf) -> Result<(), PluginError> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path.to_string_lossy(), &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_state_serialize() {
        let state = PluginState::Enabled;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"enabled\"");
    }

    #[test]
    fn test_installed_plugin_serialize() {
        let plugin = InstalledPlugin {
            manifest: PluginManifest {
                id: "test".into(),
                name: "Test Plugin".into(),
                version: "1.0.0".into(),
                description: "A test plugin".into(),
                author: None,
                homepage: None,
                permissions: vec![],
                entry: "index.js".into(),
                min_termex_version: None,
            },
            state: PluginState::Enabled,
            install_path: "/tmp/plugins/test".into(),
        };
        let json = serde_json::to_string(&plugin).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"state\":\"enabled\""));
    }
}
