//! Typed, owner-scoped core resource observations.

use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;
use serde::{Deserialize, Serialize};

pub const CORE_MEMORY_SOFT_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

/// Availability of one resource fact. Missing support and failed reads stay
/// distinct from a real zero value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "availability", content = "value", rename_all = "snake_case")]
pub enum ResourceAvailability<T> {
    #[default]
    NotObserved,
    Available(T),
    Unsupported,
    Failed(Failure),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CoreGcStatus {
    #[default]
    NotAttempted,
    NotNeeded,
    Collecting,
    Collected {
        before_bytes: u64,
        after_bytes: u64,
    },
    Failed {
        failure: Failure,
    },
    Unsupported,
}

/// A surface-ready resource observation. An observed value always carries the
/// owner scope that produced it; no Optional data is used to encode support or
/// failure states.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "observation", rename_all = "snake_case")]
pub enum CoreResourceSnapshot {
    #[default]
    NotObserved,
    Observed {
        scope: Box<CoreScope>,
        memory: ResourceAvailability<u64>,
        cpu_percent: ResourceAvailability<f32>,
        memory_soft_limit_bytes: u64,
        gc: Box<CoreGcStatus>,
    },
}

impl CoreResourceSnapshot {
    pub fn over_memory_limit(&self) -> bool {
        match self {
            Self::NotObserved => false,
            Self::Observed {
                memory: ResourceAvailability::Available(bytes),
                memory_soft_limit_bytes,
                ..
            } => bytes > memory_soft_limit_bytes,
            Self::Observed { .. } => false,
        }
    }

    pub fn scope(&self) -> Option<&CoreScope> {
        match self {
            Self::NotObserved => None,
            Self::Observed { scope, .. } => Some(scope),
        }
    }
}
