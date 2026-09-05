//! Cross-surface controller authentication status.
//!
//! The secret itself is never part of this contract. Hosts generate and
//! retain it privately; surfaces receive only an honest status badge.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerAuthStatus {
    #[default]
    Unknown,
    Secured,
    Missing,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerAuthSnapshot {
    pub status: ControllerAuthStatus,
}
