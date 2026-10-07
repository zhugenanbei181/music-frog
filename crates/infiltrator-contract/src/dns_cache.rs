//! Typed independent DNS cache facts; configuration reads never author flush outcomes.
use crate::error::Failure;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsCacheOperationId(pub u64);

/// Outcome of one DNS cache flush target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsFlushOutcome {
    /// No flush has been requested in this session.
    #[default]
    NotRequested,
    /// The target accepted the flush.
    Flushed,
    /// The host has no drivable flush for this target.
    Unsupported { reason: String },
    /// The flush ran and failed.
    Failed { failure: Failure },
}

impl DnsFlushOutcome {
    pub fn is_flushed(&self) -> bool {
        matches!(self, Self::Flushed)
    }
}

/// Honest per-target report of the last DNS cache flush.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsCacheFlushReport {
    /// The running core's Fake-IP mapping cache.
    pub fake_ip: DnsFlushOutcome,
    /// The operating system resolver cache.
    pub os_cache: DnsFlushOutcome,
}

impl DnsCacheFlushReport {
    /// Classify only these recorded outcomes, independently of a newer running request.
    pub fn operation(&self) -> DnsCacheOperation {
        if matches!(self.fake_ip, DnsFlushOutcome::Failed { .. })
            || matches!(self.os_cache, DnsFlushOutcome::Failed { .. })
        {
            DnsCacheOperation::Failed
        } else if self.fake_ip.is_flushed() || self.os_cache.is_flushed() {
            DnsCacheOperation::Completed
        } else if self.is_requested() {
            DnsCacheOperation::Unsupported
        } else {
            DnsCacheOperation::Idle
        }
    }
    /// Whether any target has a recorded outcome (a flush was requested).
    pub fn is_requested(&self) -> bool {
        self.fake_ip != DnsFlushOutcome::NotRequested
            || self.os_cache != DnsFlushOutcome::NotRequested
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsCacheOperation {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Unsupported,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsCacheSnapshot {
    pub revision: u64,
    pub operation_id: Option<DnsCacheOperationId>,
    pub report_id: Option<DnsCacheOperationId>,
    pub operation: DnsCacheOperation,
    pub report: DnsCacheFlushReport,
    pub failure: Option<Failure>,
}
impl DnsCacheSnapshot {
    pub fn unavailable() -> Self {
        Self {
            operation: DnsCacheOperation::Unsupported,
            failure: Some(Failure::unsupported("DNS cache service is unavailable")),
            ..Self::default()
        }
    }
}
