//! Unified DomainState<T> state machine for Bevy UI data models.
//!
//! Charter law (docs/BEVY_UI_FRONTEND.md & docs/FRONTENDS.md):
//! Every domain surface (Overview, Proxies, Profiles, Rules, Connections,
//! Logs, DNS, Doctor, App Routing, Sync, Settings) projects state through
//! an explicit, typed state machine (Idle / Loading / Ready / Error)
//! without fabricating dummy values or concealing errors. Includes 64-bit
//! state fingerprinting to prevent dirty redundant UI restamping.

use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

/// Compute a 64-bit non-cryptographic content fingerprint for a hashable structure.
pub fn compute_hash_fingerprint<H: Hash + ?Sized>(item: &H) -> u64 {
    let mut hasher = DefaultHasher::new();
    item.hash(&mut hasher);
    hasher.finish()
}

/// Lifecycle phase of a domain dataset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DomainPhase {
    /// Initial uninitialized or idle state before the first fetch starts.
    #[default]
    Idle,
    /// Asynchronous fetch or transition in progress (retaining any previous cached data).
    Loading,
    /// Valid data snapshot ready for presentation.
    Ready,
    /// An error or unreachable condition occurred.
    Error,
}

/// Generic container for domain state, tracking payload, error, timestamp, sequence, and fingerprint.
#[derive(Clone, Debug, PartialEq)]
pub struct DomainState<T> {
    /// Current lifecycle phase.
    pub phase: DomainPhase,
    /// Domain payload when available (even during loading/error as a cached fallback).
    pub data: Option<T>,
    /// Human-readable failure description if in Error phase.
    pub error: Option<String>,
    /// Sample timestamp relative to the source origin.
    pub sampled_at: Duration,
    /// Monotonically increasing revision counter.
    pub sequence: u64,
    /// 64-bit content hash fingerprint to detect true mutations.
    pub fingerprint: u64,
}

impl<T> Default for DomainState<T> {
    fn default() -> Self {
        Self::idle()
    }
}

impl<T> DomainState<T> {
    /// Create an idle domain state without data.
    pub fn idle() -> Self {
        Self {
            phase: DomainPhase::Idle,
            data: None,
            error: None,
            sampled_at: Duration::ZERO,
            sequence: 0,
            fingerprint: 0,
        }
    }

    /// Create a loading domain state, optionally retaining cached data.
    pub fn loading(cached: Option<T>) -> Self {
        Self {
            phase: DomainPhase::Loading,
            data: cached,
            error: None,
            sampled_at: Duration::ZERO,
            sequence: 0,
            fingerprint: 0,
        }
    }

    /// Create a ready domain state with valid payload.
    pub fn ready(data: T, sampled_at: Duration) -> Self {
        Self {
            phase: DomainPhase::Ready,
            data: Some(data),
            error: None,
            sampled_at,
            sequence: 1,
            fingerprint: 0,
        }
    }

    /// Create a ready domain state with explicit fingerprint.
    pub fn ready_with_fingerprint(data: T, sampled_at: Duration, fingerprint: u64) -> Self {
        Self {
            phase: DomainPhase::Ready,
            data: Some(data),
            error: None,
            sampled_at,
            sequence: 1,
            fingerprint,
        }
    }

    /// Create an error domain state with reason, optionally retaining cached data.
    pub fn error(reason: impl Into<String>, cached: Option<T>) -> Self {
        Self {
            phase: DomainPhase::Error,
            data: cached,
            error: Some(reason.into()),
            sampled_at: Duration::ZERO,
            sequence: 1,
            fingerprint: 0,
        }
    }

    /// Check if state is in Idle phase.
    pub fn is_idle(&self) -> bool {
        self.phase == DomainPhase::Idle
    }

    /// Check if state is in Loading phase.
    pub fn is_loading(&self) -> bool {
        self.phase == DomainPhase::Loading
    }

    /// Check if state is in Ready phase.
    pub fn is_ready(&self) -> bool {
        self.phase == DomainPhase::Ready
    }

    /// Check if state is in Error phase.
    pub fn is_error(&self) -> bool {
        self.phase == DomainPhase::Error
    }

    /// Borrow the payload data if available.
    pub fn data(&self) -> Option<&T> {
        self.data.as_ref()
    }

    /// Mutably borrow the payload data if available.
    pub fn data_mut(&mut self) -> Option<&mut T> {
        self.data.as_mut()
    }

    /// Consume into optional data payload.
    pub fn into_data(self) -> Option<T> {
        self.data
    }

    /// Read error reason if in error state.
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Transition into Loading phase, retaining existing payload.
    pub fn transition_to_loading(&mut self) {
        self.phase = DomainPhase::Loading;
        self.error = None;
        self.sequence = self.sequence.wrapping_add(1);
    }

    /// Transition into Ready phase with new payload and sample time.
    pub fn transition_to_ready(&mut self, data: T, sampled_at: Duration) {
        self.phase = DomainPhase::Ready;
        self.data = Some(data);
        self.error = None;
        self.sampled_at = sampled_at;
        self.sequence = self.sequence.wrapping_add(1);
    }

    /// Transition into Ready phase with fingerprint.
    /// Returns `true` if this represents a true mutation (dirty state).
    pub fn transition_to_ready_with_fingerprint(
        &mut self,
        data: T,
        sampled_at: Duration,
        fingerprint: u64,
    ) -> bool {
        let is_dirty = self.phase != DomainPhase::Ready || self.fingerprint != fingerprint;
        self.phase = DomainPhase::Ready;
        self.data = Some(data);
        self.error = None;
        self.sampled_at = sampled_at;
        self.sequence = self.sequence.wrapping_add(1);
        self.fingerprint = fingerprint;
        is_dirty
    }

