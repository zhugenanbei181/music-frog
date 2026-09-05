//! Desktop system HTTP/SOCKS proxy port.

use async_trait::async_trait;
use infiltrator_contract::system_proxy::SystemProxyObservation;
use infiltrator_ports::error::PortError;
use infiltrator_ports::system_proxy::SystemProxyPort;

/// Stateless desktop adapter over the OS-specific proxy implementations.
/// Windows registry, Linux GSettings/KDE/environment and macOS
/// `networksetup` remain behind `crate::proxy`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopSystemProxy;

impl DesktopSystemProxy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SystemProxyPort for DesktopSystemProxy {
    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
        tokio::task::spawn_blocking(crate::proxy::read_system_proxy_state)
            .await
            .map_err(|error| PortError::Io(format!("system proxy probe worker failed: {error}")))?
            .map(to_observation)
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn apply(
        &self,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<(), PortError> {
        tokio::task::spawn_blocking(move || {
            if bypass.is_some() {
                crate::proxy::apply_system_proxy_with_bypass(endpoint.as_deref(), bypass.as_deref())
            } else {
                crate::proxy::apply_system_proxy(endpoint.as_deref())
            }
        })
        .await
        .map_err(|error| PortError::Io(format!("system proxy apply worker failed: {error}")))?
        .map_err(|error| PortError::Io(error.to_string()))
    }
}

fn to_observation(state: crate::proxy::SystemProxyState) -> SystemProxyObservation {
    SystemProxyObservation {
        enabled: state.enabled,
        endpoint: state.endpoint,
        bypass: state.bypass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_proxy_adapter_preserves_os_observation_shape() {
        let observation = to_observation(crate::proxy::SystemProxyState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        });
        assert!(observation.enabled);
        assert_eq!(observation.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(observation.bypass.as_deref(), Some("localhost"));
    }
}
