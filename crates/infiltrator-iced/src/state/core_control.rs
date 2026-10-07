//! Toolkit adaptation of lifecycle facts; the action decision belongs to the shared application.
use crate::state::AppState;
use crate::types::runtime::RuntimeStatus;
use infiltrator_application::core_control_projection::project_core_control;
use infiltrator_contract::capability::{Availability, Capability};
use infiltrator_contract::core_control::CoreControlProjection;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};

impl AppState {
    pub fn core_lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        if self.commands.is_some() {
            self.runtime.core_lifecycle.clone()
        } else {
            self.surface
                .latest()
                .map(|snapshot| snapshot.core.lifecycle_snapshot())
                .unwrap_or_else(|| CoreLifecycleSnapshot {
                    lifecycle: match self.runtime.status {
                        RuntimeStatus::Stopped => CoreLifecycle::Stopped,
                        RuntimeStatus::Starting => CoreLifecycle::Starting,
                        RuntimeStatus::Running => CoreLifecycle::Running,
                        RuntimeStatus::Error(_) => CoreLifecycle::Failed,
                    },
                    ..self.runtime.core_lifecycle.clone()
                })
        }
    }
    pub fn core_control_projection(&self) -> CoreControlProjection {
        let lifecycle = self.core_lifecycle_snapshot();
        // The legacy desktop boot adapter can compose a process host on start.
        // New host-composed sessions publish their exact availability instead.
        let legacy = self.runtime.host_composition_failure.as_ref().map_or(
            Availability::Supported,
            |failure| Availability::Unavailable {
                reason: failure.message.clone(),
            },
        );
        let availability = self
            .surface
            .latest()
            .map(|snapshot| {
                snapshot
                    .capabilities
                    .entries
                    .iter()
                    .find(|entry| entry.capability == Capability::CoreLifecycle)
                    .map(|entry| &entry.availability)
            })
            .unwrap_or(Some(&legacy));
        project_core_control(
            &lifecycle,
            availability,
            self.runtime.lifecycle_pending.is_some(),
            self.runtime.lifecycle_failure.as_ref(),
        )
    }
}
