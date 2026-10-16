//! Inbound command serialization and nested lifecycle transactions use separate gates.
use futures_util::future::Abortable;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::speedtest::SpeedtestRunId;
use infiltrator_contract::speedtest::SpeedtestTarget;
use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease;
use std::mem;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use super::CoreApplication;
use super::command_name::command_name;
use crate::command_application::handler::{CoreLifecycleChildPermit, ValidatedCoreCommandScope};
use crate::command_capability::{CommandCapabilityRoute, classify_command};
use crate::core_capability_application::CoreCapabilityGateFailure;
use crate::speedtest_application::{SpeedtestRunAdmission, SpeedtestTaskOutcome};
use futures_util::FutureExt;
use infiltrator_contract::command::{CommandIntent, CommandKind, CommandResult, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreEvent, CoreLifecycle};
use infiltrator_ports::application_runtime::{ApplicationTaskAdmission, ApplicationTaskShutdown};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};

struct CommandScope<'a>(&'a AtomicBool);
impl Drop for CommandScope<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct OwnerBindingReleaseGuard<'a> {
    state: &'a Mutex<super::ActiveProfileOwnerBindingState>,
    binding: Option<
        Arc<
            dyn ActiveProfileOwnerLease,
        >,
    >,
}

impl<'a> OwnerBindingReleaseGuard<'a> {
    fn new(
        state: &'a Mutex<super::ActiveProfileOwnerBindingState>,
        binding: Arc<
            dyn ActiveProfileOwnerLease,
        >,
    ) -> Self {
        Self {
            state,
            binding: Some(binding),
        }
    }

    fn binding(
        &self,
    ) -> &dyn ActiveProfileOwnerLease {
        self.binding
            .as_deref()
            .expect("owner-binding release guard retains its lease")
    }

    fn commit(mut self) {
        let binding = self
            .binding
            .take()
            .expect("owner-binding release guard retains its lease");
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *state = super::ActiveProfileOwnerBindingState::Released;
        drop(state);
        drop(binding);
    }
}

impl Drop for OwnerBindingReleaseGuard<'_> {
    fn drop(&mut self) {
        let Some(binding) = self.binding.take() else {
            return;
        };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // `Releasing` blocks installation while this future is suspended.
        // Cancellation or a release error restores the same lease so a later
        // close can retry with its terminal snapshot.
        *state = super::ActiveProfileOwnerBindingState::Bound(binding);
    }
}

