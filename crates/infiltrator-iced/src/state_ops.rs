//! Lightweight [`AppState`] operations split out of `state.rs`.
//!
//! The domain structs and the shared-surface projection stay in `state.rs`
//! because the parity guards anchor their canonical fields and assignments
//! there; these are the small command/plumbing helpers that carry no marker.

use crate::state::AppState;
use std::sync::Arc;

impl AppState {
    /// Move an Overview card up/down using the shared layout operators, then
    /// store the resulting order. Reusing `OverviewLayoutSnapshot` guarantees
    /// the Iced surface applies the exact same swap semantics as Bevy.
    pub fn move_overview_card(
        &mut self,
        kind: infiltrator_contract::overview_layout::OverviewCardKind,
        up: bool,
    ) {
        let mut layout = infiltrator_contract::overview_layout::OverviewLayoutSnapshot::new(
            self.diag.overview_card_order.clone(),
        );
        let changed = if up {
            layout.move_up(kind)
        } else {
            layout.move_down(kind)
        };
        if changed {
            self.diag.overview_card_order = layout.order;
        }
    }

    pub fn attach_exit_cleanup(&mut self, cleanup: Arc<dyn Fn() + Send + Sync>) {
        self.exit_cleanup = Some(cleanup);
    }

    /// Single choke point for `error_msg`: raw error chains can embed
    /// subscription query tokens or the controller secret, so the text is
    /// redacted here before any view can render it (CORE-001).
    pub fn set_error(&mut self, source: impl std::fmt::Display) {
        self.shell.error_msg = Some(crate::utils::sanitize_ui_text(&source.to_string()));
    }
}
