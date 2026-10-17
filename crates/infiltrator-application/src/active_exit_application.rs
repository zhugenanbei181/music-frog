//! Application seam for the selected outbound node read model.

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::active_exit::derive;
use lurker_core_contract::error::Failure;
use lurker_core_contract::outbound::inventory::OutboundObservationSnapshot;

#[derive(Clone, Copy, Debug, Default)]
pub struct ActiveExitApplication;

impl ActiveExitApplication {
    pub fn project(
        &self,
        core: &CoreSnapshot,
        observation: Option<&Result<OutboundObservationSnapshot, Failure>>,
    ) -> ActiveExitSnapshot {
        let revision = core.revision.max(1);
        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return ActiveExitSnapshot::unavailable(
                core.generation,
                revision,
                "core is not running; active exit is not sampled",
            );
        }
        let Some(observation) = observation else {
            return ActiveExitSnapshot::unsupported(
                core.generation,
                revision,
                "outbound inventory source is not composed",
            );
        };
        let observation = match observation {
            Ok(observation) => observation,
            Err(failure) => {
                return ActiveExitSnapshot::failed(
                    core.generation,
                    revision,
                    format!(
                        "outbound inventory active-exit input failed: {}",
                        failure.message
                    ),
                );
            }
        };
        let Some(scope) = core.scope() else {
            return ActiveExitSnapshot::unavailable(
                core.generation,
                revision,
                "active core scope is unavailable",
            );
        };
        if observation.inventory().scope() != &scope {
            return ActiveExitSnapshot::unavailable(
                core.generation,
                revision,
                "outbound observation belongs to a different core scope",
            );
        }
        derive(
            core.generation,
            observation.inventory().revision().value(),
            observation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::active_exit::ActiveExitStatus;
    use infiltrator_contract::snapshot::CoreLifecycle;

    fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            identity: None,
            lifecycle,
            generation: 4,
            revision: 5,
            session_token: None,
            routing_intent: Default::default(),
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn missing_outbound_inventory_is_typed_unsupported() {
        let snapshot = ActiveExitApplication.project(&core(CoreLifecycle::Running), None);
        assert!(matches!(snapshot.status, ActiveExitStatus::Unsupported));
    }

    #[test]
    fn stopped_core_does_not_claim_a_selected_exit() {
        let snapshot = ActiveExitApplication.project(&core(CoreLifecycle::Stopped), None);
        assert!(matches!(snapshot.status, ActiveExitStatus::Unknown));
    }
}
