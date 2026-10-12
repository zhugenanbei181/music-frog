//! Ownership boundary for an independently launched desktop UI product.
use crate::composition::{DesktopProductBackend, open_default_product_backend};
use crate::product_switch::{retry_default_product_engine, switch_default_product_engine};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_core::settings_io::app_config_manager;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::host_runtime::HostRuntime;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesktopProductSessionAction {
    StartCore,
    StopCore,
    RestartCore,
    SelectProfile(ActiveProfileReferenceTarget),
    RecomposeSelectedProfile,
    RetryOwnerSwitch,
}

#[derive(Debug)]
pub enum DesktopProductSessionActionFailure {
    Core(Failure),
    OwnerSwitch(DesktopProductSwitchFailure),
    SessionUnavailable(Failure),
}

impl Display for DesktopProductSessionActionFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(failure) | Self::SessionUnavailable(failure) => {
                write!(formatter, "{}", failure.message)
            }
            Self::OwnerSwitch(failure) => write!(formatter, "{failure}"),
        }
    }
}

impl Error for DesktopProductSessionActionFailure {}

#[derive(Clone)]
pub struct DesktopProductSessionBinding {
    pub application: Arc<CoreApplication>,
    pub host_runtime: Arc<dyn HostRuntime>,
    pub surface_pump: SurfacePump,
    pub lifecycle: CoreLifecycleSnapshot,
}

#[derive(Clone)]
pub struct DesktopProductSessionHandle {
    inner: Arc<Mutex<DesktopProductSession>>,
}

impl DesktopProductSessionHandle {
    pub fn binding(
        &self,
    ) -> Result<DesktopProductSessionBinding, DesktopProductSessionActionFailure> {
        let session = self.lock()?;
        session
            .binding()
            .map_err(DesktopProductSessionActionFailure::SessionUnavailable)
    }

    pub fn apply(
        &self,
        action: DesktopProductSessionAction,
    ) -> Result<DesktopProductSessionBinding, DesktopProductSessionActionFailure> {
        let mut session = self.lock()?;
        match session.apply(action) {
            Ok(()) => {}
            Err(DesktopProductSessionActionFailure::OwnerSwitch(
                DesktopProductSwitchFailure::ReservationPending(_)
                | DesktopProductSwitchFailure::OwnerTerminationUnconfirmed(_)
                | DesktopProductSwitchFailure::OldOwnerClosed(_),
            )) => {
                session.retry_owner_switch()?;
            }
            Err(failure) => return Err(failure),
        }
        session
            .binding()
            .map_err(DesktopProductSessionActionFailure::SessionUnavailable)
    }

    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, DesktopProductSession>, DesktopProductSessionActionFailure> {
        self.inner.lock().map_err(|_| {
            DesktopProductSessionActionFailure::SessionUnavailable(Failure::new(
                ErrorCode::Internal,
                "Desktop product session lock is poisoned",
                false,
            ))
        })
    }
}

pub struct DesktopProductSession {
    executor: Runtime,
    application: Arc<CoreApplication>,
    surface: SurfaceKind,
    backend_factory: Arc<dyn DesktopProductBackendFactory>,
    state: DesktopProductSessionState,
}

