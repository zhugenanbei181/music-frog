use crate::core_application::CoreApplication;
use infiltrator_contract::command::{CommandIntent, CommandResult, ProxyMode};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;

pub struct ProxyModeApplication;

impl ProxyModeApplication {
    pub async fn change(
        application: &CoreApplication,
        target: ProxyMode,
    ) -> Result<ProxyMode, Failure> {
        Self::command_receipt(
            target,
            application
                .execute(CommandIntent::SetProxyMode { mode: target })
                .await,
        )
    }

    pub fn command_receipt(target: ProxyMode, result: CommandResult) -> Result<ProxyMode, Failure> {
        result.into_unit().map(|()| target)
    }

    pub fn with_verified_mode(snapshot: &SurfaceSnapshot, actual: ProxyMode) -> SurfaceSnapshot {
        let mut snapshot = snapshot.clone();
        snapshot.revision = snapshot
            .revision
            .checked_add(1)
            .expect("surface revision exhausted");
        snapshot.core.proxy_mode = Some(actual);
        snapshot.runtime_control.mode = Some(actual);
        if let Some(page) = &mut snapshot.pages.overview.data {
            page.proxy_mode = Some(actual);
        }
        snapshot
    }

    pub fn from_surface(snapshot: &SurfaceSnapshot) -> ProxyModeSnapshot {
        let observed = &snapshot.runtime_control;
        let current = observed.mode.or(snapshot.core.proxy_mode);
        let failure = match &observed.status {
            RuntimeControlStatus::Failed { failure }
            | RuntimeControlStatus::Unsupported { failure } => Some(failure.clone()),
            _ => snapshot.core.failure.clone(),
        };
        let status = match (&observed.status, &snapshot.core.lifecycle, current) {
            (RuntimeControlStatus::Unsupported { .. }, _, _) => ProxyModeStatus::Unsupported,
            (RuntimeControlStatus::Failed { .. }, _, _) | (_, CoreLifecycle::Failed, _) => {
                ProxyModeStatus::Failed
            }
            (_, CoreLifecycle::Starting | CoreLifecycle::Stopping, _) => ProxyModeStatus::Pending,
            (_, _, None) => ProxyModeStatus::Unobserved,
            (_, CoreLifecycle::Ready | CoreLifecycle::Running, Some(_)) => ProxyModeStatus::Ready,
            (_, CoreLifecycle::Stopped, _) => ProxyModeStatus::Unobserved,
        };
        ProxyModeSnapshot {
            current,
            script_available: observed.script_available,
            status,
            failure,
        }
    }

    pub fn intent(
        current: &ProxyModeSnapshot,
        target: ProxyMode,
    ) -> Result<CommandIntent, Failure> {
        if !current.is_mode_selectable(target) {
            return Err(current.failure.clone().unwrap_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "The requested proxy mode is unavailable, unobserved or pending",
                    true,
                )
            }));
        }
        if current.current == Some(target) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "The requested proxy mode is already active",
                false,
            ));
        }
        Ok(CommandIntent::SetProxyMode { mode: target })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_mode_intent_rejects_same_or_unsupported() {
        let snapshot = ProxyModeSnapshot {
            current: Some(ProxyMode::Rule),
            script_available: Some(false),
            status: ProxyModeStatus::Ready,
            failure: None,
        };

        // Same mode rejected
        assert!(ProxyModeApplication::intent(&snapshot, ProxyMode::Rule).is_err());

        // Script rejected when unavailable
        assert!(ProxyModeApplication::intent(&snapshot, ProxyMode::Script).is_err());

        // Global accepted
        assert_eq!(
            ProxyModeApplication::intent(&snapshot, ProxyMode::Global),
            Ok(CommandIntent::SetProxyMode {
                mode: ProxyMode::Global
            })
        );
    }
}