    /// Transition into Error phase with given error message, retaining payload.
    pub fn transition_to_error(&mut self, reason: impl Into<String>) {
        self.phase = DomainPhase::Error;
        self.error = Some(reason.into());
        self.sequence = self.sequence.wrapping_add(1);
    }

    /// Map inner data payload to a new type while preserving phase, error, sequence, and fingerprint.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> DomainState<U> {
        DomainState {
            phase: self.phase,
            data: self.data.map(f),
            error: self.error,
            sampled_at: self.sampled_at,
            sequence: self.sequence,
            fingerprint: self.fingerprint,
        }
    }

    /// Borrow inner payload as reference.
    pub fn as_ref(&self) -> DomainState<&T> {
        DomainState {
            phase: self.phase,
            data: self.data.as_ref(),
            error: self.error.clone(),
            sampled_at: self.sampled_at,
            sequence: self.sequence,
            fingerprint: self.fingerprint,
        }
    }
}

/// Generic ECS resource wrapping a domain state.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct DomainResource<T: Send + Sync + 'static>(pub DomainState<T>);

impl<T: Send + Sync + 'static> Default for DomainResource<T> {
    fn default() -> Self {
        Self(DomainState::default())
    }
}

/// Typed event triggered whenever a domain state is updated.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct DomainStateUpdated<T: Send + Sync + 'static>(pub DomainState<T>);

/// Comprehensive anti-entropy consistency audit report across all 11 UI domain state machines.
#[derive(Clone, Debug, PartialEq)]
pub struct DomainAntiEntropyReport {
    pub total_domains: usize,
    pub ready_count: usize,
    pub stale_domains: Vec<String>,
    pub is_system_healthy: bool,
}

/// Audit consistency across a collection of domain dataset snapshots.
pub fn audit_domain_anti_entropy(
    domain_snapshots: &[(&str, DomainPhase, Duration)],
    now_duration: Duration,
    max_stale_limit: Duration,
) -> DomainAntiEntropyReport {
    let total = domain_snapshots.len();
    let mut ready = 0;
    let mut stale = Vec::new();

    for &(name, phase, sampled_at) in domain_snapshots {
        let is_stale = now_duration.saturating_sub(sampled_at) > max_stale_limit;
        if phase == DomainPhase::Ready && !is_stale {
            ready += 1;
        } else {
            stale.push(name.to_string());
        }
    }

    let is_system_healthy = ready == total && total > 0;
    DomainAntiEntropyReport {
        total_domains: total,
        ready_count: ready,
        stale_domains: stale,
        is_system_healthy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_state_lifecycle_transitions() {
        let mut state: DomainState<Vec<String>> = DomainState::idle();
        assert!(state.is_idle());
        assert_eq!(state.sequence, 0);

        state.transition_to_loading();
        assert!(state.is_loading());
        assert_eq!(state.sequence, 1);
        assert!(state.data().is_none());

        state.transition_to_ready(vec!["item1".into()], Duration::from_millis(50));
        assert!(state.is_ready());
        assert_eq!(state.sequence, 2);
        assert_eq!(state.data().unwrap(), &vec!["item1".to_string()]);
        assert_eq!(state.sampled_at, Duration::from_millis(50));

        state.transition_to_error("Network timeout");
        assert!(state.is_error());
        assert_eq!(state.sequence, 3);
        assert_eq!(state.error_message(), Some("Network timeout"));
        assert_eq!(state.data().unwrap(), &vec!["item1".to_string()]);
    }

    #[test]
    fn domain_state_fingerprint_change_detection() {
        let mut state = DomainState::idle();
        let fp1 = compute_hash_fingerprint(&"alpha");
        let fp2 = compute_hash_fingerprint(&"beta");

        // Transition from Idle to Ready is dirty
        assert!(state.transition_to_ready_with_fingerprint(
            "alpha".to_string(),
            Duration::ZERO,
            fp1
        ));

        // Successive update with same fingerprint is NOT dirty
        assert!(!state.transition_to_ready_with_fingerprint(
            "alpha".to_string(),
            Duration::from_secs(1),
            fp1
        ));

        // Different fingerprint IS dirty
        assert!(state.transition_to_ready_with_fingerprint(
            "beta".to_string(),
            Duration::from_secs(2),
            fp2
        ));
    }

    #[test]
    fn domain_state_map_transforms_payload() {
        let ready = DomainState::ready_with_fingerprint(42, Duration::from_secs(1), 12345);
        let mapped = ready.map(|n| format!("number: {n}"));
        assert!(mapped.is_ready());
        assert_eq!(mapped.data().unwrap(), "number: 42");
        assert_eq!(mapped.sampled_at, Duration::from_secs(1));
        assert_eq!(mapped.fingerprint, 12345);
    }
    #[test]
    fn test_domain_anti_entropy_audit() {
        let now = Duration::from_secs(100);
        let max_stale = Duration::from_secs(30);

        let snapshots = vec![
            ("Overview", DomainPhase::Ready, Duration::from_secs(95)), // Fresh (5s old)
            ("Proxies", DomainPhase::Ready, Duration::from_secs(98)),  // Fresh (2s old)
            ("Logs", DomainPhase::Loading, Duration::from_secs(50)),   // Stale & Loading (50s old)
        ];

        let report = audit_domain_anti_entropy(&snapshots, now, max_stale);
        assert_eq!(report.total_domains, 3);
        assert_eq!(report.ready_count, 2);
        assert_eq!(report.stale_domains, vec!["Logs"]);
        assert!(!report.is_system_healthy);
    }
}
