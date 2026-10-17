//! Async control operations: start, stop, switch and readiness wait.
//!
//! Exactly one control operation may be in flight. The fence is a real binary
//! fact owned by a guard, never a lifecycle phase: concurrent callers receive
//! [`RuntimeError::ControlBusy`] before any adapter or state mutation.

use super::process::{prepare_standalone, start_and_bind_owner, stop_process};
use super::state_stream::{StreamWait, next_before};
use super::switch_host::RuntimeSwitchHost;
use super::{
    LurkerRuntime, LurkerRuntimeState, RuntimeError, RuntimeFailurePhase, RuntimeInner, lock_active,
};
use crate::switch::coordinator::EngineSwitchCoordinator;
use crate::switch::error::EngineSwitchError;
use crate::switch::host::EngineSwitchHost;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchDisposition, EngineSwitchStage, EngineSwitchTarget,
};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreScope, CoreVersion, EngineId,
};
use lurker_core_ports::engine_backend::{EngineBackendPorts, EngineBackendProviderAvailability};
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl LurkerRuntime {
    /// Start the selected engine and publish `Starting`/`Running`.
    ///
    /// The journal owner is bound before the process is spawned, so a process
    /// can never run without an exact durable owner fence. A failure after that
    /// point leaves the durable binding for recovery instead of pretending the
    /// owner was released.
    pub async fn start(&self) -> Result<(), RuntimeError> {
        let _control = self.inner.begin_control()?;
        let engine = match self.state() {
            LurkerRuntimeState::Idle {
                selected: Some(selected),
            } => selected,
            LurkerRuntimeState::Stopped {
                engine: Some(engine),
                ..
            } => engine,
            state => {
                return Err(RuntimeError::InvalidState {
                    state: Box::new(state),
                });
            }
        };
        self.inner.publish(LurkerRuntimeState::Starting {
            engine: engine.clone(),
        });
        match self.inner.start_engine(&engine).await {
            Ok(()) => Ok(()),
            Err((phase, failure)) => {
                self.inner.publish(LurkerRuntimeState::Failed {
                    engine: Some(engine),
                    phase,
                    failure: failure.clone(),
                });
                Err(RuntimeError::Port(failure))
            }
        }
    }

    /// Stop the active Core and publish `Stopping`/`Stopped`.
    ///
    /// Success requires the exact `Stopped`/no-PID terminal. The in-process
    /// owner handle is released only after that evidence.
    pub async fn stop(&self) -> Result<(), RuntimeError> {
        let _control = self.inner.begin_control()?;
        let (backend, active_scope) = match lock_active(&self.inner.active) {
            Ok(active) => (active.backend.clone(), active.scope.clone()),
            Err(cause) => {
                self.inner.publish(LurkerRuntimeState::Failed {
                    engine: None,
                    phase: RuntimeFailurePhase::StopProcess,
                    failure: cause.clone(),
                });
                return Err(RuntimeError::Port(cause));
            }
        };
        let Some(backend) = backend else {
            return match self.state() {
                LurkerRuntimeState::Idle { .. } | LurkerRuntimeState::Stopped { .. } => Ok(()),
                state => Err(RuntimeError::InvalidState {
                    state: Box::new(state),
                }),
            };
        };
        let engine = backend.engine_id().clone();
        let scope = match active_scope.or_else(|| {
            backend.process().core_identity().map(|identity| CoreScope {
                identity,
                generation: CoreGeneration::new(1),
                session_token: None,
            })
        }) {
            Some(scope) => scope,
            None => {
                let cause = PortError::Failed(
                    "active engine backend has no owner scope to stop".to_owned(),
                );
                self.inner.publish(LurkerRuntimeState::Failed {
                    engine: Some(engine),
                    phase: RuntimeFailurePhase::StopTerminal,
                    failure: cause.clone(),
                });
                return Err(RuntimeError::Port(cause));
            }
        };
        self.inner.publish(LurkerRuntimeState::Stopping {
            engine: engine.clone(),
            scope: scope.clone(),
        });
        if let Err(failure) = stop_process(&backend).await {
            self.inner.publish(LurkerRuntimeState::Failed {
                engine: Some(engine),
                phase: failure.phase,
                failure: failure.cause.clone(),
            });
            return Err(RuntimeError::Port(failure.cause));
        }
        let terminal = self.inner.terminal_snapshot(&scope);
        let lease = match lock_active(&self.inner.active) {
            Ok(mut current) => current.lease.take(),
            Err(cause) => {
                self.inner.publish(LurkerRuntimeState::Failed {
                    engine: Some(engine),
                    phase: RuntimeFailurePhase::OwnerRelease,
                    failure: cause.clone(),
                });
                return Err(RuntimeError::Port(cause));
            }
        };
        let Some(lease) = lease else {
            let cause =
                PortError::Failed("active engine backend has no owner lease to release".to_owned());
            self.inner.publish(LurkerRuntimeState::Failed {
                engine: Some(engine),
                phase: RuntimeFailurePhase::OwnerRelease,
                failure: cause.clone(),
            });
            return Err(RuntimeError::Port(cause));
        };
        if let Err(cause) = lease.release_after_terminal(&terminal).await {
            if let Ok(mut current) = lock_active(&self.inner.active) {
                current.lease = Some(lease);
            }
            self.inner.publish(LurkerRuntimeState::Failed {
                engine: Some(engine),
                phase: RuntimeFailurePhase::OwnerRelease,
                failure: cause.clone(),
            });
            return Err(RuntimeError::Port(cause));
        }
        drop(lease);
        if let Ok(mut current) = lock_active(&self.inner.active) {
            current.backend = None;
            current.scope = None;
            current.version = None;
            current.lease = None;
        }
        self.inner.publish(LurkerRuntimeState::Stopped {
            engine: Some(engine),
            terminal: Some(terminal),
        });
        Ok(())
    }

    /// Replace the active owner with `target` through the switch coordinator.
    ///
    /// An unregistered target is rejected before any journal or process call.
    /// Pre-reservation failures roll back to the still-live previous owner; any
    /// later failure publishes [`LurkerRuntimeState::RecoveryRequired`].
    pub async fn switch_engine(&self, target: EngineId) -> Result<(), RuntimeError> {
        let _control = self.inner.begin_control()?;
        let (from, previous_scope, previous_version) = match self.state() {
            LurkerRuntimeState::Running {
                engine,
                scope,
                version,
            } => (engine, scope, version),
            state => {
                return Err(RuntimeError::InvalidState {
                    state: Box::new(state),
                });
            }
        };
        match self.inner.registry.availability(&target) {
            EngineBackendProviderAvailability::Registered => {}
            EngineBackendProviderAvailability::Unsupported { reason } => {
                return Err(RuntimeError::UnregisteredEngine {
                    engine_id: target,
                    reason,
                });
            }
        }
        let journal_snapshot = self
            .inner
            .journal
            .snapshot()
            .await
            .map_err(RuntimeError::Port)?;
        if let EngineOwnerState::Bound {
            identity,
            generation,
        } = &journal_snapshot.owner
        {
            self.inner.publish(LurkerRuntimeState::Switching {
                from: from.clone(),
                to: target.clone(),
                stage: EngineSwitchStage::ClosingOld {
                    previous: from.clone(),
                    owner_identity: identity.clone(),
                    owner_generation: *generation,
                },
            });
        }
        let host: Arc<dyn EngineSwitchHost<EngineBackendPorts, EngineId>> =
            Arc::new(RuntimeSwitchHost {
                inner: Arc::clone(&self.inner),
                from: from.clone(),
                to: target.clone(),
            });
        let coordinator: EngineSwitchCoordinator<
            EngineBackendPorts,
            dyn EngineSwitchJournal<Selection = EngineId>,
        > = EngineSwitchCoordinator::new(
            Arc::clone(&self.inner.registry),
            Arc::clone(&self.inner.journal),
            host,
        );
        let requested = EngineSwitchTarget::Selected {
            engine_id: target.clone(),
            selection: target.clone(),
        };
        let reservation = match coordinator
            .reserve_switch(journal_snapshot.revision, requested)
            .await
        {
            Ok(reservation) => reservation,
            Err(error) => {
                return Err(self
                    .inner
                    .rollback_switch(error, &from, &previous_scope, &previous_version)
                    .await);
            }
        };
        let candidate_identity = reservation.candidate_identity().clone();
        let previous = reservation.previous_selection().clone();
        let selection = reservation.selection().clone();
        let stage = match &reservation.snapshot().owner {
            EngineOwnerState::Switching { stage } => stage.clone(),
            EngineOwnerState::Unowned | EngineOwnerState::Bound { .. } => {
                EngineSwitchStage::BindingNew {
                    selection: selection.clone(),
                    engine_id: selection.clone(),
                }
            }
        };
        self.inner.publish(LurkerRuntimeState::Switching {
            from: previous,
            to: selection.clone(),
            stage,
        });
        match coordinator.finalize_switch_with_lease(reservation).await {
            Ok((_snapshot, lease)) => {
                if let Ok(mut active) = lock_active(&self.inner.active) {
                    active.lease = Some(lease);
                }
                match self.inner.running_projection() {
                    Ok((scope, version)) => {
                        self.inner.publish(LurkerRuntimeState::Running {
                            engine: selection,
                            scope,
                            version,
                        });
                        Ok(())
                    }
                    Err(cause) => {
                        self.inner.publish(LurkerRuntimeState::Failed {
                            engine: Some(selection),
                            phase: RuntimeFailurePhase::ScopeProjection,
                            failure: cause.clone(),
                        });
                        Err(RuntimeError::Port(cause))
                    }
                }
            }
            Err(error) => Err(self
                .inner
                .fail_switch(error, &selection, &candidate_identity)
                .await),
        }
    }

    /// Wait until the runtime reaches `Running`, a terminal failure or the
    /// supplied bound. The state stream replay makes this safe to call after
    /// the transition already happened.
    pub async fn wait_ready(&self, timeout: Duration) -> Result<(), RuntimeError> {
        match self.state() {
            LurkerRuntimeState::Running { .. } => return Ok(()),
            LurkerRuntimeState::Failed { failure, .. }
            | LurkerRuntimeState::RecoveryRequired { failure, .. } => {
                return Err(RuntimeError::Port(failure));
            }
            state @ (LurkerRuntimeState::Idle { .. } | LurkerRuntimeState::Stopped { .. }) => {
                return Err(RuntimeError::NotReady {
                    state: Box::new(state),
                });
            }
            LurkerRuntimeState::Starting { .. }
            | LurkerRuntimeState::Switching { .. }
            | LurkerRuntimeState::Stopping { .. } => {}
        }
        let now = Instant::now();
        let deadline = now.checked_add(timeout).unwrap_or(now);
        let mut stream = self.state_stream();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(RuntimeError::ReadinessTimeout {
                    engine_id: self.inner.observed_engine(),
                    timeout,
                });
            }
            match next_before(&mut stream, remaining).await {
                StreamWait::State(state) => match *state {
                    LurkerRuntimeState::Running { .. } => return Ok(()),
                    LurkerRuntimeState::Failed { failure, .. }
                    | LurkerRuntimeState::RecoveryRequired { failure, .. } => {
                        return Err(RuntimeError::Port(failure));
                    }
                    state @ (LurkerRuntimeState::Idle { .. }
                    | LurkerRuntimeState::Stopped { .. }) => {
                        return Err(RuntimeError::NotReady {
                            state: Box::new(state),
                        });
                    }
                    LurkerRuntimeState::Starting { .. }
                    | LurkerRuntimeState::Switching { .. }
                    | LurkerRuntimeState::Stopping { .. } => {}
                },
                StreamWait::Timeout => {
                    return Err(RuntimeError::ReadinessTimeout {
                        engine_id: self.inner.observed_engine(),
                        timeout,
                    });
                }
                StreamWait::Closed => return Err(RuntimeError::StateStreamClosed),
            }
        }
    }
}

