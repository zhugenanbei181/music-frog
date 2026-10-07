//! The Bevy UI command-bus event vocabulary.
//!
//! Commands submitted through the sink may surface user notifications or a
//! completion/failure verdict back onto the event bus; these types are the
//! typed carriers for that feedback, kept beside the [`UiCommand`]
//! vocabulary they describe.
//!
//! [`UiCommand`]: crate::command::UiCommand

use crate::command::UiCommand;
use bevy::ecs::event::Event;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::Failure;

/// Notification severity level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// User notification event dispatched onto the event bus.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct UiNotificationEvent {
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
}

/// Event dispatched when a command completes or fails.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct CommandExecutedEvent {
    pub command: UiCommand,
    pub request_id: RequestId,
    pub result: Result<CommandOutput, Failure>,
}

impl CommandExecutedEvent {
    pub fn unit_result(&self) -> Result<(), Failure> {
        self.result.clone().and_then(CommandOutput::into_unit)
    }
    pub fn error_message(&self) -> Option<&str> {
        self.result
            .as_ref()
            .err()
            .map(|failure| failure.message.as_str())
    }
}
