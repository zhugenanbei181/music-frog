//! Application seam for the selected outbound node read model.

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_ports::error::PortError;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default)]
pub struct ActiveExitApplication;

impl ActiveExitApplication {
    pub fn project(
        &self,
        core: &CoreSnapshot,
        proxies: Option<&Result<HashMap<String, Proxy>, PortError>>,
    ) -> ActiveExitSnapshot {
        let revision = core.revision.max(1);
        if !matches!(core.lifecycle, CoreLifecycle::Running | CoreLifecycle::Ready) {
            return ActiveExitSnapshot::unavailable(
                core.generation,
                revision,
                "core is not running; active exit is not sampled",
            );
        }
        let Some(proxies) = proxies else {
            return ActiveExitSnapshot::unsupported(
                core.generation,
                revision,
                "proxy gateway is not composed",
            );
        };
        let proxies = match proxies {
            Ok(proxies) => proxies,
            Err(error) => {
                return ActiveExitSnapshot::failed(
                    core.generation,
                    revision,
                    format!("proxy list active-exit input failed: {error}"),
                );
            }
        };
        infiltrator_domain::active_exit::derive(core.generation, revision, proxies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::active_exit::ActiveExitStatus;

    fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            lifecycle,
            generation: 4,
            revision: 5,
            session_token: None,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: None,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn missing_proxy_port_is_typed_unsupported() {
        let snapshot = ActiveExitApplication.project(&core(CoreLifecycle::Running), None);
        assert_eq!(snapshot.status, ActiveExitStatus::Unsupported);
    }

    #[test]
    fn stopped_core_does_not_claim_a_selected_exit() {
        let snapshot = ActiveExitApplication.project(&core(CoreLifecycle::Stopped), None);
        assert_eq!(snapshot.status, ActiveExitStatus::Unknown);
    }
}
