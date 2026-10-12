//! Ownership boundary for an independently launched desktop UI product.
use crate::composition::open_default_product_backend;
use crate::product_switch::switch_default_product_engine;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceTarget;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_ports::host_runtime::HostRuntime;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::mem;
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopProductSessionAvailability {
    Open,
    Switching,
    RecoveryRequired,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesktopProductSwitchFailure {
    BeforeReservation(Failure),
    ReservationPending(Failure),
    OwnerTerminationUnconfirmed(Failure),
    OldOwnerClosed(Failure),
}

impl Display for DesktopProductSwitchFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let failure = match self {
            Self::BeforeReservation(failure)
            | Self::ReservationPending(failure)
            | Self::OwnerTerminationUnconfirmed(failure)
            | Self::OldOwnerClosed(failure) => failure,
        };
        write!(formatter, "{}", failure.message)
    }
}

impl Error for DesktopProductSwitchFailure {}

pub struct DesktopProductSession {
    executor: Runtime,
    application: Arc<CoreApplication>,
    surface: SurfaceKind,
    state: DesktopProductSessionState,
}

enum DesktopProductSessionState {
    Open {
        host_runtime: Arc<dyn HostRuntime>,
        pump: SurfacePump,
    },
    Switching,
    RecoveryRequired {
        host_runtime: Arc<dyn HostRuntime>,
        pump: SurfacePump,
    },
    Closed,
}
impl DesktopProductSession {
    /// Native ports are prepared offline; starting a stopped kernel remains an explicit intent.
    pub fn open(surface: SurfaceKind) -> anyhow::Result<Self> {
        let executor = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let backend = executor.block_on(open_default_product_backend(surface))?;
        Ok(Self {
            executor,
            application: backend.application,
            surface,
            state: DesktopProductSessionState::Open {
                host_runtime: backend.host_runtime,
                pump: backend.surface_pump,
            },
        })
    }
    pub fn application(&self) -> Arc<CoreApplication> {
        match &self.state {
            DesktopProductSessionState::Open { .. } => Arc::clone(&self.application),
            DesktopProductSessionState::Switching => {
                panic!("desktop product session is switching Core owners")
            }
            DesktopProductSessionState::RecoveryRequired { .. } => {
                panic!("desktop product session requires owner recovery")
            }
            DesktopProductSessionState::Closed => panic!("desktop product session is closed"),
        }
    }
    pub fn surface_pump(&self) -> SurfacePump {
        match &self.state {
            DesktopProductSessionState::Open { pump, .. } => pump.clone(),
            DesktopProductSessionState::Switching => {
                panic!("desktop product session is switching Core owners")
            }
            DesktopProductSessionState::RecoveryRequired { .. } => {
                panic!("desktop product session requires owner recovery")
            }
            DesktopProductSessionState::Closed => panic!("desktop product session is closed"),
        }
    }
    pub fn host_runtime(&self) -> Arc<dyn HostRuntime> {
        match &self.state {
            DesktopProductSessionState::Open { host_runtime, .. } => host_runtime.clone(),
            DesktopProductSessionState::Switching => {
                panic!("desktop product session is switching Core owners")
            }
            DesktopProductSessionState::RecoveryRequired { .. } => {
                panic!("desktop product session requires owner recovery")
            }
            DesktopProductSessionState::Closed => panic!("desktop product session is closed"),
        }
    }

    pub fn availability(&self) -> DesktopProductSessionAvailability {
        match &self.state {
            DesktopProductSessionState::Open { .. } => DesktopProductSessionAvailability::Open,
            DesktopProductSessionState::Switching => DesktopProductSessionAvailability::Switching,
            DesktopProductSessionState::RecoveryRequired { .. } => {
                DesktopProductSessionAvailability::RecoveryRequired
            }
            DesktopProductSessionState::Closed => DesktopProductSessionAvailability::Closed,
        }
    }

    /// Replace the live engine owner through the revision-fenced Core switch
    /// transaction. Callers must reacquire application and surface handles
    /// after this method succeeds.
    pub fn switch_engine_profile(
        &mut self,
        target: ActiveProfileReferenceTarget,
    ) -> Result<(), DesktopProductSwitchFailure> {
        let previous_state = mem::replace(&mut self.state, DesktopProductSessionState::Switching);
        let DesktopProductSessionState::Open { host_runtime, pump } = previous_state else {
            self.state = previous_state;
            return Err(DesktopProductSwitchFailure::BeforeReservation(
                Failure::new(
                    ErrorCode::InvalidState,
                    "Desktop product session is not open for an engine switch",
                    false,
                ),
            ));
        };

        let result = self.executor.block_on(switch_default_product_engine(
            Arc::clone(&self.application),
            target,
            self.surface,
        ));
        match result {
            Ok(backend) => {
                let old_application = mem::replace(&mut self.application, backend.application);
                self.state = DesktopProductSessionState::Open {
                    host_runtime: backend.host_runtime,
                    pump: backend.surface_pump,
                };
                drop(old_application);
                drop(host_runtime);
                drop(pump);
                Ok(())
            }
            Err(failure @ DesktopProductSwitchFailure::BeforeReservation(_)) => {
                self.state = DesktopProductSessionState::Open { host_runtime, pump };
                Err(failure)
            }
            Err(
                failure @ (DesktopProductSwitchFailure::ReservationPending(_)
                | DesktopProductSwitchFailure::OwnerTerminationUnconfirmed(_)
                | DesktopProductSwitchFailure::OldOwnerClosed(_)),
            ) => {
                self.state = DesktopProductSessionState::RecoveryRequired { host_runtime, pump };
                Err(failure)
            }
        }
    }
}
impl Drop for DesktopProductSession {
    fn drop(&mut self) {
        let state = mem::replace(&mut self.state, DesktopProductSessionState::Closed);
        match state {
            DesktopProductSessionState::Open { host_runtime, pump }
            | DesktopProductSessionState::RecoveryRequired { host_runtime, pump } => {
                drop(pump);
                if let Err(error) = self.executor.block_on(self.application.close()) {
                    log::error!("desktop product shutdown failed: {}", error.message);
                }
                drop(host_runtime);
            }
            DesktopProductSessionState::Switching => {
                log::error!("desktop product session dropped during an owner switch");
                if let Err(error) = self.executor.block_on(self.application.close()) {
                    log::error!("desktop product shutdown failed: {}", error.message);
                }
            }
            DesktopProductSessionState::Closed => {}
        }
    }
}
