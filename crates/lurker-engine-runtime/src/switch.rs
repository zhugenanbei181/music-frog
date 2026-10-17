//! Engine-neutral owner-switch coordination.
//!
//! The switch protocol is driven by [`coordinator::EngineSwitchCoordinator`].
//! Hosts implement [`host::EngineSwitchHost`]; the in-memory
//! [`memory_journal::MemoryEngineSwitchJournal`] implements the durable journal
//! port for the facade and for deterministic tests. Failure is always typed by
//! [`error::EngineSwitchError`].

pub mod coordinator;
pub mod error;
pub mod host;
pub mod memory_journal;

#[cfg(test)]
#[path = "switch/coordinator_tests.rs"]
mod tests;
