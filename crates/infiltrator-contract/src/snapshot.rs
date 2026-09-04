use crate::command::{CommandKind, ProxyMode, RequestId};
use crate::error::Failure;
use serde::{Deserialize, Serialize};

/// Stable lifecycle vocabulary for REST, UI projection, and FFI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreLifecycle {
    Stopped,
    Starting,
    Ready,
    Running,
    Stopping,
    Failed,
}

/// A read-only Core projection. Secrets and client objects never cross this
/// boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoreSnapshot {
    pub lifecycle: CoreLifecycle,
    pub generation: u64,
    pub revision: u64,
    pub proxy_mode: Option<ProxyMode>,
    pub core_version: Option<String>,
    pub sampled_at_epoch_ms: Option<i64>,
    pub failure: Option<Failure>,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub active_connections: u32,
    pub memory_bytes: Option<u64>,
}

/// Result of a user-requested public-egress probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpSnapshot {
    pub ip: String,
    pub provider: String,
    pub checked_at_epoch_ms: i64,
}

/// Bounded, surface-neutral events emitted by the application layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CoreEvent {
    SnapshotUpdated(CoreSnapshot),
    CommandAccepted {
        request_id: RequestId,
        kind: CommandKind,
    },
    CommandCompleted {
        request_id: RequestId,
        kind: CommandKind,
    },
    CommandFailed {
        request_id: RequestId,
        kind: CommandKind,
        failure: Failure,
    },
}
