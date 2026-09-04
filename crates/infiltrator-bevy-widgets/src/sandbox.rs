//! Sandboxed runtime container for third-party WebAssembly / JS micro-frontend dashboard widgets.

use std::collections::HashMap;

/// Resource quota for a sandboxed widget instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SandboxQuota {
    pub max_memory_bytes: usize,
    pub max_instructions_per_frame: u64,
    pub allow_network_read: bool,
}

impl Default for SandboxQuota {
    fn default() -> Self {
        Self {
            max_memory_bytes: 4 * 1024 * 1024, // 4 MB limit
            max_instructions_per_frame: 50_000,
            allow_network_read: false,
        }
    }
}

/// A sandboxed third-party widget instance descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct SandboxedWidgetInstance {
    pub widget_id: String,
    pub plugin_name: String,
    pub version: String,
    pub quota: SandboxQuota,
    pub state_store: HashMap<String, String>,
}

impl SandboxedWidgetInstance {
    pub fn new(id: impl Into<String>, plugin_name: impl Into<String>) -> Self {
        Self {
            widget_id: id.into(),
            plugin_name: plugin_name.into(),
            version: "1.0.0".to_string(),
            quota: SandboxQuota::default(),
            state_store: HashMap::new(),
        }
    }

    pub fn write_state(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        if self.state_store.len() >= 128 {
            return false;
        }
        self.state_store.insert(key.into(), value.into());
        true
    }

    pub fn read_state(&self, key: &str) -> Option<&str> {
        self.state_store.get(key).map(|s| s.as_str())
    }
}

/// Explicit permissions a third-party micro-frontend widget can request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetPermission {
    ReadTrafficStats,
    ReadNodeList,
    SwitchProxyNode,
    ManageProfiles,
    ExecuteNetworkDiagnostics,
}

/// Metadata manifest describing a community micro-frontend dashboard widget.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub min_bevy_version: String,
    pub permissions: Vec<WidgetPermission>,
}

impl WidgetManifest {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: "0.1.0".to_string(),
            author: "Community".to_string(),
            min_bevy_version: "0.19.1".to_string(),
            permissions: Vec::new(),
        }
    }

    pub fn with_permission(mut self, perm: WidgetPermission) -> Self {
        if !self.permissions.contains(&perm) {
            self.permissions.push(perm);
        }
        self
    }

    pub fn has_permission(&self, perm: WidgetPermission) -> bool {
        self.permissions.contains(&perm)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.id.trim().is_empty() {
            return Err("Widget ID cannot be empty");
        }
        if self.name.trim().is_empty() {
            return Err("Widget name cannot be empty");
        }
        if self.min_bevy_version != "0.19.1" {
            return Err("Incompatible Bevy version dependency");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_manifest_and_permissions() {
        let manifest = WidgetManifest::new("custom-speedtest", "Live Speed Test")
            .with_permission(WidgetPermission::ReadTrafficStats)
            .with_permission(WidgetPermission::ExecuteNetworkDiagnostics);

        assert!(manifest.validate().is_ok());
        assert!(manifest.has_permission(WidgetPermission::ReadTrafficStats));
        assert!(manifest.has_permission(WidgetPermission::ExecuteNetworkDiagnostics));
        assert!(!manifest.has_permission(WidgetPermission::ManageProfiles));

        // Invalid version check
        let mut bad_version = manifest;
        bad_version.min_bevy_version = "0.20.0".to_string();
        assert!(bad_version.validate().is_err());
    }
}
