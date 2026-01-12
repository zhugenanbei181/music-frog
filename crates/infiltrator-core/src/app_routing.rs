//! Per-app proxy routing configuration
//!
//! Manages which apps should be routed through the VPN/proxy.
//! Supports whitelist (proxy selected) and blacklist (bypass selected) modes.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

use mihomo_platform::get_home_dir;

/// Routing mode for per-app proxy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppRoutingMode {
    /// Route all apps through VPN (default)
    #[default]
    ProxyAll,
    /// Only route selected apps through VPN
    ProxySelected,
    /// Bypass selected apps (route all others)
    BypassSelected,
}

/// Per-app routing configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppRoutingConfig {
    /// Routing mode
    pub mode: AppRoutingMode,
    /// Selected package names (interpretation depends on mode)
    #[serde(default)]
    pub packages: HashSet<String>,
}

impl AppRoutingConfig {
    /// Check if a package should be routed through VPN
    pub fn should_proxy(&self, package: &str) -> bool {
        match self.mode {
            AppRoutingMode::ProxyAll => true,
            AppRoutingMode::ProxySelected => self.packages.contains(package),
            AppRoutingMode::BypassSelected => !self.packages.contains(package),
        }
    }

    /// Get packages that should be allowed through VPN (for VpnService.Builder)
    /// Returns None if all apps should be proxied
    pub fn get_allowed_packages(&self) -> Option<Vec<String>> {
        match self.mode {
            AppRoutingMode::ProxyAll => None,
            AppRoutingMode::ProxySelected => {
                if self.packages.is_empty() {
                    None // Fallback to proxy all
                } else {
                    Some(self.packages.iter().cloned().collect())
                }
            }
            AppRoutingMode::BypassSelected => None, // Android VPN doesn't support disallowed list well
        }
    }

    /// Get packages that should be disallowed (bypassed)
    /// Returns None if using whitelist mode
    pub fn get_disallowed_packages(&self) -> Option<Vec<String>> {
        match self.mode {
            AppRoutingMode::BypassSelected => {
                if self.packages.is_empty() {
                    None
                } else {
                    Some(self.packages.iter().cloned().collect())
                }
            }
            _ => None,
        }
    }
}

/// Get the path to app routing config file
fn config_path() -> anyhow::Result<PathBuf> {
    let home = get_home_dir()?;
    Ok(home.join("app_routing.toml"))
}

/// Load app routing configuration
pub fn load_app_routing() -> anyhow::Result<AppRoutingConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppRoutingConfig::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let config: AppRoutingConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Save app routing configuration
pub fn save_app_routing(config: &AppRoutingConfig) -> anyhow::Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Add a package to the selected list
pub fn add_package(package: &str) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.packages.insert(package.to_string());
    save_app_routing(&config)
}

/// Remove a package from the selected list
pub fn remove_package(package: &str) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.packages.remove(package);
    save_app_routing(&config)
}

/// Set the routing mode
pub fn set_routing_mode(mode: AppRoutingMode) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.mode = mode;
    save_app_routing(&config)
}

/// Set multiple packages at once
pub fn set_packages(packages: Vec<String>) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.packages = packages.into_iter().collect();
    save_app_routing(&config)
}

/// Toggle a package selection
pub fn toggle_package(package: &str) -> anyhow::Result<bool> {
    let mut config = load_app_routing()?;
    let is_selected = if config.packages.contains(package) {
        config.packages.remove(package);
        false
    } else {
        config.packages.insert(package.to_string());
        true
    };
    save_app_routing(&config)?;
    Ok(is_selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_proxy() {
        let mut config = AppRoutingConfig::default();
        
        // ProxyAll mode
        assert!(config.should_proxy("com.example.app"));
        
        // ProxySelected mode
        config.mode = AppRoutingMode::ProxySelected;
        config.packages.insert("com.example.app".to_string());
        assert!(config.should_proxy("com.example.app"));
        assert!(!config.should_proxy("com.other.app"));
        
        // BypassSelected mode
        config.mode = AppRoutingMode::BypassSelected;
        assert!(!config.should_proxy("com.example.app"));
        assert!(config.should_proxy("com.other.app"));
    }

    #[test]
    fn test_get_allowed_packages() {
        let mut config = AppRoutingConfig::default();
        
        // ProxyAll returns None
        assert!(config.get_allowed_packages().is_none());
        
        // ProxySelected with packages
        config.mode = AppRoutingMode::ProxySelected;
        config.packages.insert("com.example.app".to_string());
        let allowed = config.get_allowed_packages().unwrap();
        assert!(allowed.contains(&"com.example.app".to_string()));
    }
}
