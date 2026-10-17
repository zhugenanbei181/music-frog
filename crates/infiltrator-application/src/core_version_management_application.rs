//! Shared, engine-keyed version management with owner-tracked install jobs.

use crate::version_application::VersionApplication;
use futures_util::FutureExt;
use infiltrator_contract::version::{
    CoreVersionInstallCancelOutcome, CoreVersionInstallContext, CoreVersionInstallId,
    CoreVersionInstallObservation, CoreVersionInstallReceipt, CoreVersionInstallState,
    CoreVersionManagementAction, CoreVersionManagementRequest, CoreVersionManagementResult,
    VersionDownloadProgress,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationTaskAdmission,
};
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::version::VersionProgressSink;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use std::collections::{HashMap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex, MutexGuard};

const MAX_RECORDED_INSTALLS: usize = 32;
const MAX_RELEASES_PER_REQUEST: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstallIdSequence {
    Next(u64),
    Exhausted,
}

struct InstallTaskOwnerState {
    next_id: InstallIdSequence,
    tasks: HashMap<CoreVersionInstallId, CoreVersionInstallState>,
    task_order: VecDeque<CoreVersionInstallId>,
    active_by_engine: HashMap<EngineId, CoreVersionInstallId>,
}

impl Default for InstallTaskOwnerState {
    fn default() -> Self {
        Self {
            next_id: InstallIdSequence::Next(1),
            tasks: HashMap::new(),
            task_order: VecDeque::new(),
            active_by_engine: HashMap::new(),
        }
    }
}

impl InstallTaskOwnerState {
    fn allocate_id(&mut self) -> Result<CoreVersionInstallId, Failure> {
        let InstallIdSequence::Next(next) = self.next_id else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Core version install identifiers are exhausted",
                false,
            ));
        };
        let id = CoreVersionInstallId::new(next).map_err(|_| {
            Failure::new(
                ErrorCode::Internal,
                "Core version install identifier sequence contains zero",
                false,
            )
        })?;
        self.next_id = match next.checked_add(1) {
            Some(next) => InstallIdSequence::Next(next),
            None => InstallIdSequence::Exhausted,
        };
        Ok(id)
    }

    fn prune_terminal_history(&mut self) {
        while self.tasks.len() >= MAX_RECORDED_INSTALLS {
            let Some(position) = self.task_order.iter().position(|install_id| {
                self.tasks
                    .get(install_id)
                    .is_some_and(|state| !state.is_active())
            }) else {
                return;
            };
            let Some(install_id) = self.task_order.remove(position) else {
                return;
            };
            self.tasks.remove(&install_id);
        }
    }
}

/// One shared version-management owner for every engine registered by the host.
/// Long downloads return a receipt immediately and update a typed task state.
#[derive(Clone)]
pub struct CoreVersionManagementApplication {
    factory: Arc<dyn EngineVersionBackendFactory>,
    runtime: Arc<dyn ApplicationRuntime>,
    installs: Arc<Mutex<InstallTaskOwnerState>>,
}

impl CoreVersionManagementApplication {
    pub fn new(
        factory: Arc<dyn EngineVersionBackendFactory>,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self {
            factory,
            runtime,
            installs: Arc::new(Mutex::new(InstallTaskOwnerState::default())),
        }
    }

    pub async fn execute(
        &self,
        request: CoreVersionManagementRequest,
    ) -> Result<CoreVersionManagementResult, Failure> {
        let engine_id = request.engine_id;
        match request.action {
            CoreVersionManagementAction::ListInstalled => {
                let versions = self.versions(&engine_id)?.list_installed().await?;
                Ok(CoreVersionManagementResult::InstalledVersions {
                    engine_id,
                    versions,
                })
            }
            CoreVersionManagementAction::ListReleases { limit } => {
                if limit == 0 || limit > MAX_RELEASES_PER_REQUEST {
                    return Err(Failure::new(
                        ErrorCode::InvalidInput,
                        format!(
                            "Core version release limit must be between 1 and {MAX_RELEASES_PER_REQUEST}"
                        ),
                        false,
                    ));
                }
                let releases = self
                    .versions(&engine_id)?
                    .list_releases(limit as usize)
                    .await?;
                Ok(CoreVersionManagementResult::Releases {
                    engine_id,
                    releases,
                })
            }
            CoreVersionManagementAction::Latest { channel } => {
                let release = self.versions(&engine_id)?.latest_core(channel).await?;
                Ok(CoreVersionManagementResult::Latest { engine_id, release })
            }
            CoreVersionManagementAction::Install { version } => {
                let receipt = self.start_install(engine_id, version)?;
                let state = match self.read_install(receipt.install_id) {
                    CoreVersionInstallObservation::Found { state } => state,
                    CoreVersionInstallObservation::NotFound { .. } => {
                        return Err(Failure::new(
                            ErrorCode::Internal,
                            "accepted Core version install disappeared before its receipt was read",
                            true,
                        ));
                    }
                    CoreVersionInstallObservation::Unavailable { failure } => {
                        return Err(failure);
                    }
                };
                Ok(CoreVersionManagementResult::InstallAccepted { receipt, state })
            }
            CoreVersionManagementAction::Activate { version } => {
                self.versions(&engine_id)?.activate(&version).await?;
                Ok(CoreVersionManagementResult::Activated { engine_id, version })
            }
            CoreVersionManagementAction::Uninstall { version } => {
                self.versions(&engine_id)?.uninstall(&version).await?;
                Ok(CoreVersionManagementResult::Uninstalled { engine_id, version })
            }
            CoreVersionManagementAction::Rollback => {
                let selected_version = self.versions(&engine_id)?.rollback().await?;
                Ok(CoreVersionManagementResult::RolledBack {
                    engine_id,
                    selected_version,
                })
            }
        }
    }