impl CoreApplication {
    pub(super) async fn execute_scoped_with_id(
        &self,
        request_id: RequestId,
        request: CoreCommandRequest,
    ) -> CommandResult {
        let CoreCommandRequest { scope, intent } = request;
        let _commands = self.inner.commands.lock().await;
        let kind = intent.kind();
        let current_scope = self.snapshot().scope();
        if current_scope.as_ref() != Some(&scope) {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                "stale or unavailable core identity; command was not executed",
                false,
            );
            self.push_event(CoreEvent::CommandFailed {
                scope: current_scope,
                request_id,
                kind,
                failure: failure.clone(),
            });
            return CommandResult::Rejected {
                request_id,
                failure,
            };
        }
        if self.inner.closed.load(Ordering::Acquire) {
            return self.finish_command(request_id, kind, Err(closed_failure()));
        }
        self.inner.active_command.store(true, Ordering::Release);
        let _scope = CommandScope(&self.inner.active_command);
        if matches!(
            intent,
            CommandIntent::StartProxy
                | CommandIntent::StopProxy
                | CommandIntent::ResumeKernel
                | CommandIntent::PauseKernel
                | CommandIntent::RestartKernel
        ) {
            if let Err(failure) = self.authorize_lifecycle_admission(&intent) {
                return self.finish_command(request_id, kind, Err(failure));
            }
            match &intent {
                CommandIntent::StartProxy | CommandIntent::ResumeKernel => {
                    if let Err(failure) = self.adopt_if_running().await {
                        return self.finish_command(request_id, kind, Err(failure));
                    }
                }
                CommandIntent::PauseKernel | CommandIntent::RestartKernel => {
                    let _ = self.adopt_if_running().await;
                }
                _ => {}
            }
            return self
                .execute_product_control_with_id(request_id, intent)
                .await;
        }
        if matches!(
            intent,
            CommandIntent::StartCore | CommandIntent::StopCore | CommandIntent::RestartCore
        ) {
            return self.execute_lifecycle_with_id(request_id, intent).await;
        }
        if let CommandIntent::StartSpeedtest { target } = &intent {
            return self
                .start_speedtest_with_id(request_id, scope, target.clone())
                .await;
        }
        if let CommandIntent::CancelSpeedtestRun { run_id } = &intent {
            return self.cancel_speedtest_run_with_id(request_id, kind, *run_id);
        }
        if let CommandIntent::SetSpeedtestConcurrency { concurrency } = &intent {
            let speedtest = self.speedtest_application();
            return match speedtest {
                Ok(speedtest) => {
                    speedtest.set_concurrency(*concurrency);
                    self.finish_command(request_id, kind, Ok(()))
                }
                Err(failure) => self.finish_command(request_id, kind, Err(failure)),
            };
        }
        self.push_event(CoreEvent::CommandAccepted {
            scope: self.snapshot().scope(),
            request_id,
            kind,
        });
        let handler = self
            .inner
            .command_handler
            .read()
            .expect("command handler lock")
            .clone();
        let outcome = match handler {
            Some(handler) => {
                let expected = intent.clone();
                handler
                    .handle_output_scoped(
                        ValidatedCoreCommandScope::from_core_application(self.snapshot().scope()),
                        intent,
                    )
                    .await
                    .and_then(|output| {
                        output.validate_for(&expected)?;
                        Ok(output)
                    })
            }
            None => Err(Failure::unsupported(format!(
                "command `{}` has no application command handler",
                command_name(&intent)
            ))),
        };
        match outcome {
            Ok(CommandOutput::Unit) => self.finish_command(request_id, kind, Ok(())),
            Ok(output) => {
                self.push_event(CoreEvent::CommandCompleted {
                    scope: self.snapshot().scope(),
                    request_id,
                    kind,
                });
                CommandResult::Produced {
                    request_id,
                    output: Box::new(output),
                }
            }
            Err(failure) => self.finish_command(request_id, kind, Err(failure)),
        }
    }

    async fn start_speedtest_with_id(
        &self,
        request_id: RequestId,
        scope: CoreScope,
        target: SpeedtestTarget,
    ) -> CommandResult {
        let kind = CommandIntent::StartSpeedtest {
            target: target.clone(),
        }
        .kind();
        if target.core_scope() != &scope {
            return self.finish_command(
                request_id,
                kind,
                Err(Failure::new(
                    ErrorCode::InvalidState,
                    "speedtest target belongs to another Core activation",
                    false,
                )),
            );
        }
        let requirement = match classify_command(&CommandIntent::StartSpeedtest {
            target: target.clone(),
        }) {
            CommandCapabilityRoute::CoreBackend(requirement) => requirement,
            _ => {
                return self.finish_command(
                    request_id,
                    kind,
                    Err(Failure::new(
                        ErrorCode::InvalidState,
                        "speedtest command lost its Core capability classification",
                        false,
                    )),
                );
            }
        };
        if let Err(failure) = self
            .inner
            .core_capabilities
            .read()
            .expect("core capability lock")
            .authorize(Some(&scope), requirement)
            .map_err(CoreCapabilityGateFailure::into_failure)
        {
            return self.finish_command(request_id, kind, Err(failure));
        }
        let speedtest = match self.speedtest_application() {
            Ok(speedtest) => speedtest,
            Err(failure) => return self.finish_command(request_id, kind, Err(failure)),
        };
        let prepared = match speedtest.prepare(target).await {
            Ok(prepared) => prepared,
            Err(failure) => return self.finish_command(request_id, kind, Err(failure)),
        };
        let admission = match speedtest.admit(prepared) {
            Ok(admission) => admission,
            Err(failure) => return self.finish_command(request_id, kind, Err(failure)),
        };
        let SpeedtestRunAdmission {
            receipt,
            registration,
            prepared,
            concurrency,
        } = admission;
        let run_id = receipt.run_id;
        let task_speedtest = speedtest.clone();
        let task_sender = self.inner.dispatch_tx.clone();
        let task_scope = scope.clone();
        let task = async move {
            let outcome = match AssertUnwindSafe(Abortable::new(
                task_speedtest.execute_prepared(run_id, prepared, concurrency),
                registration,
            ))
            .catch_unwind()
            .await
            {
                Ok(Ok(())) => SpeedtestTaskOutcome::Completed,
                Ok(Err(_)) => SpeedtestTaskOutcome::Cancelled,
                Err(_) => SpeedtestTaskOutcome::Failed(Failure::new(
                    ErrorCode::Internal,
                    "speedtest probe task panicked",
                    true,
                )),
            };
            if task_sender
                .send(super::DispatchedMessage::SpeedtestCompleted {
                    scope: task_scope,
                    run_id,
                    outcome,
                })
                .is_err()
            {
                log::warn!(
                    "speedtest completion for run {} was dropped because the Core actor channel closed",
                    run_id.get()
                );
            }
        };
        match self.inner.runtime.spawn_background(Box::pin(task)) {
            ApplicationTaskAdmission::Scheduled { .. } => {
                self.push_event(CoreEvent::CommandAccepted {
                    scope: Some(scope),
                    request_id,
                    kind,
                });
                self.finish_output_command(
                    request_id,
                    kind,
                    CommandOutput::SpeedtestStarted(receipt),
                )
            }
            admission => {
                let failure = speedtest_scheduler_failure(admission);
                speedtest.fail_admission(run_id, scope, failure.clone());
                self.finish_command(request_id, kind, Err(failure))
            }
        }
    }

    fn finish_output_command(
        &self,
        request_id: RequestId,
        kind: CommandKind,
        output: CommandOutput,
    ) -> CommandResult {
        self.push_event(CoreEvent::CommandCompleted {
            scope: self.snapshot().scope(),
            request_id,
            kind,
        });
        CommandResult::Produced {
            request_id,
            output: Box::new(output),
        }
    }

    fn cancel_speedtest_run_with_id(
        &self,
        request_id: RequestId,
        kind: CommandKind,
        run_id: SpeedtestRunId,
    ) -> CommandResult {
        let speedtest = match self.speedtest_application() {
            Ok(speedtest) => speedtest,
            Err(failure) => return self.finish_command(request_id, kind, Err(failure)),
        };
        let outcome = speedtest.cancel_run(run_id);
        let output = CommandOutput::SpeedtestCancelled(outcome);
        let intent = CommandIntent::CancelSpeedtestRun { run_id };
        if let Err(failure) = output.validate_for(&intent) {
            return self.finish_command(request_id, kind, Err(failure));
        }
        self.push_event(CoreEvent::CommandAccepted {
            scope: self.snapshot().scope(),
            request_id,
            kind,
        });
        self.finish_output_command(request_id, kind, output)
    }

    async fn execute_product_control_with_id(
        &self,
        request_id: RequestId,
        intent: CommandIntent,
    ) -> CommandResult {
        let kind = intent.kind();
        self.push_event(CoreEvent::CommandAccepted {
            scope: self.snapshot().scope(),
            request_id,
            kind,
        });
        let _operation = self.inner.operation.lock().await;
        let initial_lifecycle = self.snapshot().lifecycle;
        let outcome = async {
            match intent {
                CommandIntent::StartProxy => {
                    match initial_lifecycle {
                        CoreLifecycle::Ready | CoreLifecycle::Running => {
                            self.execute_handler_command(CommandIntent::CheckVpnAuthorization)
                                .await?;
                            self.authorize_lifecycle_command(&CommandIntent::StartProxy)?;
                        }
                        _ => {
                            self.execute_handler_command(CommandIntent::CheckVpnAuthorization)
                                .await?;
                            self.start_locked().await?;
                            self.authorize_lifecycle_command(&CommandIntent::StartProxy)?;
                        }
                    }
                    self.execute_handler_command(CommandIntent::StartVpn)
                        .await?;
                    Ok(())
                }
                CommandIntent::StopProxy => {
                    self.execute_handler_command(CommandIntent::StopVpn).await
                }
                CommandIntent::ResumeKernel => {
                    if matches!(
                        initial_lifecycle,
                        CoreLifecycle::Ready | CoreLifecycle::Running
                    ) {
                        Ok(())
                    } else {
                        self.start_locked().await
                    }
                }
                CommandIntent::PauseKernel => {
                    self.execute_handler_command(CommandIntent::StopVpn).await?;
                    self.stop_locked().await
                }
                CommandIntent::RestartKernel => {
                    self.execute_handler_command(CommandIntent::StopVpn).await?;
                    self.restart_locked().await
                }
                _ => Err(Failure::unsupported(
                    "non-product intent entered Android control execution",
                )),
            }
        }
        .await;
        self.finish_command(request_id, kind, outcome)
    }

    async fn execute_handler_command(&self, intent: CommandIntent) -> Result<(), Failure> {
        let handler = self
            .inner
            .command_handler
            .read()
            .expect("command handler lock")
            .clone()
            .ok_or_else(|| {
                Failure::unsupported(format!(
                    "command `{}` has no application command handler",
                    command_name(&intent)
                ))
            })?;
        let output = handler
            .handle_lifecycle_child_output(
                CoreLifecycleChildPermit::from_core_application(),
                ValidatedCoreCommandScope::from_core_application(self.snapshot().scope()),
                intent.clone(),
            )
            .await?;
        output.validate_for(&intent)?;
        match output {
            CommandOutput::Unit => Ok(()),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Android control child command returned an unexpected output",
                true,
            )),
        }
    }

    /// Internal lifecycle port calls can run inside a command's managed apply transaction.
    pub(super) async fn execute_lifecycle_with_id(
        &self,
        request_id: RequestId,
        intent: CommandIntent,
    ) -> CommandResult {
        let kind = intent.kind();
        if let Err(failure) = self.authorize_lifecycle_command(&intent) {
            return self.finish_command(request_id, kind, Err(failure));
        }
        self.push_event(CoreEvent::CommandAccepted {
            scope: self.snapshot().scope(),
            request_id,
            kind,
        });
        let _operation = self.inner.operation.lock().await;
        if self.inner.closed.load(Ordering::Acquire)
            && !self.inner.active_command.load(Ordering::Acquire)
        {
            return self.finish_command(request_id, kind, Err(closed_failure()));
        }
        let outcome = match intent {
            CommandIntent::StartCore => self.start_locked().await,
            CommandIntent::StopCore => self.stop_locked().await,
            CommandIntent::RestartCore => self.restart_locked().await,
            _ => Err(Failure::unsupported(
                "non-lifecycle intent cannot enter the lifecycle port",
            )),
        };
        self.finish_command(request_id, kind, outcome)
    }

    fn finish_command(
        &self,
        request_id: RequestId,
        kind: CommandKind,
        outcome: Result<(), Failure>,
    ) -> CommandResult {
        match outcome {
            Ok(()) => {
                self.push_event(CoreEvent::CommandCompleted {
                    scope: self.snapshot().scope(),
                    request_id,
                    kind,
                });
                CommandResult::Completed { request_id }
            }
            Err(failure) => {
                self.push_event(CoreEvent::CommandFailed {
                    scope: self.snapshot().scope(),
                    request_id,
                    kind,
                    failure: failure.clone(),
                });
                CommandResult::Rejected {
                    request_id,
                    failure,
                }
            }
        }
    }

    /// Close admission, finish the current transaction, release its facade and stop the owned core.
    pub async fn close(&self) -> Result<(), Failure> {
        self.inner.closed.store(true, Ordering::Release);
        self.inner.log_pump.lock().expect("log driver").take();
        let _commands = self.inner.commands.lock().await;
        if let Some(speedtest) = self
            .inner
            .speedtest
            .read()
            .expect("speedtest application lock")
            .as_ref()
        {
            speedtest.cancel_for_shutdown();
        }
        self.inner
            .command_handler
            .write()
            .expect("command handler lock")
            .take();
        let _operation = self.inner.operation.lock().await;
        let stop_result = self.stop_locked().await;
        let scheduler_result = self.inner.runtime.shutdown_background_tasks();
        if matches!(scheduler_result, ApplicationTaskShutdown::WorkerFailed) {
            return Err(Failure::new(
                ErrorCode::Internal,
                "background task scheduler failed to close",
                true,
            ));
        }
        stop_result?;
        let terminal = self.snapshot().lifecycle_snapshot();
        if terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Core owner close did not read back Stopped without a session token",
                true,
            ));
        }
        let binding = {
            let mut owner_binding = self
                .inner
                .active_profile_owner_binding
                .lock()
                .expect("active profile owner binding lock");
            if let super::ActiveProfileOwnerBindingState::Bound(binding) = &*owner_binding {
                if !terminal
                    .identity
                    .as_ref()
                    .is_some_and(|identity| binding.identity().same_instance(identity))
                    || terminal.generation < binding.generation().value()
                {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "Core owner terminal readback did not match its bound identity and generation",
                        true,
                    ));
                }
            }
            match mem::replace(
                &mut *owner_binding,
                super::ActiveProfileOwnerBindingState::Releasing,
            ) {
                super::ActiveProfileOwnerBindingState::Bound(binding) => Some(binding),
                super::ActiveProfileOwnerBindingState::Unbound
                | super::ActiveProfileOwnerBindingState::Released => {
                    *owner_binding = super::ActiveProfileOwnerBindingState::Released;
                    None
                }
                super::ActiveProfileOwnerBindingState::Releasing => {
                    *owner_binding = super::ActiveProfileOwnerBindingState::Releasing;
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "Core owner profile binding release is already in progress",
                        true,
                    ));
                }
            }
        };
        if let Some(binding) = binding {
            let release_guard =
                OwnerBindingReleaseGuard::new(&self.inner.active_profile_owner_binding, binding);
            if let Err(error) = release_guard
                .binding()
                .release_after_terminal(&terminal)
                .await
            {
                return Err(Failure::new(ErrorCode::NotReady, error.to_string(), true));
            }
            release_guard.commit();
        }
        Ok(())
    }
}

fn speedtest_scheduler_failure(admission: ApplicationTaskAdmission) -> Failure {
    match admission {
        ApplicationTaskAdmission::AtCapacity { occupying_task_id } => Failure::new(
            ErrorCode::NotReady,
            format!(
                "background task slot is occupied by task {}",
                occupying_task_id.get()
            ),
            true,
        ),
        ApplicationTaskAdmission::Closing => Failure::new(
            ErrorCode::Canceled,
            "background task scheduler is closing",
            false,
        ),
        ApplicationTaskAdmission::Closed => Failure::new(
            ErrorCode::Canceled,
            "background task scheduler is closed",
            false,
        ),
        ApplicationTaskAdmission::WorkerFailed => Failure::new(
            ErrorCode::Internal,
            "background task worker is unavailable",
            true,
        ),
        ApplicationTaskAdmission::IdExhausted => Failure::new(
            ErrorCode::Internal,
            "background task identity space is exhausted",
            false,
        ),
        ApplicationTaskAdmission::Scheduled { .. } => Failure::new(
            ErrorCode::InvalidState,
            "scheduled task was treated as rejected",
            false,
        ),
    }
}

pub(super) fn closed_failure() -> Failure {
    Failure::new(ErrorCode::Canceled, "application is closed", false)
}
