//! Pure domain logic for MusicFrog Infiltrator.
//!
//! This crate is deliberately independent from Tokio, network clients,
//! filesystems, operating-system APIs, and UI toolkits. It may be embedded by
//! any native surface or tested without an executor.

pub mod core_state;
