//! Shared subscription quota dashboard read model.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionQuotaStatus {
    #[default]
    Unknown,
    Ready,
    Empty,
    Warning,
    Critical,
    Exhausted,
    Expired,
    ExpiringSoon,
    Unsupported,
    Failed,
}

/// Shared active-profile quota facts. Missing provider metadata stays
/// optional; consumers must not replace it with a believable fake quota.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionQuotaSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: SubscriptionQuotaStatus,
    pub failure: Option<String>,
    pub profile_name: Option<String>,
    pub used_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub remaining_bytes: Option<u64>,
    pub usage_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub expire_at_epoch_secs: Option<i64>,
    pub expires_at_label: Option<String>,
    pub remaining_days: Option<i64>,
    pub next_update_at_epoch_secs: Option<i64>,
    pub next_update_label: Option<String>,
    /// Reserved for a provider that reports a distinct billing reset. None
    /// is rendered as “not reported”, never inferred from expiry.
    pub reset_at_epoch_secs: Option<i64>,
    pub reset_days: Option<i64>,
}

impl SubscriptionQuotaSnapshot {
    /// Explicit deterministic fixture used only by demo/screenshot hosts.
    pub fn demo_fixture() -> Self {
        Self {
            generation: 1,
            revision: 1,
            status: SubscriptionQuotaStatus::Ready,
            failure: None,
            profile_name: Some("主力高速订阅".to_owned()),
            used_bytes: Some(46_430_000_000),
            total_bytes: Some(186_260_000_000),
            remaining_bytes: Some(139_830_000_000),
            usage_percent: Some(24.9),
            remaining_percent: Some(75.1),
            expire_at_epoch_secs: None,
            expires_at_label: Some("2026-10-01".to_owned()),
            remaining_days: Some(25),
            next_update_at_epoch_secs: None,
            next_update_label: Some("next update · 24h".to_owned()),
            reset_at_epoch_secs: None,
            reset_days: None,
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: SubscriptionQuotaStatus::Unsupported,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: SubscriptionQuotaStatus::Failed,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn is_drawable(&self) -> bool {
        matches!(
            self.status,
            SubscriptionQuotaStatus::Ready
                | SubscriptionQuotaStatus::Warning
                | SubscriptionQuotaStatus::Critical
                | SubscriptionQuotaStatus::Exhausted
                | SubscriptionQuotaStatus::Expired
                | SubscriptionQuotaStatus::ExpiringSoon
        ) && self.profile_name.is_some()
    }

    pub fn usage_fraction(&self) -> f32 {
        self.usage_percent
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| (value / 100.0).clamp(0.0, 1.0) as f32)
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_exposes_dashboard_quota_without_inferred_reset() {
        let snapshot = SubscriptionQuotaSnapshot::demo_fixture();
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.usage_fraction(), 0.249);
        assert!(snapshot.reset_days.is_none());
    }

    #[test]
    fn unsupported_quota_is_not_drawable() {
        let snapshot = SubscriptionQuotaSnapshot::unsupported(2, 3, "profile reader missing");
        assert!(!snapshot.is_drawable());
        assert_eq!(snapshot.failure.as_deref(), Some("profile reader missing"));
    }
}
