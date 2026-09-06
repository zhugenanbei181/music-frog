//! Desktop adapter for the Windows AppContainer loopback port.

use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::uwp::UwpPackageSnapshot;
use infiltrator_ports::error::PortError;
use infiltrator_ports::uwp_loopback::UwpLoopbackPort;

#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopUwpLoopbackPort;

fn unsupported() -> PortError {
    PortError::unsupported(
        Capability::UwpLoopback,
        "Windows CheckNetIsolation is unavailable on this host",
    )
}

fn map_error(error: anyhow::Error) -> PortError {
    PortError::Failed(error.to_string())
}

#[async_trait]
impl UwpLoopbackPort for DesktopUwpLoopbackPort {
    async fn scan(&self) -> Result<Vec<UwpPackageSnapshot>, PortError> {
        if !cfg!(windows) {
            return Err(unsupported());
        }
        crate::uwp_loopback::try_list_app_containers()
            .map(|packages| {
                packages
                    .into_iter()
                    .map(|package| UwpPackageSnapshot {
                        sid: package.sid,
                        display_name: package.display_name,
                        package_family_name: package.package_family_name,
                        loopback_exempt: package.loopback_exempt,
                    })
                    .collect()
            })
            .map_err(map_error)
    }

    async fn set_exempt(&self, sid: &str, exempt: bool) -> Result<(), PortError> {
        if !cfg!(windows) {
            return Err(unsupported());
        }
        crate::uwp_loopback::set_loopback_exempt(sid, exempt).map_err(map_error)
    }

    async fn set_all(&self, exempt: bool) -> Result<(), PortError> {
        if !cfg!(windows) {
            return Err(unsupported());
        }
        if exempt {
            crate::uwp_loopback::exempt_all()
        } else {
            crate::uwp_loopback::clear_all()
        }
        .map_err(map_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[tokio::test]
    async fn non_windows_host_reports_typed_unsupported_instead_of_empty_scan() {
        let result = DesktopUwpLoopbackPort.scan().await;
        assert!(matches!(
            result,
            Err(PortError::Unsupported {
                capability: Capability::UwpLoopback,
                ..
            })
        ));
    }
}
