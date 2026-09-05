//! Runtime-neutral port for managed-core crash recovery.

use async_trait::async_trait;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreWatchdogSnapshot;

use crate::error::PortError;

/// Result of one bounded watchdog poll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchdogTick {
    Healthy,
    Waiting {
        session_token: SessionToken,
        attempt: u32,
        retry_in_ms: u64,
    },
    Restarting {
        session_token: SessionToken,
        attempt: u32,
    },
    Recovered {
        session_token: SessionToken,
        attempts: u32,
    },
    Tripped {
        session_token: Option<SessionToken>,
        attempts: u32,
    },
}

/// Application-owned crash watchdog capability.
///
/// A host only has to call `watchdog_tick` from its own scheduler. The
/// application performs the process probe, backoff decision and serialized
/// restart; no UI or transport adapter may implement a second retry loop.
#[async_trait]
pub trait CoreWatchdogPort: Send + Sync {
    fn watchdog_snapshot(&self) -> CoreWatchdogSnapshot;

    async fn watchdog_tick(&self) -> Result<WatchdogTick, PortError>;
}
