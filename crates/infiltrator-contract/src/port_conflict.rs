//! Cross-surface port-conflict observations and safe repair results.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortBinding {
    MixedProxy,
    Controller,
}

impl PortBinding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MixedProxy => "mixed-port",
            Self::Controller => "external-controller",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortConflict {
    pub binding: PortBinding,
    pub port: u16,
    pub available: bool,
    pub owner_pid: Option<u32>,
    pub owner_name: Option<String>,
    pub can_release: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortConflictSnapshot {
    pub revision: u64,
    pub conflicts: Vec<PortConflict>,
}

impl PortConflictSnapshot {
    pub fn has_conflicts(&self) -> bool {
        self.conflicts.iter().any(|conflict| !conflict.available)
    }
}
