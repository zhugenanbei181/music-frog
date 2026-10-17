//! Durable owner-switch lease transitions shared by all engine stores.
//!
//! The lease implementations live in the neutral
//! [`lurker_profile_store::owner_switch::leases`] module; this module keeps the
//! product revision conversion and test-only mount points.

#[path = "active_owner_switch/stage_mapping.rs"]
pub mod stage_mapping;

#[cfg(test)]
#[path = "active_owner_switch_tests.rs"]
mod tests;