    /// Resolve an engine-bound view from the same factory owner used by
    /// asynchronous version-management actions.
    pub fn version_application(&self, engine_id: &EngineId) -> Result<VersionApplication, Failure> {
        self.versions(engine_id)
    }

    pub fn read_install(&self, install_id: CoreVersionInstallId) -> CoreVersionInstallObservation {
        let Ok(state) = self.installs.lock() else {
            return CoreVersionInstallObservation::Unavailable {
                failure: install_owner_unavailable(),
            };
        };
        match state.tasks.get(&install_id) {
            Some(state) => CoreVersionInstallObservation::Found {
                state: state.clone(),
            },
            None => CoreVersionInstallObservation::NotFound { install_id },
        }
    }

    pub fn cancel_install(
        &self,
        install_id: CoreVersionInstallId,
    ) -> CoreVersionInstallCancelOutcome {
        let Ok(mut owner) = self.installs.lock() else {
            return CoreVersionInstallCancelOutcome::Unavailable {
                failure: install_owner_unavailable(),
            };
        };
        let Some(current) = owner.tasks.get(&install_id).cloned() else {
            return CoreVersionInstallCancelOutcome::NotFound { install_id };
        };
        let outcome = match current {
            CoreVersionInstallState::Running { context, progress } => {
                let context = next_revision(context);
                owner.tasks.insert(
                    install_id,
                    CoreVersionInstallState::CancelRequested { context, progress },
                );
                CoreVersionInstallCancelOutcome::Requested { install_id }
            }
            CoreVersionInstallState::CancelRequested { .. } => {
                CoreVersionInstallCancelOutcome::AlreadyRequested { install_id }
            }
            CoreVersionInstallState::Completed { .. }
            | CoreVersionInstallState::Cancelled { .. }
            | CoreVersionInstallState::Failed { .. } => {
                CoreVersionInstallCancelOutcome::AlreadyFinished { install_id }
            }
        };
        outcome
    }

