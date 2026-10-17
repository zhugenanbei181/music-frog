//! Default-config bootstrap and proxy-port conflict repair for the active
//! profile.

use super::ConfigManager;
use crate::port::{find_available_port, is_port_available, split_listen_addr};
use crate::yaml;
use crate::yaml_ast::update_nested_field;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::profile_document_identity::ProfileId;
use mihomo_driver::error::{MihomoError, Result};

impl<S: SecureStore> ConfigManager<S> {
    /// Create a missing Mihomo profile during legacy selection migration. The
    /// caller holds the product reference lock; take only the engine profile
    /// lock here to preserve root -> profile order without reacquiring root.
    pub(crate) async fn ensure_profile_document(&self, profile: &str) -> Result<()> {
        let path = self.config_dir().join(format!("{}.yaml", profile));
        if !path.exists() {
            log::info!("Mihomo profile '{}' not found, creating...", profile);
            let port = find_available_port(9090).ok_or_else(|| {
                MihomoError::Config("No available ports found in range 9090-9190".to_string())
            })?;
            let default_config = format!(
                r#"# mihomo configuration
port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:{}
"#,
                port
            );

            let _profile_guard = self.lock_profile_writes().await?;
            self.save_profile_locked(profile, &default_config).await?;
            log::info!("Created default config at: {}", path.display());
        }
        Ok(())
    }

    pub async fn ensure_proxy_ports(&self) -> Result<()> {
        let profile = self
            .profile_store
            .get_current()
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile_id =
            ProfileId::new(profile).map_err(|error| MihomoError::Config(error.to_string()))?;
        self.ensure_proxy_ports_for_profile(&profile_id).await
    }

    pub async fn ensure_proxy_ports_for_profile(&self, profile: &ProfileId) -> Result<()> {
        let workspace = self
            .read_workspace(profile)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let content = String::from_utf8(workspace.document.raw_bytes.clone())
            .map_err(|error| MihomoError::Config(format!("profile YAML is not UTF-8: {error}")))?;
        let mut doc = yaml::load_yaml(&content)?;
        let mut changed = false;

        for key in ["mixed-port", "port", "socks-port"] {
            let port = match yaml::get_u16(&doc, key) {
                Some(port) => port,
                None => continue,
            };
            if port == 0 {
                continue;
            }
            if !is_port_available(port) {
                let fallback = find_available_port(port).ok_or_else(|| {
                    MihomoError::Config(format!("No available port found for {key}"))
                })?;
                if fallback != port {
                    yaml::set_u16(&mut doc, key, fallback)?;
                    log::warn!("{} {} is in use, switched to {}", key, port, fallback);
                    changed = true;
                }
            }
        }

        if changed {
            let updated = yaml::to_string(&doc)?;
            self.persist_derived_workspace_content(&workspace, &updated)
                .await?;
        }

        Ok(())
    }

    /// DUAL-14-13: relocate a taken `dns.listen` port.
    ///
    /// The DNS listener binds TCP and UDP; the availability probe used here is
    /// the same TCP check the proxy-port repair uses, which is stated instead
    /// of implied. A profile without `dns.listen` is left untouched, and the
    /// host part of the address is preserved.
    pub async fn ensure_dns_listen_port(&self) -> Result<()> {
        let profile = self
            .profile_store
            .get_current()
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile_id =
            ProfileId::new(profile).map_err(|error| MihomoError::Config(error.to_string()))?;
        self.ensure_dns_listen_port_for_profile(&profile_id).await
    }

    pub async fn ensure_dns_listen_port_for_profile(&self, profile: &ProfileId) -> Result<()> {
        let workspace = self
            .read_workspace(profile)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let content = String::from_utf8(workspace.document.raw_bytes.clone())
            .map_err(|error| MihomoError::Config(format!("profile YAML is not UTF-8: {error}")))?;
        let doc = yaml::load_yaml(&content)?;
        let Some(raw) = doc["dns"]["listen"].as_str() else {
            return Ok(());
        };
        let Some((host, port)) = split_listen_addr(raw) else {
            return Ok(());
        };
        if port == 0 || is_port_available(port) {
            return Ok(());
        }
        let fallback = find_available_port(port).ok_or_else(|| {
            MihomoError::Config(format!("No available port found for dns.listen {port}"))
        })?;
        if fallback == port {
            return Ok(());
        }
        let host = if host.is_empty() { "0.0.0.0" } else { &host };
        let updated =
            update_nested_field(&content, &["dns", "listen"], &format!("{host}:{fallback}"))
                .map_err(|error| MihomoError::Config(error.to_string()))?;
        if updated != content {
            self.persist_derived_workspace_content(&workspace, &updated)
                .await?;
            log::warn!("dns.listen {port} is in use, switched to {fallback}");
        }
        Ok(())
    }
}
