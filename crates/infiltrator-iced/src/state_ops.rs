//! Lightweight [`AppState`] operations split out of `state.rs`.
//!
//! The domain structs and the shared-surface projection stay in `state.rs`
//! because the parity guards anchor their canonical fields and assignments
//! there; these are the small command/plumbing helpers that carry no marker.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::sanitize_ui_text;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::overview_layout::OverviewCardMoveDirection;
use std::fmt::Display;
use std::sync::Arc;

impl AppState {
    /// Ask the owner to move one overview card.
    ///
    /// The card order is the owner's fact: the surface dispatches the shared
    /// intent and renders the order the owner publishes, so the surface never
    /// keeps a second copy of it.
    pub fn dispatch_overview_card_move(
        &mut self,
        kind: OverviewCardKind,
        direction: OverviewCardMoveDirection,
    ) -> Task<Message> {
        let Some(commands) = self.command_ingress() else {
            return Task::none();
        };
        Task::perform(
            async move {
                match commands
                    .execute(CommandIntent::MoveOverviewCard { kind, direction })
                    .await
                    .into_output()
                    .and_then(CommandOutput::into_overview_layout)
                {
                    Ok(snapshot) => Message::OverviewCardOrderApplied(snapshot),
                    Err(failure) => Message::OverviewCardOrderFailed(failure),
                }
            },
            |message| message,
        )
    }

    /// Ask the owner to restore the canonical overview card order.
    pub fn dispatch_overview_card_reset(&mut self) -> Task<Message> {
        let Some(commands) = self.command_ingress() else {
            return Task::none();
        };
        Task::perform(
            async move {
                match commands
                    .execute(CommandIntent::ResetOverviewCardOrder)
                    .await
                    .into_output()
                    .and_then(CommandOutput::into_overview_layout)
                {
                    Ok(snapshot) => Message::OverviewCardOrderApplied(snapshot),
                    Err(failure) => Message::OverviewCardOrderFailed(failure),
                }
            },
            |message| message,
        )
    }

    pub fn attach_exit_cleanup(&mut self, cleanup: Arc<dyn Fn() + Send + Sync>) {
        self.exit_cleanup = Some(cleanup);
    }

    /// Single choke point for `error_msg`: raw error chains can embed
    /// subscription query tokens or the controller secret, so the text is
    /// redacted here before any view can render it (CORE-001).
    pub fn set_error(&mut self, source: impl Display) {
        self.shell.error_msg = Some(sanitize_ui_text(&source.to_string()));
    }
}
