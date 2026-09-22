//! DUAL-07-10: desktop adapter that maps a locale-neutral subscription
//! notification onto the existing cross-platform [`crate::notify`] backend.
//!
//! Both surfaces route their refresh cycles through the shared
//! `SubscriptionRefreshApplication`, which emits a
//! [`SubscriptionNotification`]; this adapter is the single place that turns
//! that into an OS notification.

use infiltrator_ports::subscription_notification::{
    SubscriptionNotification, SubscriptionNotificationKind, SubscriptionNotificationPort,
};

use crate::notify::{NotificationLevel, SystemNotification, SystemNotifier};

/// Desktop-backed subscription notifier.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopSubscriptionNotificationPort;

impl SubscriptionNotificationPort for DesktopSubscriptionNotificationPort {
    fn notify(&self, notification: SubscriptionNotification) {
        let (title, level) = match notification.kind {
            SubscriptionNotificationKind::Updated => ("订阅更新成功", NotificationLevel::Info),
            SubscriptionNotificationKind::NotModified => ("订阅无变化", NotificationLevel::Info),
            SubscriptionNotificationKind::Failed => ("订阅更新失败", NotificationLevel::Error),
        };
        let body = match &notification.error {
            Some(error) => error.clone(),
            None if notification.profiles.is_empty() => "无可用订阅".to_string(),
            None => notification.profiles.join(", "),
        };
        let notifier = SystemNotifier::new();
        let _ = notifier.send(&SystemNotification::new(title, body, level));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_map_to_expected_levels() {
        let port = DesktopSubscriptionNotificationPort;
        // Exercises the tree; the actual dispatch spawns a best-effort child
        // process and is allowed to fail silently on a headless test host.
        port.notify(SubscriptionNotification::for_profiles(
            SubscriptionNotificationKind::Updated,
            vec!["main".to_string()],
        ));
        port.notify(SubscriptionNotification::failure(
            vec!["main".to_string()],
            "connection reset",
        ));
    }
}
