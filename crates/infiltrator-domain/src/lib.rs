//! Pure domain logic for MusicFrog Infiltrator.
//!
//! This crate is deliberately independent from Tokio, network clients,
//! filesystems, operating-system APIs, and UI toolkits. It may be embedded by
//! any native surface or tested without an executor.

pub mod backoff_strategy;
pub mod core_state;
pub mod mtu_optimizer;
pub mod packet_loss_tracker;
pub mod rule_hit_counter;
pub mod sub_rules;
pub mod vector_clock;
