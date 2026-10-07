//! Production command correlation and terminal feedback for native scenes.
use crate::command::{CommandSinkHandle, UiCommand, UiCommandSink};
use crate::command_events::CommandExecutedEvent;
use bevy::ecs::prelude::*;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::command::{CommandResult, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

struct PendingCommand {
    command: UiCommand,
    reply: Receiver<CommandResult>,
}

#[derive(Clone)]
pub struct ApplicationCommandSink {
    application: Arc<CoreApplication>,
    pending: Arc<Mutex<HashMap<RequestId, PendingCommand>>>,
}
impl ApplicationCommandSink {
    pub fn new(application: Arc<CoreApplication>) -> Self {
        Self {
            application,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn application(&self) -> &Arc<CoreApplication> {
        &self.application
    }
}
impl UiCommandSink for ApplicationCommandSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        if let Some(intent) = command.to_intent() {
            let mut pending = self.pending.lock().expect("command correlation lock");
            let (id, reply) = self.application.dispatch_tracked(intent);
            pending.insert(id, PendingCommand { command, reply });
            Some(id)
        } else {
            None
        }
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        let mut pending = self.pending.lock().expect("command correlation lock");
        let mut results = Vec::new();
        pending.retain(|request_id, item| {
            let result = match item.reply.try_recv() {
                Ok(CommandResult::Completed { .. }) => Ok(CommandOutput::Unit),
                Ok(CommandResult::Produced { output, .. }) => Ok(output),
                Ok(CommandResult::Rejected { failure, .. }) => Err(failure),
                Ok(CommandResult::Accepted { .. }) | Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => Err(Failure::new(
                    ErrorCode::NotReady,
                    "application acknowledgment channel disconnected",
                    true,
                )),
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
}

pub(crate) fn drain_command_results(handle: Res<CommandSinkHandle>, mut commands: Commands) {
    for result in handle.0.drain_results() {
        commands.trigger(result);
    }
}
