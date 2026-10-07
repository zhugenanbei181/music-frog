//! Application types, grouped by business domain.
//!
//! Each submodule owns one domain's state/DTO types and is their single
//! authoritative path (`crate::types::message::Message`,
//! `crate::types::app::Route`, ...). No forwarding layer is allowed here.

pub mod app;
pub mod app_routing;
pub mod dns;
pub mod doctor;
pub mod editor;
pub mod message;
pub mod options;
pub mod perf;
pub mod rule_list;
pub mod rule_trace;
pub mod rules;
pub mod runtime;

pub mod dns_query;

pub mod script;
pub mod snapshot_restore;

pub mod profile_edit;