#[async_trait::async_trait]
pub(crate) trait DesktopProductBackendFactory: Send + Sync {
    async fn switch_owner(
        &self,
        previous_application: Arc<CoreApplication>,
        target: ActiveProfileReferenceTarget,
        surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure>;

    async fn retry_owner_switch(
        &self,
        target: ActiveProfileReferenceTarget,
        old_owner_terminal: &CoreLifecycleSnapshot,
        surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure>;
}

struct ProductionDesktopProductBackendFactory;

#[async_trait::async_trait]
impl DesktopProductBackendFactory for ProductionDesktopProductBackendFactory {
    async fn switch_owner(
        &self,
        previous_application: Arc<CoreApplication>,
        target: ActiveProfileReferenceTarget,
        surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
        switch_default_product_engine(previous_application, target, surface).await
    }

    async fn retry_owner_switch(
        &self,
        target: ActiveProfileReferenceTarget,
        old_owner_terminal: &CoreLifecycleSnapshot,
        surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
        retry_default_product_engine(target, old_owner_terminal, surface).await
    }
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
        target: ActiveProfileReferenceTarget,
        failure: DesktopProductSwitchFailure,
        resume: CoreRecompositionResume,
    },
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoreRecompositionResume {
    KeepPaused,
    StartAfterRecomposition,
}

fn core_recomposition_resume(lifecycle: CoreLifecycle) -> Result<CoreRecompositionResume, Failure> {
    match lifecycle {
        CoreLifecycle::Ready | CoreLifecycle::Running => {
            Ok(CoreRecompositionResume::StartAfterRecomposition)
        }
        CoreLifecycle::Stopped | CoreLifecycle::Failed => Ok(CoreRecompositionResume::KeepPaused),
        CoreLifecycle::Starting | CoreLifecycle::Stopping => Err(Failure::new(
            ErrorCode::NotReady,
            "Desktop Core owner is changing lifecycle state",
            true,
        )),
    }
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
            backend_factory: Arc::new(ProductionDesktopProductBackendFactory),
            state: DesktopProductSessionState::Open {
                host_runtime: backend.host_runtime,
                pump: backend.surface_pump,
            },
        })
    }

    pub fn into_shared(self) -> DesktopProductSessionHandle {
        DesktopProductSessionHandle {
            inner: Arc::new(Mutex::new(self)),
        }
    }

    fn binding(&self) -> Result<DesktopProductSessionBinding, Failure> {
        let (host_runtime, pump) = match &self.state {
            DesktopProductSessionState::Open { host_runtime, pump } => {
                (Arc::clone(host_runtime), pump.clone())
            }
            DesktopProductSessionState::Switching => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "Desktop product session is switching Core owners",
                    true,
                ));
            }
            DesktopProductSessionState::RecoveryRequired {
                target, failure, ..
            } => {
                let target = match target {
                    ActiveProfileReferenceTarget::Selected {
                        engine_id,
                        profile_id,
                    } => format!("{}/{}", engine_id.as_str(), profile_id.as_str()),
                    ActiveProfileReferenceTarget::Unselected => "unselected".to_owned(),
                };
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    format!(
                        "Desktop product session requires owner recovery for {target}: {failure}"
                    ),
                    true,
                ));
            }
            DesktopProductSessionState::Closed => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "Desktop product session is closed",
                    false,
                ));
            }
        };
        Ok(DesktopProductSessionBinding {
            application: Arc::clone(&self.application),
            host_runtime,
            surface_pump: pump,
            lifecycle: self.application.snapshot().lifecycle_snapshot(),
        })
    }

    fn apply(
        &mut self,
        action: DesktopProductSessionAction,
    ) -> Result<(), DesktopProductSessionActionFailure> {
        match action {
            DesktopProductSessionAction::StartCore => {
                self.binding()
                    .map_err(DesktopProductSessionActionFailure::SessionUnavailable)?;
                self.execute_core_command(CommandIntent::StartCore)
                    .map_err(DesktopProductSessionActionFailure::Core)
            }
            DesktopProductSessionAction::StopCore => {
                self.binding()
                    .map_err(DesktopProductSessionActionFailure::SessionUnavailable)?;
                self.execute_core_command(CommandIntent::StopCore)
                    .map_err(DesktopProductSessionActionFailure::Core)
            }
            DesktopProductSessionAction::RestartCore => {
                self.binding()
                    .map_err(DesktopProductSessionActionFailure::SessionUnavailable)?;
                self.execute_core_command(CommandIntent::RestartCore)
                    .map_err(DesktopProductSessionActionFailure::Core)
            }
            DesktopProductSessionAction::SelectProfile(target) => {
                self.apply_profile_selection(target)
            }
            DesktopProductSessionAction::RecomposeSelectedProfile => {
                self.recompose_selected_profile()
            }
            DesktopProductSessionAction::RetryOwnerSwitch => self.retry_owner_switch(),
        }
    }

    fn retry_owner_switch(&mut self) -> Result<(), DesktopProductSessionActionFailure> {
        let previous_state = mem::replace(&mut self.state, DesktopProductSessionState::Closed);
        let (host_runtime, pump, target, resume) = match previous_state {
            DesktopProductSessionState::RecoveryRequired {
                host_runtime,
                pump,
                target,
                resume,
                ..
            } => (host_runtime, pump, target, resume),
            state => {
                self.state = state;
                return Err(DesktopProductSessionActionFailure::SessionUnavailable(
                    Failure::new(
                        ErrorCode::InvalidState,
                        "Desktop product session has no owner switch to retry",
                        false,
                    ),
                ));
            }
        };

        let terminal = match self.executor.block_on(self.application.close()) {
            Ok(_) => self.application.snapshot().lifecycle_snapshot(),
            Err(failure) => {
                let switch_failure =
                    DesktopProductSwitchFailure::OwnerTerminationUnconfirmed(failure);
                self.state = DesktopProductSessionState::RecoveryRequired {
                    host_runtime,
                    pump,
                    target,
                    failure: switch_failure.clone(),
                    resume,
                };
                return Err(DesktopProductSessionActionFailure::OwnerSwitch(
                    switch_failure,
                ));
            }
        };

        let replacement = self
            .executor
            .block_on(self.backend_factory.retry_owner_switch(
                target.clone(),
                &terminal,
                self.surface,
            ));
        let backend = match replacement {
            Ok(backend) => backend,
            Err(failure) => {
                self.state = DesktopProductSessionState::RecoveryRequired {
                    host_runtime,
                    pump,
                    target,
                    failure: failure.clone(),
                    resume,
                };
                return Err(DesktopProductSessionActionFailure::OwnerSwitch(failure));
            }
        };

        let old_application = mem::replace(&mut self.application, backend.application);
        self.state = DesktopProductSessionState::Open {
            host_runtime: backend.host_runtime,
            pump: backend.surface_pump,
        };
        drop(old_application);
        drop(host_runtime);
        drop(pump);
        match resume {
            CoreRecompositionResume::KeepPaused => Ok(()),
            CoreRecompositionResume::StartAfterRecomposition => self
                .execute_core_command(CommandIntent::StartCore)
                .map_err(DesktopProductSessionActionFailure::Core),
        }
    }

    fn execute_core_command(&self, intent: CommandIntent) -> Result<(), Failure> {
        let before = self.application.snapshot();
        let scope = before.scope().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Desktop Core owner has no scope for a lifecycle command",
                true,
            )
        })?;
        let outcome = self
            .executor
            .block_on(self.application.execute_scoped(CoreCommandRequest {
                scope: scope.clone(),
                intent,
            }));
        let terminal_scope = outcome.scope.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "Desktop Core command returned no owner scope",
                true,
            )
        })?;
        if !terminal_scope.identity.same_instance(&scope.identity) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Desktop Core command changed the bound owner instance",
                true,
            ));
        }
        match outcome.result {
            CommandResult::Completed { .. } => Ok(()),
            result => result.into_unit(),
        }
    }

    fn recompose_selected_profile(&mut self) -> Result<(), DesktopProductSessionActionFailure> {
        let target = self
            .executor
            .block_on(async {
                let manager = app_config_manager()
                    .await
                    .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), true))?;
                match ActiveProfileReferenceStore::read(manager.engine_profile_store())
                    .await
                    .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), true))?
                {
                    ActiveProfileReferenceState::Selected(reference) => {
                        Ok(ActiveProfileReferenceTarget::selected(
                            reference.engine_id,
                            reference.profile_id,
                        ))
                    }
                    ActiveProfileReferenceState::Unselected { .. } => Err(Failure::new(
                        ErrorCode::NotReady,
                        "Desktop owner recomposition requires a selected active profile",
                        true,
                    )),
                }
            })
            .map_err(|failure| {
                DesktopProductSessionActionFailure::OwnerSwitch(
                    DesktopProductSwitchFailure::BeforeReservation(failure),
                )
            })?;
        self.replace_profile_owner(target)
    }

    fn apply_profile_selection(
        &mut self,
        target: ActiveProfileReferenceTarget,
    ) -> Result<(), DesktopProductSessionActionFailure> {
        self.binding()
            .map_err(DesktopProductSessionActionFailure::SessionUnavailable)?;
        let scope = self.application.snapshot().scope().ok_or_else(|| {
            DesktopProductSessionActionFailure::SessionUnavailable(Failure::new(
                ErrorCode::NotReady,
                "Desktop Core owner has no scope for profile selection",
                true,
            ))
        })?;
        match &target {
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } if engine_id == &scope.identity.engine => self
                .execute_core_command(CommandIntent::SwitchProfile {
                    profile_id: profile_id.as_str().to_owned(),
                })
                .map_err(DesktopProductSessionActionFailure::Core),
            ActiveProfileReferenceTarget::Selected { .. } => self.replace_profile_owner(target),
            ActiveProfileReferenceTarget::Unselected => {
                Err(DesktopProductSessionActionFailure::OwnerSwitch(
                    DesktopProductSwitchFailure::BeforeReservation(Failure::unsupported(
                        "Desktop Core owner cannot select an unselected profile target",
                    )),
                ))
            }
        }
    }

    fn replace_profile_owner(
        &mut self,
        target: ActiveProfileReferenceTarget,
    ) -> Result<(), DesktopProductSessionActionFailure> {
        let resume = core_recomposition_resume(self.application.snapshot().lifecycle)
            .map_err(DesktopProductSessionActionFailure::SessionUnavailable)?;
        match self.availability() {
            DesktopProductSessionAvailability::Open => {}
            DesktopProductSessionAvailability::Switching
            | DesktopProductSessionAvailability::RecoveryRequired
            | DesktopProductSessionAvailability::Closed => {
                return Err(DesktopProductSessionActionFailure::SessionUnavailable(
                    Failure::new(
                        ErrorCode::InvalidState,
                        "Desktop product session is unavailable for owner recomposition",
                        false,
                    ),
                ));
            }
        }
        self.switch_engine_profile_with_resume(target, resume)
            .map_err(DesktopProductSessionActionFailure::OwnerSwitch)?;
        match resume {
            CoreRecompositionResume::KeepPaused => Ok(()),
            CoreRecompositionResume::StartAfterRecomposition => self
                .execute_core_command(CommandIntent::StartCore)
                .map_err(DesktopProductSessionActionFailure::Core),
        }
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
        let resume = core_recomposition_resume(self.application.snapshot().lifecycle)
            .map_err(DesktopProductSwitchFailure::BeforeReservation)?;
        self.switch_engine_profile_with_resume(target, resume)
    }

    fn switch_engine_profile_with_resume(
        &mut self,
        target: ActiveProfileReferenceTarget,
        resume: CoreRecompositionResume,
    ) -> Result<(), DesktopProductSwitchFailure> {
        let (host_runtime, pump) = self.begin_engine_switch()?;
        let result = self.executor.block_on(self.backend_factory.switch_owner(
            Arc::clone(&self.application),
            target.clone(),
            self.surface,
        ));
        self.complete_engine_switch(target, resume, host_runtime, pump, result)
    }

    fn begin_engine_switch(
        &mut self,
    ) -> Result<(Arc<dyn HostRuntime>, SurfacePump), DesktopProductSwitchFailure> {
        let previous_state = mem::replace(&mut self.state, DesktopProductSessionState::Switching);
        match previous_state {
            DesktopProductSessionState::Open { host_runtime, pump } => Ok((host_runtime, pump)),
            state => {
                self.state = state;
                Err(DesktopProductSwitchFailure::BeforeReservation(
                    Failure::new(
                        ErrorCode::InvalidState,
                        "Desktop product session is not open for an engine switch",
                        false,
                    ),
                ))
            }
        }
    }

    fn complete_engine_switch(
        &mut self,
        target: ActiveProfileReferenceTarget,
        resume: CoreRecompositionResume,
        host_runtime: Arc<dyn HostRuntime>,
        pump: SurfacePump,
        result: Result<DesktopProductBackend, DesktopProductSwitchFailure>,
    ) -> Result<(), DesktopProductSwitchFailure> {
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
                self.state = DesktopProductSessionState::RecoveryRequired {
                    host_runtime,
                    pump,
                    target,
                    failure: failure.clone(),
                    resume,
                };
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
            | DesktopProductSessionState::RecoveryRequired {
                host_runtime, pump, ..
            } => {
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

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;

#[cfg(test)]
mod recomposition_tests {
    use super::{CoreRecompositionResume, core_recomposition_resume};
    use infiltrator_contract::snapshot::CoreLifecycle;

    #[test]
    fn profile_recomposition_preserves_the_observed_core_running_state() {
        assert_eq!(
            core_recomposition_resume(CoreLifecycle::Running),
            Ok(CoreRecompositionResume::StartAfterRecomposition),
        );
        assert_eq!(
            core_recomposition_resume(CoreLifecycle::Ready),
            Ok(CoreRecompositionResume::StartAfterRecomposition),
        );
        assert_eq!(
            core_recomposition_resume(CoreLifecycle::Stopped),
            Ok(CoreRecompositionResume::KeepPaused),
        );
        assert_eq!(
            core_recomposition_resume(CoreLifecycle::Failed),
            Ok(CoreRecompositionResume::KeepPaused),
        );
        assert!(core_recomposition_resume(CoreLifecycle::Starting).is_err());
        assert!(core_recomposition_resume(CoreLifecycle::Stopping).is_err());
    }
}
