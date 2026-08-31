//! Application types, grouped by business domain.
//!
//! Each submodule owns one domain's state/DTO types and is their single
//! authoritative path (`crate::types::message::Message`,
//! `crate::types::app::Route`, ...). No forwarding layer is allowed here.

pub mod app;
pub mod dns;
pub mod doctor;
pub mod editor;
pub mod message;
pub mod options;
pub mod perf;
pub mod rules;
pub mod runtime;
