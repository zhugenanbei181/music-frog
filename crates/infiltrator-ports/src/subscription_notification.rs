//! DUAL-07-10: outbound port for subscription-lifecycle system notifications.
//!
//! The subscription refresh orchestration lives in the application layer and
//! is shared by both surfaces, so the *decision* to notify also belongs
//! there. The application emits a locale-neutral [`SubscriptionNotification`]
//! describing what happened; each host adapter localizes and dispatches it
//! (the desktop host reuses its existing `SystemNotifier`, a headless host
//! degrades silently). No OS/notification type crosses this boundary.

/// What a subscription refresh cycle did, in locale-neutral terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscriptionNotificationKind {
    /// One or more subscriptions downloaded fresh content.
    Updated,
    /// The server answered `304 Not Modified` for every refreshed profile.
    NotModified,
    /// At least one refresh failed after its retry budget was exhausted.
    Failed,
}

/// Locale-neutral notification payload for a subscription refresh cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionNotification {
    pub kind: SubscriptionNotificationKind,
    /// Profile names the cycle touched, in report order.
    pub profiles: Vec<String>,
    /// Sanitized error summary for [`SubscriptionNotificationKind::Failed`].
    pub error: Option<String>,
}

impl SubscriptionNotification {
    /// Terminal notification for a single-profile refresh.
    pub fn for_profiles(kind: SubscriptionNotificationKind, profiles: Vec<String>) -> Self {
        Self {
            kind,
            profiles,
            error: None,
        }
    }

    /// Failure notification carrying the (already redacted) error summary.
    pub fn failure(profiles: Vec<String>, error: impl Into<String>) -> Self {
        Self {
            kind: SubscriptionNotificationKind::Failed,
            profiles,
            error: Some(error.into()),
        }
    }
}

/// Host adapter that dispatches a subscription notification to the OS.
///
/// Implementations must be non-blocking from the caller's perspective (the
/// desktop adapter spawns its notifier process) and must degrade silently on
/// failure rather than panicking or propagating an error into the refresh
/// result.
pub trait SubscriptionNotificationPort: Send + Sync {
    fn notify(&self, notification: SubscriptionNotification);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingNotifier {
        seen: Arc<Mutex<Vec<SubscriptionNotification>>>,
    }

    impl SubscriptionNotificationPort for RecordingNotifier {
        fn notify(&self, notification: SubscriptionNotification) {
            self.seen.lock().expect("poisoned").push(notification);
        }
    }

    #[test]
    fn failure_notification_carries_redacted_summary() {
        let notifier = RecordingNotifier::default();
        notifier.notify(SubscriptionNotification::failure(
            vec!["main".to_string()],
            "connection reset",
        ));
        let seen = notifier.seen.lock().expect("poisoned").clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].kind, SubscriptionNotificationKind::Failed);
        assert_eq!(seen[0].profiles, vec!["main".to_string()]);
        assert_eq!(seen[0].error.as_deref(), Some("connection reset"));
    }
}
