//! The business panels of the filters workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the filters page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let page = snapshot.pages.profiles.data.as_ref();
    let profiles = page.map(|page| page.profiles.len()).unwrap_or(0);
    let active = page
        .and_then(|page| page.profiles.iter().find(|profile| profile.is_active))
        .map(|profile| profile.name.clone())
        .unwrap_or_default();
    let quota = &snapshot.subscription_quota;
    let quota_status = format!("{:?}", quota.status);
    let quota_used = quota
        .usage_percent
        .map(|percent| format!("{percent:.0}%"))
        .unwrap_or_default();

    let failure = super::page_failure(&snapshot.pages.profiles.status);
    let mut panel = Panel::new(
        BusinessId::ProfilesSubscriptionError,
        catalog.text("profiles_subscription_settings_title"),
        active,
        vec![
            catalog.text("profiles_update_now").into_owned(),
            catalog.text("subscription_reload_unknown").into_owned(),
            format!("{profiles}"),
            quota_status,
            quota_used,
        ],
    );
    panel.failure = failure;
    panel.empty = super::page_empty(&snapshot.pages.profiles.status);
    vec![panel]
}
