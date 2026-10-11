//! Production command correlation and terminal feedback for native scenes.
use crate::command::CommandSinkHandle;
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use crate::command::{UiCommand, UiCommandSink};
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use crate::command_events::CommandExecutedEvent;
use bevy::ecs::prelude::*;
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_application::command_capability::validate_terminal_scope;
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_application::core_application::{CoreApplication, CoreScopeObservation};
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_contract::command::{CommandResult, RequestId};
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_contract::command_output::CommandOutput;
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreCommandRequest, CoreScope};
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_contract::error::{ErrorCode, Failure};
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use infiltrator_ports::profile_document_codec::ProfileMixinToggleId;
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use std::collections::HashMap;
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use std::sync::mpsc::{Receiver, TryRecvError};
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
use std::sync::{Arc, Mutex, RwLock};

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
struct PendingCommand {
    command: UiCommand,
    reply: PendingReply,
}

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
enum PendingReply {
    Scoped {
        request: Box<CoreCommandRequest>,
        reply: Receiver<CoreCommandOutcome>,
    },
    Immediate(CoreCommandOutcome),
}

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
#[derive(Clone)]
pub struct ApplicationCommandSink {
    application: Arc<CoreApplication>,
    observed_scope: Arc<RwLock<CoreScopeObservation>>,
    pending: Arc<Mutex<HashMap<RequestId, PendingCommand>>>,
}
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
impl ApplicationCommandSink {
    /// Create the product sink for a surface source that publishes only the
    /// scope of its latest accepted snapshot.
    pub fn new_for_surface(
        application: Arc<CoreApplication>,
        observed_scope: Arc<RwLock<CoreScopeObservation>>,
    ) -> Self {
        Self {
            application,
            observed_scope,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn application(&self) -> &Arc<CoreApplication> {
        &self.application
    }
}
#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
impl UiCommandSink for ApplicationCommandSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let Some(scope) = observed_scope(&self.observed_scope) else {
            let outcome = self.application.reject_missing_scope(command.to_intent()?);
            let request_id = result_request_id(&outcome.result);
            self.pending
                .lock()
                .expect("command correlation lock")
                .insert(
                    request_id,
                    PendingCommand {
                        command,
                        reply: PendingReply::Immediate(outcome),
                    },
                );
            return Some(request_id);
        };

        if let Some(intent) = command.to_intent() {
            let mut pending = self.pending.lock().expect("command correlation lock");
            let request = CoreCommandRequest { scope, intent };
            let (id, reply) = self.application.dispatch_scoped_tracked(request.clone());
            pending.insert(
                id,
                PendingCommand {
                    command,
                    reply: PendingReply::Scoped {
                        request: Box::new(request),
                        reply,
                    },
                },
            );
            Some(id)
        } else {
            None
        }
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        let mut pending = self.pending.lock().expect("command correlation lock");
        let mut results = Vec::new();
        pending.retain(|request_id, item| {
            let result = match &mut item.reply {
                PendingReply::Scoped { request, reply } => match reply.try_recv() {
                    Ok(outcome) => {
                        let expected_id = *request_id;
                        if result_request_id(&outcome.result) != expected_id {
                            Err(Failure::new(
                                ErrorCode::InvalidState,
                                "application acknowledgment request ID did not match",
                                false,
                            ))
                        } else {
                            let outcome =
                                validate_terminal_scope(&request.scope, &request.intent, outcome);
                            command_result(outcome.result)
                        }
                    }
                    Err(TryRecvError::Empty) => return true,
                    Err(TryRecvError::Disconnected) => Err(disconnected_failure()),
                },
                PendingReply::Immediate(outcome) => command_result(outcome.result.clone()),
            };
            results.push(CommandExecutedEvent {
                command: item.command.clone(),
                request_id: *request_id,
                result,
            });
            false
        });
        results
    }

    fn inspect_profile_buffer(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        content: &str,
    ) -> Result<infiltrator_contract::profile_document::DocumentInspection, Failure> {
        self.application.inspect_profile_buffer(identity, content)
    }

    fn insert_profile_snippet(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome, Failure>
    {
        self.application
            .insert_profile_snippet(identity, content, snippet_id, line, column)
    }

    fn inspect_mixin_draft(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.application
            .inspect_mixin_draft(identity, document_content, mixin_yaml)
    }

    fn inspect_mixin_toggle(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.application
            .inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    fn set_mixin_toggle(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.application
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    fn inspect_mixin_studio(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<infiltrator_ports::profile_document_codec::ProfileMixinStudioProjection, Failure>
    {
        self.application
            .inspect_mixin_studio(identity, document_content, mixin_yaml)
    }
}

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
fn observed_scope(observed: &Arc<RwLock<CoreScopeObservation>>) -> Option<CoreScope> {
    match observed.read().expect("observed Core scope lock").clone() {
        CoreScopeObservation::Observed(scope) => Some(scope),
        CoreScopeObservation::Unavailable => None,
    }
}

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
fn command_result(result: CommandResult) -> Result<CommandOutput, Failure> {
    match result {
        CommandResult::Completed { .. } => Ok(CommandOutput::Unit),
        CommandResult::Produced { output, .. } => Ok(*output),
        CommandResult::Rejected { failure, .. } => Err(failure),
        CommandResult::Accepted { .. } => Err(Failure::new(
            ErrorCode::InvalidState,
            "application acknowledgment was not terminal",
            false,
        )),
    }
}

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
fn result_request_id(result: &CommandResult) -> RequestId {
    match result {
        CommandResult::Accepted { request_id }
        | CommandResult::Completed { request_id }
        | CommandResult::Produced { request_id, .. }
        | CommandResult::Rejected { request_id, .. } => *request_id,
    }
}

#[cfg(any(feature = "capture-tools", feature = "desktop-host"))]
fn disconnected_failure() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "application acknowledgment channel disconnected",
        true,
    )
}

pub(crate) fn drain_command_results(handle: Res<CommandSinkHandle>, mut commands: Commands) {
    for result in handle.0.drain_results() {
        commands.trigger(result);
    }
}
