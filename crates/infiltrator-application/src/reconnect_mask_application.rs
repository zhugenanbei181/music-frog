//! Application logic deriving the reconnect/reload graceful degradation mask snapshot.

use infiltrator_contract::reconnect_mask::{ReconnectMaskSnapshot, ReconnectMaskStatus};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot, CoreWatchdogState};

/// Application service projecting the core lifecycle state into an overlay mask snapshot.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReconnectMaskApplication;

impl ReconnectMaskApplication {
    pub fn project(&self, core: &CoreSnapshot) -> ReconnectMaskSnapshot {
        match &core.watchdog.state {
            CoreWatchdogState::Waiting { attempt, retry_in_ms } => {
                ReconnectMaskSnapshot::reconnecting(*attempt, *retry_in_ms, "看门狗重连中，保留上一帧快照")
            }
            CoreWatchdogState::Restarting { attempt } => {
                ReconnectMaskSnapshot::reconnecting(*attempt, 0, "核心重启中，平滑重连")
            }
            _ => {
                if matches!(core.lifecycle, CoreLifecycle::Starting | CoreLifecycle::Stopping) {
                    ReconnectMaskSnapshot::reloading("配置重载中，保持上一帧有效快照")
                } else if core.lifecycle == CoreLifecycle::Failed {
                    ReconnectMaskSnapshot {
                        status: ReconnectMaskStatus::Degraded,
                        reloading: false,
                        message: core.failure.as_ref().map(|f| f.message.clone()),
                        preserves_last_frame: true,
                        attempt: None,
                        retry_in_ms: None,
                    }
                } else {
                    ReconnectMaskSnapshot::normal()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_core(lifecycle: CoreLifecycle, watchdog: CoreWatchdogState) -> CoreSnapshot {
        CoreSnapshot {
            lifecycle,
            generation: 1,
            revision: 1,
            session_token: None,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: None,
            watchdog: infiltrator_contract::snapshot::CoreWatchdogSnapshot {
                state: watchdog,
                ..Default::default()
            },
        }
    }

    #[test]
    fn running_core_yields_normal_mask() {
        let core = sample_core(CoreLifecycle::Running, CoreWatchdogState::Idle);
        let mask = ReconnectMaskApplication.project(&core);
        assert!(!mask.is_active());
        assert_eq!(mask.status, ReconnectMaskStatus::Normal);
    }

    #[test]
    fn starting_core_yields_reloading_mask() {
        let core = sample_core(CoreLifecycle::Starting, CoreWatchdogState::Idle);
        let mask = ReconnectMaskApplication.project(&core);
        assert!(mask.is_active());
        assert!(mask.preserves_last_frame);
        assert_eq!(mask.status, ReconnectMaskStatus::Reloading);
    }

    #[test]
    fn watchdog_waiting_yields_reconnecting_mask() {
        let core = sample_core(CoreLifecycle::Failed, CoreWatchdogState::Waiting { attempt: 2, retry_in_ms: 1000 });
        let mask = ReconnectMaskApplication.project(&core);
        assert!(mask.is_active());
        assert_eq!(mask.status, ReconnectMaskStatus::Reconnecting);
        assert_eq!(mask.attempt, Some(2));
    }
}