impl RuntimeInner {
    async fn start_engine(
        &self,
        engine: &EngineId,
    ) -> Result<(), (RuntimeFailurePhase, PortError)> {
        let backend = self
            .registry
            .backend(engine)
            .map_err(|cause| (RuntimeFailurePhase::BackendDispatch, cause))?;
        let (context, identity) = prepare_standalone(&backend, self.expected_instance.as_ref())
            .await
            .map_err(|failure| (failure.phase, failure.cause))?;
        let snapshot = self
            .journal
            .snapshot()
            .await
            .map_err(|cause| (RuntimeFailurePhase::JournalBindOwner, cause))?;
        let (_, lease) = self
            .journal
            .bind_owner(
                snapshot.revision,
                engine.clone(),
                identity.clone(),
                CoreGeneration::new(1),
            )
            .await
            .map_err(|cause| (RuntimeFailurePhase::JournalBindOwner, cause))?;
        let (scope, version) =
            start_and_bind_owner(&backend, context, identity, self.readiness_timeout)
                .await
                .map_err(|failure| (failure.phase, failure.cause))?;
        let mut active = match lock_active(&self.active) {
            Ok(active) => active,
            Err(cause) => {
                let _ = backend.process().stop().await;
                return Err((RuntimeFailurePhase::BindOwnerScope, cause));
            }
        };
        active.backend = Some(backend);
        active.scope = Some(scope.clone());
        active.version = version.clone();
        active.lease = Some(lease);
        drop(active);
        self.publish(LurkerRuntimeState::Running {
            engine: engine.clone(),
            scope,
            version,
        });
        Ok(())
    }

