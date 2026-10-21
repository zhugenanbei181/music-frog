//! The typed product-command channel between the Android UI process and the
//! `:vpn` owner.
//!
//! The owner answers every product command with one terminal that carries the
//! typed output it produced. This module keeps that output on the wire and in
//! the readback, so a surface that waits for an owner identity learns that
//! identity instead of learning only that a command was dispatched.
//!
//! The transport is injected, so the Android host and its tests speak the
//! exact same channel: the host hands the owner IPC call in, and the owner's
//! terminal comes back out.

use infiltrator_contract::android_control::{
    AndroidProductCommand, AndroidProductCommandOutcome, AndroidProductCommandRequest,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::CoreCommandOutcome;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;

/// What the owner answered for one command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerAnswer {
    /// The owner produced a typed output for the command.
    Produced(Box<CommandOutput>),
    /// The owner finished the command without a typed output.
    Completed,
}

/// One owner terminal: the scope the owner still holds and what it answered.
///
/// The scope is the owner's own readback of the Core activation, so the UI
/// process never guesses whether the owner still holds the activation the
/// command was scoped to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerTerminal {
    /// The Core scope the owner holds after the command.
    pub scope: Option<CoreScope>,
    /// What the owner answered for the command.
    pub answer: OwnerAnswer,
}

/// Encodes one application command for the `:vpn` owner.
///
/// The payload is the shared contract envelope, so the owner deserializes the
/// command without knowing which surface sent it.
pub fn encode_application_command(
    scope: &CoreScope,
    intent: CommandIntent,
) -> Result<String, Failure> {
    let request = AndroidProductCommandRequest {
        scope: scope.clone(),
        command: AndroidProductCommand::Application(Box::new(intent)),
    };
    serde_json::to_string(&request).map_err(|error| {
        Failure::new(
            ErrorCode::Internal,
            format!("Android command could not be encoded: {error}"),
            false,
        )
    })
}

/// Sends one application command to the owner and decodes its typed terminal.
///
/// The transport is the Android owner IPC call. The owner answers with the
/// typed output it produced, and this function keeps that output instead of
/// collapsing it into an acknowledgement.
pub fn execute_owner_command(
    scope: &CoreScope,
    intent: CommandIntent,
    transport: &dyn Fn(&str) -> Result<String, Failure>,
) -> Result<OwnerTerminal, Failure> {
    let request_json = encode_application_command(scope, intent)?;
    let outcome_json = transport(&request_json)?;
    decode_owner_terminal(&outcome_json)
}

/// Runs the owner's commands in dispatch order and reports the typed readback.
///
/// The owner answers in dispatch order, so the typed output of the last
/// command is the readback of the command the surface is waiting for. A
/// command the owner finished without a typed output stays an acknowledgement:
/// it never replaces an output the owner really produced, and it never claims
/// an owner identity the owner did not give.
pub fn execute_owner_commands(
    scope: CoreScope,
    intents: Vec<CommandIntent>,
    transport: &dyn Fn(&str) -> Result<String, Failure>,
) -> Result<CommandOutput, Failure> {
    let mut scope = scope;
    let mut output = CommandOutput::Unit;
    let command_count = intents.len();
    for (index, intent) in intents.into_iter().enumerate() {
        let terminal = execute_owner_command(&scope, intent, transport)?;
        let OwnerTerminal {
            scope: next_scope,
            answer,
        } = terminal;
        match next_scope {
            Some(next_scope) => scope = next_scope,
            None if index + 1 < command_count => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "The Android owner retired the Core scope during a command sequence",
                    true,
                ));
            }
            None => {}
        }
        if let OwnerAnswer::Produced(produced) = answer {
            output = *produced;
        }
    }
    Ok(output)
}

/// Decodes the owner's terminal into the typed readback it carries.
///
/// A refusal the owner gave stays the owner's own typed failure; a command the
/// owner finished without a typed output stays an acknowledgement, never a
/// fabricated identity.
pub fn decode_owner_terminal(raw: &str) -> Result<OwnerTerminal, Failure> {
    let outcome = serde_json::from_str::<AndroidProductCommandOutcome>(raw).map_err(|error| {
        Failure::new(
            ErrorCode::Internal,
            format!("Android owner command result could not be decoded: {error}"),
            true,
        )
    })?;
    match outcome {
        AndroidProductCommandOutcome::Unavailable { failure, .. } => Err(failure),
        AndroidProductCommandOutcome::Terminal(outcome) => {
            let CoreCommandOutcome { scope, result } = *outcome;
            match result {
                CommandResult::Completed { .. } | CommandResult::Accepted { .. } => {
                    Ok(OwnerTerminal {
                        scope,
                        answer: OwnerAnswer::Completed,
                    })
                }
                CommandResult::Produced { output, .. } => Ok(OwnerTerminal {
                    scope,
                    answer: OwnerAnswer::Produced(Box::new(*output)),
                }),
                CommandResult::Rejected { failure, .. } => Err(failure),
            }
        }
    }
}

#[cfg(test)]
#[path = "owner_command_tests.rs"]
mod tests;
