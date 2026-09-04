//! Outbound ports used by the 0.30 application layer.
//!
//! Traits here describe capabilities, not implementations. They deliberately
//! contain no Tokio channels, task handles, HTTP response types, or UI types.

pub mod capability_provider;
pub mod core_process;
pub mod data_store;
pub mod error;
pub mod secure_store;