    async fn rollback_switch(
        &self,
        error: EngineSwitchError<EngineId>,
        from: &EngineId,
        scope: &CoreScope,
        version: &Option<CoreVersion>,
    ) -> RuntimeError {
        if let EngineSwitchDisposition::RolledBackToPrevious { selection } = &error.disposition {
            self.publish(LurkerRuntimeState::Running {
                engine: selection.clone(),
                scope: scope.clone(),
                version: version.clone(),
            });
        } else {
            let selection = match self.journal.snapshot().await {
                Ok(snapshot) => snapshot.selection,
                Err(_) => from.clone(),
            };
            self.publish(LurkerRuntimeState::RecoveryRequired {
                selection,
                phase: error.phase,
                failure: error.cause.clone(),
            });
        }
        RuntimeError::Switch(error)
    }

    async fn fail_switch(
        &self,
        error: EngineSwitchError<EngineId>,
        target: &EngineId,
        candidate: &CoreIdentity,
    ) -> RuntimeError {
        self.abort_candidate(target, candidate).await;
        let selection = match self.journal.snapshot().await {
            Ok(snapshot) => snapshot.selection,
            Err(_) => target.clone(),
        };
        self.publish(LurkerRuntimeState::RecoveryRequired {
            selection,
            phase: error.phase,
            failure: error.cause.clone(),
        });
        RuntimeError::Switch(error)
    }

    /// Stop a candidate that the coordinator applied but could not finalize.
    /// Only the exact reserved identity is stopped, so an unrelated process is
    /// never touched.
    async fn abort_candidate(&self, target: &EngineId, candidate: &CoreIdentity) {
        let backend = match lock_active(&self.active) {
            Ok(active) => active.backend.clone(),
            Err(_) => None,
        };
        if let Some(backend) = backend {
            let matches_candidate = backend.engine_id() == target
                && backend
                    .process()
                    .core_identity()
                    .is_some_and(|identity| identity.same_instance(candidate));
            if matches_candidate {
                let _ = backend.process().stop().await;
            }
        }
        if let Ok(mut active) = lock_active(&self.active) {
            active.backend = None;
            active.scope = None;
            active.version = None;
        }
    }
}
