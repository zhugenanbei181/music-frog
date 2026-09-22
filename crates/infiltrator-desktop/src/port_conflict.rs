//! Desktop port-conflict adapter.
//!
//! Detection reports the listener when the operating system can identify it.
//! Repair relocates MusicFrog's own bindings through the config manager; it
//! never terminates an unverified third-party PID from a UI action.

use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_ports::error::PortError;
use infiltrator_ports::port_conflict::PortConflictPort;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::path::PathBuf;
use std::process::Command;
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
        let profile = config.get_current().await.map_err(config_error)?;
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

        let mut bindings = Vec::with_capacity(3);
        if let Some(port) = proxy_port.filter(|port| *port != 0) {
            bindings.push((PortBinding::MixedProxy, port));
        }
        if controller_port != 0 {
            bindings.push((PortBinding::Controller, controller_port));
        }
        // DUAL-14-13: mihomo's own DNS listener. It is a string address
        // (`dns.listen: 0.0.0.0:1053`), so only a parsable `host:port` is
        // observed; an absent or unparsable value publishes no fact at all.
        if let Some(port) = dns_listen_port(&document) {
            bindings.push((PortBinding::DnsListen, port));
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
        // DUAL-14-13: the same repair relocates a taken DNS listener port.
        config
            .ensure_dns_listen_port()
            .await
            .map_err(config_error)?;
        self.snapshot().await
    }
}

/// The `dns.listen` port of a profile document, when it is a parsable address.
fn dns_listen_port(document: &yaml_rust2::Yaml) -> Option<u16> {
    mihomo_config::port::split_listen_addr(document["dns"]["listen"].as_str()?)
        .map(|(_, port)| port)
        .filter(|port| *port != 0)
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
        if columns.first() == Some(&"TCP")
            && columns.get(1).is_some_and(|value| value.ends_with(&needle))
            && columns.get(3) == Some(&"LISTENING")
        {
            Some(PortOwner {
                pid: columns.get(4)?.parse().ok()?,
                name: "unknown".to_owned(),
            })
        } else {
            None
        }
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
        assert_eq!(
            owner.as_ref().map(|value| value.name.as_str()),
            Some("mihomo")
        );
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

    #[test]
    fn the_dns_listen_address_is_decoded_from_the_profile_document() {
        let yaml = "dns:\n  listen: 127.0.0.1:1053\n";
        let document = yaml_rust2::YamlLoader::load_from_str(yaml)
            .expect("yaml")
            .into_iter()
            .next()
            .expect("document");
        assert_eq!(dns_listen_port(&document), Some(1053));

        let empty_listen = yaml_rust2::YamlLoader::load_from_str("dns:\n  listen: ''\n")
            .expect("yaml")
            .into_iter()
            .next()
            .expect("document");
        assert_eq!(dns_listen_port(&empty_listen), None);

        let no_dns = yaml_rust2::YamlLoader::load_from_str("port: 7890\n")
            .expect("yaml")
            .into_iter()
            .next()
            .expect("document");
        assert_eq!(dns_listen_port(&no_dns), None);

        let portless = yaml_rust2::YamlLoader::load_from_str("dns:\n  listen: '1053'\n")
            .expect("yaml")
            .into_iter()
            .next()
            .expect("document");
        assert_eq!(dns_listen_port(&portless), None);
    }

    #[tokio::test]
    async fn a_bound_dns_listen_port_is_observed_and_relocated() {
        let home = tempfile::tempdir().expect("temp home");
        let manager = infiltrator_core::settings_io::app_config_manager_in(home.path())
            .await
            .expect("config manager");
        manager
            .ensure_default_config()
            .await
            .expect("default config");
        let profile = manager.get_current().await.expect("profile");

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
        let port = listener.local_addr().expect("addr").port();
        manager
            .save(
                &profile,
                &format!("port: 7890\ndns:\n  listen: 127.0.0.1:{port}\n"),
            )
            .await
            .expect("save profile");

        let adapter = DesktopPortConflict::new(home.path().to_path_buf());
        let snapshot = adapter.snapshot().await.expect("snapshot");
        let dns = snapshot
            .conflicts
            .iter()
            .find(|conflict| conflict.binding == PortBinding::DnsListen)
            .expect("the dns.listen port is observed");
        assert_eq!(dns.port, port);
        assert!(
            !dns.available,
            "the reserved port must be reported as taken"
        );
        assert_ne!(
            dns.owner_name.as_deref(),
            Some("mihomo"),
            "the listener this test holds is never reported as the kernel"
        );
        assert!(
            !dns.can_release,
            "a UI action never terminates the host process holding the port"
        );

        // The port is still held, so the repair must relocate the listener.
        let repaired = adapter.repair().await.expect("repair");
        let dns = repaired
            .conflicts
            .iter()
            .find(|conflict| conflict.binding == PortBinding::DnsListen)
            .expect("the dns.listen port is observed after repair");
        assert!(
            dns.available,
            "repair must relocate the taken dns.listen port"
        );
        assert_ne!(dns.port, port);

        let content = manager.load(&profile).await.expect("reload profile");
        assert!(
            content.contains(&format!("127.0.0.1:{}", dns.port)),
            "the relocated listener must be persisted: {content}"
        );
        assert!(content.contains("dns:"), "the dns block must survive");
        drop(listener);
    }
}
