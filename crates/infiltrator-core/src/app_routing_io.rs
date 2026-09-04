//! Filesystem adapter for the per-app routing configuration.
//!
//! The routing model, classification, process aliases and CIDR logic live in
//! `infiltrator-domain::app_routing`. This module owns only the legacy
//! `app_routing.toml` location and mutation helpers.

use infiltrator_domain::app_routing::{AppRoutingConfig, AppRoutingMode};
use mihomo_platform::paths::get_home_dir;
use std::path::PathBuf;

fn config_path() -> anyhow::Result<PathBuf> {
    let home = get_home_dir()?;
    Ok(home.join("app_routing.toml"))
}

pub fn load_app_routing() -> anyhow::Result<AppRoutingConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppRoutingConfig::default());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}

pub fn save_app_routing(config: &AppRoutingConfig) -> anyhow::Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn add_package(package: &str) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.packages.insert(package.to_string());
    save_app_routing(&config)
}

pub fn remove_package(package: &str) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.packages.remove(package);
    save_app_routing(&config)
}

pub fn set_routing_mode(mode: AppRoutingMode) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.mode = mode;
    save_app_routing(&config)
}

pub fn set_packages(packages: Vec<String>) -> anyhow::Result<()> {
    let mut config = load_app_routing()?;
    config.packages = packages.into_iter().collect();
    save_app_routing(&config)
}

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
