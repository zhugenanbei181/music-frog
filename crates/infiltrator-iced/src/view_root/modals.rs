//! Overlay modals and HUD dialogs for the view root.
//!
//! Each modal family lives in its own sibling module; this file only declares
//! them. The shared backdrop/card chrome is in [`card`].

pub(super) mod add_node;
pub(super) mod card;
pub(super) mod confirmation;
pub(super) mod proxy_inspect;
pub(super) mod rule_provider_diff;
