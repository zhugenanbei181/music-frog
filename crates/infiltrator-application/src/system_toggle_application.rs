//! Shared application policy for system-level quick-toggle interactions.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot, SystemToggleState};

/// Converts one canonical surface snapshot into the compact control model and
/// fences actions that are not backed by a current host readback.
pub struct SystemToggleApplication;

impl SystemToggleApplication {
    pub fn from_surface(snapshot: &SurfaceSnapshot) -> SystemToggleSnapshot {
        let system_proxy = match &snapshot.system_proxy.status {
            SystemProxyStatus::Unknown => SystemToggleState::Unknown,
            SystemProxyStatus::Disabled => SystemToggleState::Disabled,
            SystemProxyStatus::Enabled => SystemToggleState::Enabled,
            SystemProxyStatus::Unsupported { failure } => SystemToggleState::Unsupported {
                failure: failure.clone(),
            },
            SystemProxyStatus::Failed { failure } => SystemToggleState::Failed {
                failure: failure.clone(),
            },
        };
        let tun = match (
            &snapshot.pages.settings.status,
            &snapshot.pages.settings.data,
        ) {
            (PageStatus::Ready | PageStatus::Empty, Some(settings)) => {
                SystemToggleState::from_enabled(settings.tun_enabled)
            }
            (PageStatus::Unavailable { failure }, _) => SystemToggleState::Unsupported {
                failure: failure.clone(),
            },
            (PageStatus::Failed { failure }, _) => SystemToggleState::Failed {
                failure: failure.clone(),
            },
            _ => SystemToggleState::Unknown,
        };
        SystemToggleSnapshot {
            system_proxy,
            tun,
            revision: snapshot.revision,
        }
    }

    pub fn from_legacy(system_proxy: bool, tun: Option<bool>) -> SystemToggleSnapshot {
        SystemToggleSnapshot::from_legacy(system_proxy, tun, 0)
    }

    pub fn intent(
        snapshot: &SystemToggleSnapshot,
        toggle: SystemToggle,
        desired: bool,
    ) -> Result<CommandIntent, Failure> {
        match snapshot.state(toggle) {
            SystemToggleState::Enabled | SystemToggleState::Disabled => {}
            SystemToggleState::Pending { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "system toggle action is already in flight",
                    false,
                ));
            }
            SystemToggleState::Unknown => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "system toggle has no authoritative host readback",
                    true,
                ));
            }
            SystemToggleState::Unsupported { failure } | SystemToggleState::Failed { failure } => {
                return Err(failure.clone());
            }
        }
        Ok(match toggle {
            SystemToggle::SystemProxy => CommandIntent::SetSystemProxy { enabled: desired },
            SystemToggle::Tun => CommandIntent::ToggleTun { enabled: desired },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::system_toggle::SystemToggle;

    #[test]
    fn same_policy_maps_both_controls_to_shared_intents() {
        let snapshot = SystemToggleApplication::from_legacy(true, Some(false));
        assert_eq!(
            SystemToggleApplication::intent(&snapshot, SystemToggle::SystemProxy, false),
            Ok(CommandIntent::SetSystemProxy { enabled: false })
        );
        assert_eq!(
            SystemToggleApplication::intent(&snapshot, SystemToggle::Tun, true),
            Ok(CommandIntent::ToggleTun { enabled: true })
        );
    }

    #[test]
    fn pending_or_unknown_controls_fail_closed() {
        let snapshot = SystemToggleApplication::from_legacy(false, None)
            .with_pending(SystemToggle::SystemProxy, true);
        assert_eq!(
            SystemToggleApplication::intent(&snapshot, SystemToggle::SystemProxy, false)
                .expect_err("pending action must be fenced")
                .code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            SystemToggleApplication::intent(&snapshot, SystemToggle::Tun, true)
                .expect_err("unknown action must be fenced")
                .code,
            ErrorCode::NotReady
        );
    }
}
