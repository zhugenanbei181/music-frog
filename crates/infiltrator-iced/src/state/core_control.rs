//! Toolkit adaptation of lifecycle facts; the action decision belongs to the shared application.
use crate::state::AppState;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::core_control::CoreControlProjection;
use infiltrator_presentation::core_control_projection::project_core_control;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;

impl AppState {
    pub fn core_lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        if self.commands.is_some() {
            self.runtime.core_lifecycle.clone()
        } else {
            self.surface
                .latest()
                .map(|snapshot| snapshot.core.lifecycle_snapshot())
                .unwrap_or_else(|| self.runtime.core_lifecycle.clone())
        }
    }
    pub fn core_control_projection(&self) -> CoreControlProjection {
        let lifecycle = self.core_lifecycle_snapshot();
        let composed = match self.commands.as_ref() {
            Some(_) => Availability::Supported,
            None => Availability::Unavailable {
                reason: self
                    .runtime
                    .host_composition_failure
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_else(|| "Core owner is not attached to this surface".to_owned()),
            },
        };
        let capability_availability = self.commands.as_ref().and_then(|_| {
            self.surface.latest().and_then(|snapshot| {
                snapshot
                    .capabilities
                    .entries
                    .iter()
                    .find(|entry| entry.capability == Capability::CoreLifecycle)
                    .map(|entry| &entry.availability)
            })
        });
        let availability = capability_availability.unwrap_or(&composed);
        project_core_control(
            &lifecycle,
            Some(availability),
            self.runtime.lifecycle.is_running(),
            self.runtime.lifecycle.failure(),
        )
    }
}
