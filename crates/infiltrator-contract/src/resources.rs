//! Cross-surface core resource and soft-quota state.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

pub const CORE_MEMORY_SOFT_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum CoreGcStatus {
    #[default]
    Unknown,
    NotNeeded,
    Triggered {
        before_bytes: u64,
        after_bytes: Option<u64>,
    },
    Failed { failure: Failure },
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoreResourceSnapshot {
    pub memory_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,
    pub memory_soft_limit_bytes: u64,
    pub gc: CoreGcStatus,
}

impl Default for CoreResourceSnapshot {
    fn default() -> Self {
        Self {
            memory_bytes: None,
            cpu_percent: None,
            memory_soft_limit_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES,
            gc: CoreGcStatus::Unknown,
        }
    }
}

impl CoreResourceSnapshot {
    pub fn over_memory_limit(&self) -> bool {
        self.memory_bytes
            .is_some_and(|bytes| bytes > self.memory_soft_limit_bytes)
    }
}
