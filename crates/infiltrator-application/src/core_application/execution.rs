//! Inbound command serialization and nested lifecycle transactions use separate gates.
use super::CoreApplication;
use super::command_name::command_name;
use infiltrator_contract::command::{CommandIntent, CommandKind, CommandResult, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreEvent;
use std::sync::atomic::{AtomicBool, Ordering};

struct CommandScope<'a>(&'a AtomicBool);
impl Drop for CommandScope<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl CoreApplication {
    pub(super) async fn execute_with_id(
        &self,
        request_id: RequestId,
        intent: CommandIntent,
    ) -> CommandResult {
        let _commands = self.inner.commands.lock().await;
        let kind = intent.kind();
        if self.inner.closed.load(Ordering::Acquire) {
            return self.finish_command(request_id, kind, Err(closed_failure()));
        }
        self.inner.active_command.store(true, Ordering::Release);
        let _scope = CommandScope(&self.inner.active_command);
        if matches!(
            intent,
            CommandIntent::StartCore
                | CommandIntent::StopCore
                | CommandIntent::RestartCore
                | CommandIntent::SetProxyMode { .. }
        ) {
            return self.execute_lifecycle_with_id(request_id, intent).await;
        }
        self.push_event(CoreEvent::CommandAccepted { request_id, kind });
        let handler = self
            .inner
            .command_handler
            .read()
            .expect("command handler lock")
            .clone();
        let outcome = match handler {
            Some(handler) => {
                let expected = intent.clone();
                handler.handle_output(intent).await.and_then(|output| {
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
                self.push_event(CoreEvent::CommandCompleted { request_id, kind });
                CommandResult::Produced { request_id, output }
            }
            Err(failure) => self.finish_command(request_id, kind, Err(failure)),
        }
    }

    /// Internal lifecycle port calls can run inside a command's managed apply transaction.
    pub(super) async fn execute_lifecycle_with_id(
        &self,
        request_id: RequestId,
        intent: CommandIntent,
    ) -> CommandResult {
        let kind = intent.kind();
        self.push_event(CoreEvent::CommandAccepted { request_id, kind });
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
            CommandIntent::SetProxyMode { mode } => self.set_mode_locked(mode).await,
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
                self.push_event(CoreEvent::CommandCompleted { request_id, kind });
                CommandResult::Completed { request_id }
            }
            Err(failure) => {
                self.push_event(CoreEvent::CommandFailed {
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
        self.inner
            .command_handler
            .write()
            .expect("command handler lock")
            .take();
        let _operation = self.inner.operation.lock().await;
        if self.session_token().is_some() {
            self.stop_locked().await?;
        }
        Ok(())
    }
}

pub(super) fn closed_failure() -> Failure {
    Failure::new(ErrorCode::Canceled, "application is closed", false)
}
