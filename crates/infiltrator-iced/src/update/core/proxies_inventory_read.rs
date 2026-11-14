//! Reading the owner's outbound inventory.
//!
//! Split from the proxy action core along the read/write seam: this module
//! only forwards a typed read command and publishes its readback as a message.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::error::{ErrorCode, Failure};

impl AppState {
    pub(crate) fn read_outbound_inventory(&self) -> Task<Message> {
        let Some(ingress) = self.command_ingress() else {
            return Task::done(Message::ShowToast(
                "Outbound inventory command is unavailable".to_owned(),
                ToastStatus::Error,
            ));
        };
        let Some(requested_scope) = ingress.observed_scope() else {
            return Task::done(Message::ShowToast(
                "Wait for an accepted Core scope before refreshing outbound inventory".to_owned(),
                ToastStatus::Info,
            ));
        };
        Task::perform(
            async move {
                let outcome = ingress
                    .execute_outcome(CommandIntent::ReadOutboundInventory)
                    .await;
                let terminal_scope = outcome.scope;
                let result = outcome
                    .result
                    .into_output()
                    .and_then(|output| match output {
                        CommandOutput::OutboundInventory(snapshot) => Ok(snapshot),
                        _ => Err(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound inventory command returned another output type",
                            false,
                        )),
                    });
                (terminal_scope, result)
            },
            move |(terminal_scope, result)| Message::OutboundInventoryReadFinished {
                requested_scope: Some(requested_scope),
                terminal_scope,
                result,
            },
        )
    }
}
