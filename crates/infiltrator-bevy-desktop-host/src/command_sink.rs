use crate::surface::observed_scope;
use infiltrator_application::command_capability::validate_terminal_scope;
use infiltrator_application::core_application::{CoreApplication, CoreScopeObservation};
use infiltrator_bevy_ui::command::{UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_contract::command::{CommandResult, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreCommandRequest};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::profile_document_codec::ProfileMixinToggleId;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex, RwLock};

struct PendingCommand {
    command: UiCommand,
    reply: PendingReply,
}

enum PendingReply {
    Scoped {
        request: Box<CoreCommandRequest>,
        reply: Receiver<CoreCommandOutcome>,
    },
    Immediate(CoreCommandOutcome),
}

/// Bevy desktop command adapter. It is owned by the desktop composition root,
/// while the UI crate only receives the `UiCommandSink` contract.
#[derive(Clone)]
pub struct DesktopCommandSink {
    application: Arc<CoreApplication>,
    observed_scope: Arc<RwLock<CoreScopeObservation>>,
    pending: Arc<Mutex<HashMap<RequestId, PendingCommand>>>,
}

impl DesktopCommandSink {
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
}

impl UiCommandSink for DesktopCommandSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }

    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let intent = command.to_intent()?;
        match observed_scope(&self.observed_scope) {
            CoreScopeObservation::Unavailable => {
                let outcome = self.application.reject_missing_scope(intent);
                let request_id = result_request_id(&outcome.result);
                self.pending
                    .lock()
                    .expect("desktop command correlation lock")
                    .insert(
                        request_id,
                        PendingCommand {
                            command,
                            reply: PendingReply::Immediate(outcome),
                        },
                    );
                Some(request_id)
            }
            CoreScopeObservation::Observed(scope) => {
                let mut pending = self
                    .pending
                    .lock()
                    .expect("desktop command correlation lock");
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
            }
        }
    }

    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        let mut pending = self
            .pending
            .lock()
            .expect("desktop command correlation lock");
        let mut results = Vec::new();
        pending.retain(|request_id, item| {
            let result = match &mut item.reply {
                PendingReply::Scoped { request, reply } => match reply.try_recv() {
                    Ok(outcome) => {
                        if result_request_id(&outcome.result) != *request_id {
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

fn result_request_id(result: &CommandResult) -> RequestId {
    match result {
        CommandResult::Accepted { request_id }
        | CommandResult::Completed { request_id }
        | CommandResult::Produced { request_id, .. }
        | CommandResult::Rejected { request_id, .. } => *request_id,
    }
}

fn disconnected_failure() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "application acknowledgment channel disconnected",
        true,
    )
}
