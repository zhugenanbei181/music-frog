//! Behavior cases for resolve.
//! test-intent: behavior

use super::*;

#[test]
fn resolve_tray_event_in_covers_every_menu_action_and_rejects_unknowns() {
    let profiles = vec![test_profile("Paid", true, false)];
    let ctx = event_ctx(&profiles);

    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SHOW, None), &ctx),
        Some(TrayIntent::ShowWindow)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_QUIT, None), &ctx),
        Some(TrayIntent::Exit)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_TOGGLE_THEME, None), &ctx),
        Some(TrayIntent::ToggleTheme)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_MODE_RULE, None), &ctx),
        Some(TrayIntent::SetMode("rule".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_MODE_GLOBAL, None), &ctx),
        Some(TrayIntent::SetMode("global".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_MODE_DIRECT, None), &ctx),
        Some(TrayIntent::SetMode("direct".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_MODE_SCRIPT, None), &ctx),
        Some(TrayIntent::SetMode("script".to_string()))
    );
    // Toggles resolve against the snapshot's current states.
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_TOGGLE_SYSTEM_PROXY, None), &ctx),
        Some(TrayIntent::SetSystemProxy(false))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_TOGGLE_TUN, None), &ctx),
        Some(TrayIntent::SetTunEnabled(true))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_TOGGLE_AUTOSTART, None), &ctx),
        Some(TrayIntent::SetAutostart(true))
    );
    // Node switching decodes the group␁node payload.
    assert_eq!(
        resolve_tray_event_in(
            &activated(
                TRAY_ACTION_SELECT_PROXY,
                Some(&encode_pair_payload("GLOBAL", "X"))
            ),
            &ctx
        ),
        Some(TrayIntent::SelectProxy {
            group: "GLOBAL".to_string(),
            node: "X".to_string(),
        })
    );
    assert_eq!(
        resolve_tray_event_in(
            &activated(TRAY_ACTION_SELECT_PROXY, Some("missing-separator")),
            &ctx
        ),
        None
    );
    // Legacy GLOBAL quick-switch id stays resolvable (never-reuse contract).
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SELECT_GLOBAL_PROXY, Some("X")), &ctx),
        Some(TrayIntent::SelectGlobalProxy("X".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SELECT_GLOBAL_PROXY, None), &ctx),
        None
    );

    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_ACTIVATE_PROFILE, Some("Paid")), &ctx),
        Some(TrayIntent::ActivateProfile("Paid".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_UPDATE_ALL_PROFILES, None), &ctx),
        Some(TrayIntent::UpdateAllProfilesNow)
    );
    // The auto-update checkmark target flips against the profile's state.
    assert_eq!(
        resolve_tray_event_in(
            &activated(TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, Some("Paid")),
            &ctx
        ),
        Some(TrayIntent::SetProfileAutoUpdate {
            name: "Paid".to_string(),
            enabled: true,
        })
    );
    assert_eq!(
        resolve_tray_event_in(
            &activated(TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, Some("Ghost")),
            &ctx
        ),
        None,
        "stale profile entries resolve to nothing"
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, None), &ctx),
        None
    );

    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SET_DEFAULT_KERNEL, Some("v2")), &ctx),
        Some(TrayIntent::SetDefaultKernel("v2".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_UNINSTALL_KERNEL, Some("v2")), &ctx),
        Some(TrayIntent::UninstallKernel("v2".to_string()))
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_CHECK_CORE_UPDATE, None), &ctx),
        Some(TrayIntent::UpdateCoreToLatest)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_CANCEL_CORE_DOWNLOAD, None), &ctx),
        Some(TrayIntent::CancelCoreDownload)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_FLUSH_FAKEIP, None), &ctx),
        Some(TrayIntent::FlushFakeIp)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SYNC_UPLOAD, None), &ctx),
        Some(TrayIntent::SyncUpload)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_SYNC_DOWNLOAD, None), &ctx),
        Some(TrayIntent::SyncDownload)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_CANCEL_SYNC, None), &ctx),
        Some(TrayIntent::CancelSync)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_NAVIGATE_SYNC, None), &ctx),
        Some(TrayIntent::NavigateSync)
    );
    assert_eq!(
        resolve_tray_event_in(&activated(TRAY_ACTION_FACTORY_RESET, None), &ctx),
        Some(TrayIntent::RequestFactoryReset)
    );

    // Placeholders, informational lines and unknown ids resolve to nothing.
    for id in [
        TRAY_ACTION_NO_PROXIES,
        TRAY_ACTION_NO_PROFILES,
        TRAY_ACTION_INFO_MODE,
        TRAY_ACTION_INFO_STATUS,
        TRAY_ACTION_INFO_CONTROLLER,
        TRAY_ACTION_INFO_ADMIN,
        TRAY_ACTION_INFO_KERNEL_VERSION,
        TRAY_ACTION_INFO_SYNC,
        TRAY_ACTION_INFO_KERNEL_DEFAULT,
        TRAY_ACTION_INFO_KERNEL_STATUS,
        TRAY_ACTION_INFO_DOWNLOAD,
        999,
    ] {
        assert_eq!(
            resolve_tray_event_in(&activated(id, None), &ctx),
            None,
            "id {id} must not resolve"
        );
    }
    // Icon activation shows the window (old left-click behavior).
    assert_eq!(
        resolve_tray_event_in(&TrayEvent::IconActivated, &ctx),
        Some(TrayIntent::ShowWindow)
    );
}