    fn start_install(
        &self,
        engine_id: EngineId,
        version: String,
    ) -> Result<CoreVersionInstallReceipt, Failure> {
        let versions = self.versions(&engine_id)?;
        let install_id;
        {
            let mut owner = self.lock_installs()?;
            if owner.active_by_engine.contains_key(&engine_id) {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "a Core version installation is already active for engine {}",
                        engine_id.as_str()
                    ),
                    true,
                ));
            }
            owner.prune_terminal_history();
            install_id = owner.allocate_id()?;
            let context = CoreVersionInstallContext {
                install_id,
                engine_id: engine_id.clone(),
                version: version.clone(),
                revision: 0,
            };
            owner.tasks.insert(
                install_id,
                CoreVersionInstallState::Running {
                    context,
                    progress: None,
                },
            );
            owner.task_order.push_back(install_id);
            owner.active_by_engine.insert(engine_id, install_id);
        }

        let sink: Arc<dyn VersionProgressSink> = Arc::new(InstallProgressSink {
            owner: self.clone(),
            install_id,
        });
        let owner = self.clone();
        let task: ApplicationFuture = Box::pin(async move {
            let result = AssertUnwindSafe(versions.install(version, sink))
                .catch_unwind()
                .await
                .unwrap_or_else(|_| {
                    Err(Failure::new(
                        ErrorCode::Internal,
                        "Core version adapter panicked during installation",
                        false,
                    ))
                });
            owner.finish_install(install_id, result);
        });
        match self.runtime.spawn_background(task) {
            ApplicationTaskAdmission::Scheduled { .. } => {}
            ApplicationTaskAdmission::AtCapacity { .. } => {
                self.discard_install(install_id);
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "the application runtime already owns a background task",
                    true,
                ));
            }
            ApplicationTaskAdmission::Closing | ApplicationTaskAdmission::Closed => {
                self.discard_install(install_id);
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "the application runtime is closing",
                    true,
                ));
            }
            ApplicationTaskAdmission::WorkerFailed => {
                self.discard_install(install_id);
                return Err(Failure::new(
                    ErrorCode::Internal,
                    "the application runtime worker failed before admitting the install",
                    true,
                ));
            }
            ApplicationTaskAdmission::IdExhausted => {
                self.discard_install(install_id);
                return Err(Failure::new(
                    ErrorCode::Internal,
                    "the application runtime task identifiers are exhausted",
                    false,
                ));
            }
        }
        Ok(CoreVersionInstallReceipt { install_id })
    }

    fn discard_install(&self, install_id: CoreVersionInstallId) {
        let Ok(mut owner) = self.installs.lock() else {
            return;
        };
        let Some(state) = owner.tasks.remove(&install_id) else {
            return;
        };
        owner.task_order.retain(|current| *current != install_id);
        let engine_id = state.context().engine_id.clone();
        if owner.active_by_engine.get(&engine_id) == Some(&install_id) {
            owner.active_by_engine.remove(&engine_id);
        }
    }

    fn finish_install(&self, install_id: CoreVersionInstallId, result: Result<(), Failure>) {
        let Ok(mut owner) = self.installs.lock() else {
            return;
        };
        let Some(current) = owner.tasks.get(&install_id).cloned() else {
            return;
        };
        let context = match current {
            CoreVersionInstallState::Running { context, .. }
            | CoreVersionInstallState::CancelRequested { context, .. } => context,
            CoreVersionInstallState::Completed { .. }
            | CoreVersionInstallState::Cancelled { .. }
            | CoreVersionInstallState::Failed { .. } => return,
        };
        let context = next_revision(context);
        let terminal = match result {
            Ok(()) => CoreVersionInstallState::Completed { context },
            Err(failure) if failure.code == ErrorCode::Canceled => {
                CoreVersionInstallState::Cancelled { context }
            }
            Err(failure) => CoreVersionInstallState::Failed { context, failure },
        };
        if owner.active_by_engine.get(&terminal.context().engine_id) == Some(&install_id) {
            owner.active_by_engine.remove(&terminal.context().engine_id);
        }
        owner.tasks.insert(install_id, terminal);
    }

    fn record_progress(&self, install_id: CoreVersionInstallId, progress: VersionDownloadProgress) {
        let Ok(mut owner) = self.installs.lock() else {
            return;
        };
        let Some(current) = owner.tasks.get(&install_id).cloned() else {
            return;
        };
        let updated = match current {
            CoreVersionInstallState::Running { context, .. } => CoreVersionInstallState::Running {
                context: next_revision(context),
                progress: Some(progress),
            },
            CoreVersionInstallState::CancelRequested { context, .. } => {
                CoreVersionInstallState::CancelRequested {
                    context: next_revision(context),
                    progress: Some(progress),
                }
            }
            CoreVersionInstallState::Completed { .. }
            | CoreVersionInstallState::Cancelled { .. }
            | CoreVersionInstallState::Failed { .. } => return,
        };
        owner.tasks.insert(install_id, updated);
    }

    fn is_install_cancel_requested(&self, install_id: CoreVersionInstallId) -> bool {
        let Ok(owner) = self.installs.lock() else {
            return true;
        };
        matches!(
            owner.tasks.get(&install_id),
            Some(CoreVersionInstallState::CancelRequested { .. })
                | Some(CoreVersionInstallState::Cancelled { .. })
                | Some(CoreVersionInstallState::Failed { .. })
                | None
        )
    }

    fn versions(&self, engine_id: &EngineId) -> Result<VersionApplication, Failure> {
        VersionApplication::from_factory(engine_id.clone(), self.factory.as_ref())
    }

    fn lock_installs(&self) -> Result<MutexGuard<'_, InstallTaskOwnerState>, Failure> {
        self.installs.lock().map_err(|_| {
            Failure::new(
                ErrorCode::Internal,
                "Core version install owner state is unavailable",
                true,
            )
        })
    }
}

fn install_owner_unavailable() -> Failure {
    Failure::new(
        ErrorCode::Internal,
        "Core version install owner state is unavailable",
        true,
    )
}

struct InstallProgressSink {
    owner: CoreVersionManagementApplication,
    install_id: CoreVersionInstallId,
}

impl VersionProgressSink for InstallProgressSink {
    fn progress(&self, progress: VersionDownloadProgress) {
        self.owner.record_progress(self.install_id, progress);
    }

    fn is_cancelled(&self) -> bool {
        self.owner.is_install_cancel_requested(self.install_id)
    }
}

fn next_revision(mut context: CoreVersionInstallContext) -> CoreVersionInstallContext {
    context.revision = context.revision.saturating_add(1);
    context
}

#[cfg(test)]
#[path = "core_version_management_application_tests.rs"]
mod tests;
