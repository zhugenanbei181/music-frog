//! Generic durable owner-switch machinery.
//!
//! The persisted wire stages, the active-Core owner proof and the four switch
//! leases are selection-agnostic: they only need a payload `S` and an injected
//! [`codec::OwnerSwitchCodec`] that owns the product's durable record layout
//! and selection projection. A product keeps its own record type (and any
//! product-only metadata) and implements the codec; this module never learns
//! product semantics.
//!
//! The persisted JSON of the owner proof and of the owner state is the same
//! format products already wrote before the move: no serde tag or field name is
//! changed, so there is no schema migration.

pub mod cas;
pub mod codec;
pub mod coordinator;
pub mod file;
pub mod leases;
pub mod owner;
pub mod recovery;
pub mod registry;
pub mod reset;
pub mod stage;

#[cfg(test)]
#[path = "owner_switch/generic_tests.rs"]
mod generic_tests;
