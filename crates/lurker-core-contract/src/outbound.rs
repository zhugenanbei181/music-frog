//! Engine-neutral outbound contract, split by identity, operation, and observation domain.

pub mod details;
pub mod identity;
pub mod inventory;
pub mod preferences;
pub mod probe;

#[cfg(test)]
#[path = "outbound_tests.rs"]
mod tests;
