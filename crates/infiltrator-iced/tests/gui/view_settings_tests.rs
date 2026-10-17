use super::integration::SettingsChoice;
use super::kernel::kernel_management_card;
use super::*;
use infiltrator_contract::offline_startup::{LocalAssetStatus, OfflineStartupSnapshot};
use infiltrator_presentation::settings_status_projection::format_offline_startup;
use infiltrator_shared::locales::Lang;

#[test]
fn test_settings_choice_display() {
    assert_eq!(format!("{}", SettingsChoice { value: "zh-CN" }), "zh-CN");
    assert_eq!(format!("{}", SettingsChoice { value: "stable" }), "stable");
}
#[test]
fn test_offline_startup_status_is_rendered_without_claiming_geoip_ready() {
    let (mut state, _) = AppState::new();
    state.runtime.offline_startup = OfflineStartupSnapshot::ready(LocalAssetStatus::Missing);
    let status = format_offline_startup(&state.runtime.offline_startup, &Lang("en-US"));
    assert!(status.contains("Offline-first"));
    assert!(status.contains("Offline-ready · degraded"));
    assert!(status.contains("GeoIP=Missing"));
    let _card = kernel_management_card(&state, &Lang("en-US"), false, None);
}
