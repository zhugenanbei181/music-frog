//! External-controller endpoint management for the active profile: read the
//! configured address, ensure it is usable, and rotate it when the port is
//! occupied.

use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use ring::rand::{SecureRandom, SystemRandom};

use super::ConfigManager;
use crate::port::{find_available_port, is_port_available, parse_port_from_addr};
use crate::yaml;

impl<S: SecureStore> ConfigManager<S> {
    pub async fn get_external_controller(&self) -> Result<String> {
        let profile = self.get_current().await?;
        log::debug!("Reading external-controller from profile: {}", profile);

        let content = self.load(&profile).await?;
        let config = yaml::load_yaml(&content)?;

        let controller = yaml::get_str(&config, "external-controller")
            .unwrap_or_else(|| "127.0.0.1:9090".to_string());

        let url = if controller.starts_with(':') {
            format!("http://127.0.0.1{}", controller)
        } else if controller.starts_with("http://") || controller.starts_with("https://") {
            controller.to_string()
        } else {
            format!("http://{}", controller)
        };

        log::debug!("External controller URL: {}", url);
        Ok(url)
    }

    /// Ensure external-controller is configured in the current profile
    /// If not present or port is occupied, add/update it with an available port
    pub async fn ensure_external_controller(&self) -> Result<String> {
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
        let mut config = yaml::load_yaml(&content)?;

        let needs_update = match yaml::get_str(&config, "external-controller") {
            Some(controller) => {
                // Parse the port from the controller address
                let addr = if controller.starts_with(':') {
                    format!("127.0.0.1{}", controller)
                } else {
                    controller.to_string()
                };

                match parse_port_from_addr(&addr) {
                    Some(port) => {
                        if !is_port_available(port) {
                            log::warn!("Port {} is occupied, finding alternative", port);
                            true
                        } else {
                            false
                        }
                    }
                    None => {
                        log::warn!("Invalid external-controller format: {}", controller);
                        true
                    }
                }
            }
            None => {
                log::info!("external-controller not found in config, adding default");
                true
            }
        };

        // Mihomo accepts the generated secret through the profile itself;
        // every concrete HTTP client then receives it as a Bearer header via
        // ProfileEndpointSource. Never leave a newly bootstrapped controller
        // unauthenticated, and never rotate a user-provided non-empty secret.
        let secret_updated = ensure_controller_secret(&mut config)?;

        if needs_update {
            let port = find_available_port(9090).ok_or_else(|| {
                MihomoError::Config("No available ports found in range 9090-9190".to_string())
            })?;

            let controller_addr = format!("127.0.0.1:{}", port);
            log::info!("Setting external-controller to {}", controller_addr);

            yaml::set_str(&mut config, "external-controller", &controller_addr)?;
        }

        if needs_update || secret_updated {
            let updated_content = yaml::to_string(&config)?;
            self.save(&profile, &updated_content).await?;
        }
        self.get_external_controller().await
    }

    /// Forcefully rotate the external-controller port to a new available one.
    /// Used when the service fails to start despite the port appearing available initially.
    pub async fn rotate_external_controller(&self) -> Result<String> {
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
        let mut config = yaml::load_yaml(&content)?;

        let current_port = yaml::get_str(&config, "external-controller")
            .and_then(|s| parse_port_from_addr(&s))
            .unwrap_or(9090);

        // Start searching from current_port + 1
        let new_port = find_available_port(current_port + 1).ok_or_else(|| {
            MihomoError::Config("No available ports found for rotation".to_string())
        })?;

        let controller_addr = format!("127.0.0.1:{}", new_port);
        log::info!(
            "Rotating external-controller from {} to {}",
            current_port,
            new_port
        );

        yaml::set_str(&mut config, "external-controller", &controller_addr)?;
        ensure_controller_secret(&mut config)?;
        let updated_content = yaml::to_string(&config)?;
        self.save(&profile, &updated_content).await?;

        Ok(format!("http://{}", controller_addr))
    }
}

fn ensure_controller_secret(config: &mut yaml_rust2::Yaml) -> Result<bool> {
    if yaml::get_str(config, "secret").is_some_and(|secret| !secret.trim().is_empty()) {
        return Ok(false);
    }

    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| MihomoError::Config("unable to generate controller secret".to_owned()))?;
    let mut secret = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(secret, "{byte:02x}");
    }
    yaml::set_str(config, "secret", &secret)?;
    Ok(true)
}
