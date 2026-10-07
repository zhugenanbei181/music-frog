//! Replay shared status copy on fact changes; locale replay belongs to native widgets.
use crate::pages::profiles::{ProfileItem, ProfilesProjectionUpdated};
use crate::pages::profiles_import::selected_profile;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::system::Query;
use infiltrator_application::subscription_status_projection::{
    SubscriptionCopyProjection, backup_status, conditional_request, observed_filter_status,
    schedule_status,
};
use infiltrator_bevy_widgets::localization::LocalizedText;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubscriptionStatusKind {
    #[default]
    Conditional,
    Backup,
    Filter,
    Schedule,
}
pub(super) fn projection(
    kind: SubscriptionStatusKind,
    profile: Option<&ProfileItem>,
) -> SubscriptionCopyProjection {
    if profile.is_none() {
        return SubscriptionCopyProjection::no_profile();
    }
    match kind {
        SubscriptionStatusKind::Conditional => conditional_request(
            profile.and_then(|p| p.etag.as_deref()),
            profile.and_then(|p| p.last_modified.as_deref()),
        ),
        SubscriptionStatusKind::Backup => backup_status(profile.map(|p| p.has_backup)),
        SubscriptionStatusKind::Filter => observed_filter_status(
            profile.map(|p| &p.filter),
            profile.and_then(|p| p.filter_source.as_ref().err()),
        ),
        SubscriptionStatusKind::Schedule => schedule_status(
            profile.map(|p| p.auto_update_enabled),
            profile.and_then(|p| p.cron_expression.as_deref()),
            profile.and_then(|p| p.update_interval_hours),
        ),
    }
}
pub(super) fn sync_status_copy(
    update: On<ProfilesProjectionUpdated>,
    mut lines: Query<(&SubscriptionStatusKind, &mut LocalizedText)>,
) {
    let profile = selected_profile(&update.0);
    for (kind, mut copy) in &mut lines {
        let next = projection(*kind, profile);
        if copy.key != next.key || copy.params != next.params {
            *copy = LocalizedText::new(next.key, next.params);
        }
    }
}
