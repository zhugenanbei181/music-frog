//! Pure subscription-quota dashboard derivation.

use crate::profiles::ProfileInfo;
use crate::subscription::{QuotaStatus, SubscriptionUserInfo};
use infiltrator_contract::subscription_quota::{
    SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
};

/// Project the active local profile's provider metadata into the shared
/// dashboard. now_unix is injected so status tests are deterministic.
pub fn derive(
    generation: u64,
    revision: u64,
    profiles: &[ProfileInfo],
    now_unix: i64,
) -> SubscriptionQuotaSnapshot {
    let Some(profile) = profiles.iter().find(|profile| profile.active) else {
        return SubscriptionQuotaSnapshot {
            generation,
            revision,
            status: SubscriptionQuotaStatus::Empty,
            ..Default::default()
        };
    };

    let info = SubscriptionUserInfo {
        upload: profile.traffic_upload,
        download: profile.traffic_download,
        total: profile.traffic_total,
        expire: profile.expire_at,
    };
    let has_metadata = info.upload.is_some()
        || info.download.is_some()
        || info.total.is_some()
        || info.expire.is_some();
    if !has_metadata {
        return SubscriptionQuotaSnapshot {
            generation,
            revision,
            status: SubscriptionQuotaStatus::Empty,
            profile_name: Some(profile.name.clone()),
            next_update_at_epoch_secs: profile.next_update.map(|value| value.timestamp()),
            next_update_label: profile
                .next_update
                .map(|value| value.format("%Y-%m-%d %H:%M UTC").to_string()),
            ..Default::default()
        };
    }

    let used_bytes = info
        .upload
        .zip(info.download)
        .and_then(|(upload, download)| upload.checked_add(download));
    if info.upload.zip(info.download).is_some() && used_bytes.is_none() {
        return SubscriptionQuotaSnapshot {
            profile_name: Some(profile.name.clone()),
            total_bytes: info.total,
            ..SubscriptionQuotaSnapshot::failed(
                generation,
                revision,
                "Reported traffic exceeds the supported byte range",
            )
        };
    }
    let total_bytes = info.total;
    let remaining_bytes = used_bytes
        .zip(total_bytes)
        .map(|(used, total)| total.saturating_sub(used));
    let usage_percent = used_bytes.and_then(|_| info.usage_percentage());
    let remaining_percent = usage_percent.map(|value| (100.0 - value).clamp(0.0, 100.0));
    let quota_status = info.status(now_unix);
    let (status, remaining_days) = match quota_status {
        QuotaStatus::Normal => (
            SubscriptionQuotaStatus::Ready,
            info.remaining_days(now_unix),
        ),
        QuotaStatus::NearExhaustion => (
            SubscriptionQuotaStatus::Warning,
            info.remaining_days(now_unix),
        ),
        QuotaStatus::Critical => (
            SubscriptionQuotaStatus::Critical,
            info.remaining_days(now_unix),
        ),
        QuotaStatus::Exhausted => (
            SubscriptionQuotaStatus::Exhausted,
            info.remaining_days(now_unix),
        ),
        QuotaStatus::Expired => (SubscriptionQuotaStatus::Expired, Some(0)),
        QuotaStatus::ExpiringSoon { days_left } => {
            (SubscriptionQuotaStatus::ExpiringSoon, Some(days_left))
        }
    };

    SubscriptionQuotaSnapshot {
        generation,
        revision,
        status,
        failure: None,
        source: None,
        retained: false,
        profile_name: Some(profile.name.clone()),
        used_bytes,
        total_bytes,
        remaining_bytes,
        usage_percent,
        remaining_percent,
        expire_at_epoch_secs: info.expire,
        expires_at_label: info
            .expire
            .and_then(|value| chrono::DateTime::from_timestamp(value, 0))
            .map(|value| value.format("%Y-%m-%d").to_string()),
        remaining_days,
        next_update_at_epoch_secs: profile.next_update.map(|value| value.timestamp()),
        next_update_label: profile
            .next_update
            .map(|value| value.format("%Y-%m-%d %H:%M UTC").to_string()),
        // The current provider metadata has no distinct reset timestamp.
        // Keep this absent rather than treating expiry as a billing reset.
        reset_at_epoch_secs: None,
        reset_days: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn profile(active: bool) -> ProfileInfo {
        ProfileInfo {
            name: "active".to_owned(),
            active,
            traffic_upload: Some(10),
            traffic_download: Some(20),
            traffic_total: Some(100),
            expire_at: Some(1_700_438_400),
            next_update: Some(Utc.timestamp_opt(1_700_000_000, 0).single().unwrap()),
            ..Default::default()
        }
    }

    #[test]
    fn derives_usage_remaining_expiry_and_warning_state() {
        let snapshot = derive(3, 4, &[profile(true)], 1_700_000_000);
        assert_eq!(snapshot.status, SubscriptionQuotaStatus::Ready);
        assert_eq!(snapshot.used_bytes, Some(30));
        assert_eq!(snapshot.remaining_bytes, Some(70));
        assert_eq!(snapshot.usage_percent, Some(30.0));
        assert_eq!(snapshot.remaining_percent, Some(70.0));
        assert_eq!(snapshot.expires_at_label.as_deref(), Some("2023-11-20"));
        assert_eq!(snapshot.reset_days, None);
    }

    #[test]
    fn exhausted_and_expired_are_distinct_typed_states() {
        let mut exhausted = profile(true);
        exhausted.traffic_upload = Some(100);
        exhausted.traffic_download = Some(0);
        let snapshot = derive(1, 1, &[exhausted], 1_700_000_000);
        assert_eq!(snapshot.status, SubscriptionQuotaStatus::Exhausted);

        let mut expired = profile(true);
        expired.expire_at = Some(1_600_000_000);
        let snapshot = derive(1, 1, &[expired], 1_700_000_000);
        assert_eq!(snapshot.status, SubscriptionQuotaStatus::Expired);
        assert_eq!(snapshot.remaining_days, Some(0));
    }

    #[test]
    fn overflowing_traffic_is_failed_and_never_reported_as_a_precise_saturated_usage() {
        let mut observed = profile(true);
        observed.traffic_upload = Some(u64::MAX);
        observed.traffic_download = Some(1);
        let result = derive(1, 2, &[observed], 1_700_000_000);
        assert_eq!(result.status, SubscriptionQuotaStatus::Failed);
        assert_eq!(result.used_bytes, None);
        assert_eq!(result.usage_percent, None);
        assert_eq!(result.total_bytes, Some(100));
        assert!(result.failure.unwrap().message.contains("byte range"));
    }

    #[test]
    fn no_active_profile_is_empty_not_a_fabricated_quota() {
        let snapshot = derive(1, 1, &[profile(false)], 1_700_000_000);
        assert_eq!(snapshot.status, SubscriptionQuotaStatus::Empty);
        assert!(snapshot.used_bytes.is_none());
        assert!(snapshot.total_bytes.is_none());
    }

    #[test]
    fn a_missing_transfer_direction_cannot_be_reported_as_zero_usage() {
        let mut partial = profile(true);
        partial.traffic_download = None;
        let snapshot = derive(1, 1, &[partial], 1_700_000_000);
        assert_eq!(snapshot.total_bytes, Some(100));
        assert!(snapshot.used_bytes.is_none());
        assert!(snapshot.remaining_bytes.is_none());
        assert!(snapshot.usage_percent.is_none());
    }
}
