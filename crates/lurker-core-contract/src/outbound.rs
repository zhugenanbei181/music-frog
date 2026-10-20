//! Engine-neutral outbound contract, split by identity, operation, and observation domain.
//!
//! The module is layered so a consumer only pays for what it uses:
//!
//! * [`identity`] owns the opaque, engine-scoped identifiers and the typed
//!   contract violations every other submodule reports.
//! * [`details`] owns the per-outbound facts an engine adapter observes.
//! * [`inventory`] owns the inventory/selection observation and its scope
//!   fences.
//! * [`preferences`] owns the engine-scoped presentation preferences.
//! * [`probe`] owns the selection and probe requests plus their readbacks.

pub mod details;
pub mod identity;
pub mod inventory;
pub mod preferences;
pub mod probe;

#[cfg(test)]
#[path = "outbound_tests.rs"]
mod tests;
