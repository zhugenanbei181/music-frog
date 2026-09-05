//! Desktop port-conflict adapter.
//!
//! Detection reports the listener when the operating system can identify it.
//! Repair relocates MusicFrog's own bindings through the config manager; it
//! never terminates an unverified third-party PID from a UI action.

use infiltrator_contract::port_conflict::{
    PortBinding, PortConflict, PortConflictSnapshot,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::port_conflict::PortConflictPort;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::process::Command;
use std::path::PathBuf;
use yaml_rust2::YamlLoader;

pub struct DesktopPortConflict {
    home: PathBuf,
}

impl DesktopPortConflict {
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    async fn config(&self) -> Result<ConfigManager<DefaultCredentialStore>, PortError> {
        infiltrator_core::settings_io::app_config_manager_in(&self.home)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn bindings(&self) -> Result<Vec<(PortBinding, u16)>, PortError> {
        let config = self.config().await?;
        let profile = config
            .get_current()
            .await
            .map_err(config_error)?;
        let content = config.load(&profile).await.map_err(config_error)?;
        let document = YamlLoader::load_from_str(&content)
            .map_err(|error| PortError::Failed(format!("invalid profile YAML: {error}")))?
            .into_iter()
            .next()
            .unwrap_or(yaml_rust2::Yaml::Null);
        let proxy_port = document["mixed-port"]
            .as_i64()
            .and_then(|port| u16::try_from(port).ok())
            .or_else(|| {
                document["port"]
                    .as_i64()
                    .and_then(|port| u16::try_from(port).ok())
            });
        let controller_port = config
            .get_external_controller()
            .await
            .map_err(config_error)
            .and_then(|url| {
                mihomo_config::port::parse_port_from_addr(&url).ok_or_else(|| {
                    PortError::Failed(format!("cannot parse controller port from {url}"))
                })
            })?;

        let mut bindings = Vec::with_capacity(2);
        if let Some(port) = proxy_port.filter(|port| *port != 0) {
            bindings.push((PortBinding::MixedProxy, port));
        }
        if controller_port != 0 {
            bindings.push((PortBinding::Controller, controller_port));
        }
        Ok(bindings)
    }

    fn observe(binding: PortBinding, port: u16) -> PortConflict {
        let available = mihomo_config::port::is_port_available(port);
        let (owner_pid, owner_name) = if available {
            (None, None)
        } else {
            owner_for_port(port)
                .map(|owner| (Some(owner.pid), Some(owner.name)))
                .unwrap_or((None, None))
        };
        let can_release = owner_pid
            .zip(owner_name.as_deref())
            .is_some_and(|(pid, name)| pid != std::process::id() && is_mihomo_name(name));
        PortConflict {
            binding,
            port,
            available,
            owner_pid,
            owner_name,
            can_release,
        }
    }
}

#[async_trait::async_trait]
impl PortConflictPort for DesktopPortConflict {
    async fn snapshot(&self) -> Result<PortConflictSnapshot, PortError> {
        let conflicts = self
            .bindings()
            .await?
            .into_iter()
            .map(|(binding, port)| Self::observe(binding, port))
            .collect();
        Ok(PortConflictSnapshot {
            revision: 1,
            conflicts,
        })
    }

    async fn repair(&self) -> Result<PortConflictSnapshot, PortError> {
        let config = self.config().await?;
        config.ensure_proxy_ports().await.map_err(config_error)?;
        config
            .ensure_external_controller()
            .await
            .map_err(config_error)?;
        self.snapshot().await
    }
}

struct PortOwner {
    pid: u32,
    name: String,
}

#[cfg(unix)]
fn owner_for_port(port: u16) -> Option<PortOwner> {
    let output = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fpc"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_lsof_owner(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(windows)]
fn owner_for_port(port: u16) -> Option<PortOwner> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_netstat_owner(&String::from_utf8_lossy(&output.stdout), port)
}

#[cfg(not(any(unix, windows)))]
fn owner_for_port(port: u16) -> Option<PortOwner> {
    let _ = port;
    None
}

fn parse_lsof_owner(output: &str) -> Option<PortOwner> {
    let mut pid = None;
    let mut name = None;
    for line in output.lines() {
        if let Some(value) = line.strip_prefix('p') {
            pid = value.parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix('c') {
            name = Some(value.to_owned());
        }
    }
    Some(PortOwner {
        pid: pid?,
        name: name.unwrap_or_else(|| "unknown".to_owned()),
    })
}

#[cfg(windows)]
fn parse_netstat_owner(output: &str, port: u16) -> Option<PortOwner> {
    let needle = format!(":{port}");
    output.lines().find_map(|line| {
        let columns: Vec<&str> = line.split_whitespace().collect();
        (columns.first() == Some(&"TCP")
            && columns.get(1).is_some_and(|value| value.ends_with(&needle))
            && columns.get(3) == Some(&"LISTENING"))
            .then(|| PortOwner {
                pid: columns.get(4)?.parse().ok()?,
                name: "unknown".to_owned(),
            })
    })
}

fn is_mihomo_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "mihomo" || lower == "mihomo.exe"
}

fn config_error(error: mihomo_api::error::MihomoError) -> PortError {
    match error {
        mihomo_api::error::MihomoError::Io(error) => PortError::Io(error.to_string()),
        mihomo_api::error::MihomoError::NotFound(message) => PortError::NotFound(message),
        other => PortError::Failed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn lsof_owner_parser_keeps_pid_and_process_name() {
        let owner = parse_lsof_owner("p4242\ncmihomo\n");
        assert_eq!(owner.as_ref().map(|value| value.pid), Some(4242));
        assert_eq!(owner.as_ref().map(|value| value.name.as_str()), Some("mihomo"));
    }

    #[test]
    fn only_verified_mihomo_owners_are_marked_safe_to_release() {
        assert!(is_mihomo_name("mihomo"));
        assert!(is_mihomo_name("mihomo.exe"));
        assert!(!is_mihomo_name("chrome"));
        assert!(!is_mihomo_name("mihomo-helper"));
    }

    #[test]
    fn arbitrary_binary_paths_are_not_used_by_owner_parser() {
        let _ = Path::new("/tmp/mihomo");
        assert!(parse_lsof_owner("p99\n").is_some());
    }

    #[test]
    fn available_port_has_no_release_owner() {
        let conflict = DesktopPortConflict::observe(PortBinding::MixedProxy, 0);
        assert!(conflict.available);
        assert!(conflict.owner_pid.is_none());
        assert!(!conflict.can_release);
    }
}
