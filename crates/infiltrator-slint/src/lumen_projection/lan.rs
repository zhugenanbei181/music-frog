//! The LAN sharing card projection of the Slint surface.
//!
//! The LAN sharing configuration is one owner readback: the owner publishes
//! the whole configuration or nothing at all. The shared settings page model
//! folds that readback, so the card shows the confirmed configuration or says
//! that the owner never published one — it never claims that the sharing is
//! off while the owner published nothing.

use crate::AppWindow;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::settings_page_model::{
    LanSharingObservation, SettingsPageModel, observe_settings_page,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// Projects the LAN sharing card from the shared settings page model.
///
/// The card reads the owner's readback through the shared model, so the
/// surface keeps no second copy of the LAN sharing configuration.
pub(super) fn project_lan_sharing(window: &AppWindow, snapshot: &SurfaceSnapshot, language: &str) {
    let lang = Lang(language);
    let mut model = SettingsPageModel::default();
    observe_settings_page(&mut model, snapshot);
    let detail = match model.lan_sharing_observation() {
        LanSharingObservation::Observed(sharing) => format!(
            "{} · {}: {} · {}: {}",
            lang.tr(if sharing.enabled {
                "overview_toggle_enabled"
            } else {
                "overview_toggle_disabled"
            }),
            lang.tr("lan_sharing_port"),
            sharing.mixed_port,
            lang.tr("lan_sharing_bind"),
            sharing.bind_address,
        ),
        LanSharingObservation::Unobserved => lang.tr("shell_readout_unknown").into_owned(),
    };
    window.set_lan_sharing_heading(lang.tr("lan_sharing_title").into_owned().into());
    window.set_lan_sharing_detail(detail.into());
}
