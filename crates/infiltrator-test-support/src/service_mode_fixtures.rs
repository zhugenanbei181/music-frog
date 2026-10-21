//! Isolated service-mode port fixture.
//!
//! The fixture publishes the real privilege state a host would report and it
//! never fabricates a privileged operation: a host without the privilege
//! answers `prepare` with a typed permission failure and a host without
//! service-mode support answers `Unsupported`.

use async_trait::async_trait;
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_ports::service_mode::ServiceModePort;
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// An isolated service-mode port that answers with a real privilege state.
pub struct ServiceModeCapturePort {
    platform: ServiceModePlatform,
    state: Mutex<ServiceModeState>,
}

impl ServiceModeCapturePort {
    /// A capture for one platform and privilege state.
    pub fn new(platform: ServiceModePlatform, state: ServiceModeState) -> Self {
        Self {
            platform,
            state: Mutex::new(state),
        }
    }

    /// The privilege state the port currently publishes.
    pub fn current_state(&self) -> ServiceModeState {
        *self.state.lock().expect("service mode capture state")
    }
}

#[async_trait]
impl ServiceModePort for ServiceModeCapturePort {
    async fn snapshot(&self) -> Result<ServiceModeSnapshot, PortError> {
        Ok(ServiceModeSnapshot {
            platform: self.platform,
            state: self.current_state(),
        })
    }

    async fn prepare(&self) -> Result<ServiceModeSnapshot, PortError> {
        let mut state = self.state.lock().expect("service mode capture state");
        let current = *state;
        let next = match current {
            ServiceModeState::NotInstalled => ServiceModeState::InstalledStopped,
            ServiceModeState::InstalledStopped | ServiceModeState::Ready => current,
            ServiceModeState::MissingPrivilege => {
                return Err(PortError::PermissionDenied(
                    "service mode needs an elevated session".to_owned(),
                ));
            }
            ServiceModeState::Unsupported => {
                return Err(PortError::unsupported(
                    Capability::CoreLifecycle,
                    "this host has no service mode",
                ));
            }
            ServiceModeState::Unavailable => {
                return Err(PortError::Failed(
                    "service mode state is unavailable on this host".to_owned(),
                ));
            }
        };
        *state = next;
        Ok(ServiceModeSnapshot {
            platform: self.platform,
            state: next,
        })
    }
}
