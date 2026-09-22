//! Default-config bootstrap and proxy-port conflict repair for the active
//! profile.

use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};

use super::ConfigManager;
use crate::port::{find_available_port, is_port_available};
use crate::yaml;

impl<S: SecureStore> ConfigManager<S> {
    /// Ensure a default config file exists, create one if it doesn't
    pub async fn ensure_default_config(&self) -> Result<()> {
        let profile = self.get_current().await?;
        let path = self.config_dir.join(format!("{}.yaml", profile));

        if !path.exists() {
            log::info!("Default config '{}' not found, creating...", profile);

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

            self.save(&profile, &default_config).await?;
            log::info!("Created default config at: {}", path.display());
        }

        Ok(())
    }

    pub async fn ensure_proxy_ports(&self) -> Result<()> {
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
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
            self.save(&profile, &updated).await?;
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
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
        let doc = yaml::load_yaml(&content)?;
        let Some(raw) = doc["dns"]["listen"].as_str() else {
            return Ok(());
        };
        let Some((host, port)) = crate::port::split_listen_addr(raw) else {
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
        let updated = crate::yaml_ast::update_nested_field(
            &content,
            &["dns", "listen"],
            &format!("{host}:{fallback}"),
        )
        .map_err(|error| MihomoError::Config(error.to_string()))?;
        if updated != content {
            self.save(&profile, &updated).await?;
            log::warn!("dns.listen {port} is in use, switched to {fallback}");
        }
        Ok(())
    }
}
