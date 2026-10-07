//! Overlay modals and HUD dialogs for the view root.
//!
//! Each modal family lives in its own sibling module; this file only declares
//! them. The shared backdrop/card chrome is in [`card`].

pub(super) mod add_node;
pub(super) mod card;
pub(super) mod confirmation;
pub(super) mod dns_cache;
pub(super) mod dns_hosts;
pub(super) mod dns_query;
pub(crate) mod log_export;
pub(super) mod proxy_group_order;
mod proxy_history;
pub(super) mod proxy_inspect;
pub(super) mod proxy_probe_settings;
pub(super) mod rule_provider_diff;

pub(crate) mod rule_statistics;

pub(super) mod script_export;

pub(crate) mod snapshot_restore;
