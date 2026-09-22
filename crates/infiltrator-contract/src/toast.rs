//! Shared in-app notification contract: severity vocabulary plus the
//! dedup/capacity policy behind the toast stack.
//!
//! Text redaction intentionally stays with `infiltrator_domain::redact`
//! (both surfaces already depend on it and Iced routes every toast through
//! it); this module owns the part both surfaces must agree on numerically —
//! which severities exist, how long an identical toast is coalesced, and how
//! many toasts may be on screen at once.

use serde::{Deserialize, Serialize};

/// Severity of one toast. Both surfaces map their local enum onto this set.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ToastSeverity {
    /// Neutral information.
    #[default]
    Info,
    /// A completed operation.
    Success,
    /// A degraded but non-fatal outcome.
    Warning,
    /// A failed operation.
    Error,
}

impl ToastSeverity {
    pub const ALL: [Self; 4] = [Self::Info, Self::Success, Self::Warning, Self::Error];

    /// i18n key of the severity's display name (Iced table).
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Info => "toast_level_info",
            Self::Success => "toast_level_success",
            Self::Warning => "toast_level_warning",
            Self::Error => "toast_level_error",
        }
    }

    pub const fn to_index(self) -> usize {
        match self {
            Self::Info => 0,
            Self::Success => 1,
            Self::Warning => 2,
            Self::Error => 3,
        }
    }

    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Success,
            2 => Self::Warning,
            3 => Self::Error,
            _ => Self::Info,
        }
    }
}

/// How long identical toasts are coalesced, and how many may be visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToastPolicy {
    /// Maximum simultaneously visible toasts (oldest evicted first).
    pub max_visible: usize,
    /// Window in which an identical `(severity, text)` toast is coalesced
    /// into the live one instead of stacking a duplicate.
    pub dedup_window_ms: u64,
}

impl Default for ToastPolicy {
    fn default() -> Self {
        Self {
            max_visible: 3,
            dedup_window_ms: 2_000,
        }
    }
}

/// Outcome of offering one toast to the gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastAdmission {
    /// A fresh toast: enqueue it.
    Enqueued,
    /// An identical toast is still live: refresh it, do not stack a copy.
    Coalesced,
}

/// One offer already accepted by the gate.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RecentToast {
    severity: ToastSeverity,
    text: String,
    admitted_at_ms: u64,
}

/// Pure dedup gate shared by both surfaces.
///
/// The gate owns no queue: callers keep their own toast order/capacity and
/// consult the gate *before* pushing, so the coalescing rule is identical on
/// both ends without either surface adopting the other's widget model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToastGate {
    policy: ToastPolicy,
    recent: Vec<RecentToast>,
}

impl ToastGate {
    pub fn new(policy: ToastPolicy) -> Self {
        Self {
            policy: ToastPolicy {
                max_visible: policy.max_visible.max(1),
                dedup_window_ms: policy.dedup_window_ms,
            },
            recent: Vec::new(),
        }
    }

    pub const fn policy(&self) -> ToastPolicy {
        self.policy
    }

    /// Offer a toast at `now_ms` (a monotonic millisecond clock owned by the
    /// surface). Coalesced offers refresh the window of the live toast.
    pub fn admit(&mut self, severity: ToastSeverity, text: &str, now_ms: u64) -> ToastAdmission {
        self.prune(now_ms);
        if let Some(existing) = self
            .recent
            .iter_mut()
            .find(|recent| recent.severity == severity && recent.text == text)
        {
            existing.admitted_at_ms = now_ms;
            return ToastAdmission::Coalesced;
        }
        self.recent.push(RecentToast {
            severity,
            text: text.to_string(),
            admitted_at_ms: now_ms,
        });
        ToastAdmission::Enqueued
    }

    /// How many offers are still inside the dedup window.
    pub fn live_offers(&self, now_ms: u64) -> usize {
        self.recent
            .iter()
            .filter(|recent| is_live(recent.admitted_at_ms, now_ms, self.policy.dedup_window_ms))
            .count()
    }

    /// Forget every offer (e.g. after the user cleared the stack).
    pub fn clear(&mut self) {
        self.recent.clear();
    }

    fn prune(&mut self, now_ms: u64) {
        let window = self.policy.dedup_window_ms;
        self.recent
            .retain(|recent| is_live(recent.admitted_at_ms, now_ms, window));
    }
}

/// An offer is live while its age does not exceed the dedup window (an offer
/// exactly at the window edge is still coalesced).
fn is_live(admitted_at_ms: u64, now_ms: u64, window_ms: u64) -> bool {
    now_ms.saturating_sub(admitted_at_ms) <= window_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_toasts_inside_the_window_are_coalesced() {
        let mut gate = ToastGate::default();
        assert_eq!(
            gate.admit(ToastSeverity::Error, "controller unreachable", 1_000),
            ToastAdmission::Enqueued
        );
        assert_eq!(
            gate.admit(ToastSeverity::Error, "controller unreachable", 1_500),
            ToastAdmission::Coalesced
        );
        assert_eq!(gate.live_offers(1_500), 1);
    }

    #[test]
    fn coalescing_refreshes_the_window() {
        let mut gate = ToastGate::default();
        gate.admit(ToastSeverity::Info, "saved", 0);
        assert_eq!(
            gate.admit(ToastSeverity::Info, "saved", 1_900),
            ToastAdmission::Coalesced
        );
        // The refresh at 1_900 keeps the offer alive past the original expiry.
        assert_eq!(
            gate.admit(ToastSeverity::Info, "saved", 2_500),
            ToastAdmission::Coalesced
        );
    }

    #[test]
    fn distinct_severity_or_text_admits_a_new_toast() {
        let mut gate = ToastGate::default();
        gate.admit(ToastSeverity::Info, "saved", 0);
        assert_eq!(
            gate.admit(ToastSeverity::Warning, "saved", 100),
            ToastAdmission::Enqueued
        );
        assert_eq!(
            gate.admit(ToastSeverity::Info, "saved twice", 100),
            ToastAdmission::Enqueued
        );
        assert_eq!(gate.live_offers(100), 3);
    }

    #[test]
    fn offers_expire_after_the_window() {
        let mut gate = ToastGate::default();
        gate.admit(ToastSeverity::Success, "synced", 0);
        assert_eq!(
            gate.admit(ToastSeverity::Success, "synced", 2_001),
            ToastAdmission::Enqueued
        );
        assert_eq!(gate.live_offers(2_001), 1);
        gate.clear();
        assert_eq!(gate.live_offers(2_001), 0);
    }

    #[test]
    fn policy_clamps_a_zero_capacity() {
        let gate = ToastGate::new(ToastPolicy {
            max_visible: 0,
            dedup_window_ms: 0,
        });
        assert_eq!(gate.policy().max_visible, 1);
        assert_eq!(gate.policy().dedup_window_ms, 0);
    }

    #[test]
    fn severity_indices_round_trip() {
        for severity in ToastSeverity::ALL {
            assert_eq!(ToastSeverity::from_index(severity.to_index()), severity);
        }
    }
}
