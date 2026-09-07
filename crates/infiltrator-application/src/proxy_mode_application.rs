use infiltrator_contract::command::{CommandIntent, ProxyMode};
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;

pub struct ProxyModeApplication;

impl ProxyModeApplication {
    pub fn from_surface(snapshot: &SurfaceSnapshot) -> ProxyModeSnapshot {
        let current = snapshot
            .pages
            .overview
            .data
            .as_ref()
            .and_then(|data| data.proxy_mode)
            .or(snapshot.core.proxy_mode)
            .unwrap_or(ProxyMode::Rule);

        let script_available = snapshot.pages.overview.data.as_ref().is_some();

        let status = match snapshot.core.lifecycle {
            infiltrator_contract::snapshot::CoreLifecycle::Running
            | infiltrator_contract::snapshot::CoreLifecycle::Ready => ProxyModeStatus::Ready,
            infiltrator_contract::snapshot::CoreLifecycle::Starting
            | infiltrator_contract::snapshot::CoreLifecycle::Stopping => ProxyModeStatus::Pending,
            infiltrator_contract::snapshot::CoreLifecycle::Stopped => ProxyModeStatus::Ready,
            infiltrator_contract::snapshot::CoreLifecycle::Failed => ProxyModeStatus::Failed,
        };

        ProxyModeSnapshot {
            current,
            script_available,
            status,
            failure: snapshot.failure.as_ref().map(|f| f.message.clone()),
        }
    }

    pub fn intent(current: &ProxyModeSnapshot, target: ProxyMode) -> Result<CommandIntent, String> {
        if !current.is_mode_selectable(target) {
            return Err(format!("proxy mode {:?} is not selectable in state {:?}", target, current.status));
        }
        if current.current == target {
            return Err(format!("proxy mode {:?} is already active", target));
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
            current: ProxyMode::Rule,
            script_available: false,
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
            Ok(CommandIntent::SetProxyMode { mode: ProxyMode::Global })
        );
    }
}
