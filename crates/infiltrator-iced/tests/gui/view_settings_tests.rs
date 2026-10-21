use super::integration::SettingsChoice;
use super::kernel::kernel_management_card;
use super::*;
use crate::types::app::ConfirmAction;
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

#[test]
fn test_the_core_rollback_is_armed_behind_the_shared_confirmation() {
    let (mut state, _) = AppState::new();
    state.runtime.core_versions.rollback.target = Some("v1.18.8".to_string());

    // The destructive rollback is armed, never dispatched: the shared
    // confirmation surface owns the decision.
    let _ = state.update(Message::RequestConfirmation(ConfirmAction::RollbackCore(
        "v1.18.8".to_string(),
    )));
    assert!(matches!(
        state.shell.confirmation,
        Some(ConfirmAction::RollbackCore(_))
    ));

    // Cancelling drops the armed action and never touches the engine.
    let _ = state.update(Message::CancelConfirmation);
    assert!(state.shell.confirmation.is_none());

    // The confirm step dispatches the rollback command. Only the task is
    // scheduled here, so the test never rewrites the installed engine.
    let _ = state.update(Message::RequestConfirmation(ConfirmAction::RollbackCore(
        "v1.18.8".to_string(),
    )));
    let task = state.update(Message::ConfirmAction);
    assert!(
        task.units() > 0,
        "the confirm step dispatches the rollback command"
    );
    assert!(state.shell.confirmation.is_none());
}
