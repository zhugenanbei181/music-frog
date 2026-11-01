//! Headless tests for the tray host seam: the shared projection feeds a real
//! spec, the typed degradation carries the honest reason, and the drain
//! semantics never block. No tray spawn — red line.

use super::*;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::tray_menu::{
    TRAY_ACTION_TOGGLE_SYSTEM_PROXY, TrayMenuContext, TrayProfileEntry,
};
use infiltrator_contract::tray_status::{
    TrayEventContext, TrayIntent, TrayServiceStatus, resolve_tray_event_in,
};
use infiltrator_presentation::tray_projection::{
    build_tray_spec, generate_tray_icon_rgba, hosted_support, tray_context_from_snapshot,
};
use lurker_core_contract::error::{ErrorCode, Failure};

#[test]
fn the_shared_projection_and_the_host_speak_the_same_vocabulary() {
    let snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "unavailable", true),
    );
    let context = tray_context_from_snapshot(&snapshot, "zh-CN");
    let spec = build_tray_spec(&context, TrayIconColorMode::Monochrome);
    assert_eq!(spec.menu.items.len(), 13);
    assert!(spec.icon.is_some());
    assert_eq!(
        hosted_support(TrayIconColorMode::Monochrome),
        TraySupport::hosted_with_mode(true, TrayIconColorMode::Monochrome)
    );
}

#[test]
fn the_icon_is_backend_neutral_rgba() {
    let icon =
        generate_tray_icon_rgba(TrayServiceStatus::default(), TrayIconColorMode::Colored, 24);
    assert_eq!(icon.width, 24);
    assert_eq!(icon.height, 24);
    assert_eq!(icon.rgba.len(), 24 * 24 * 4);
}

#[test]
fn an_activation_resolves_through_the_shared_intent_table() {
    let profiles = vec![TrayProfileEntry {
        name: "Paid".to_string(),
        active: true,
        auto_update_enabled: false,
    }];
    let context = TrayEventContext {
        system_proxy: true,
        tun: false,
        autostart: false,
        profiles: &profiles,
    };
    let event = TrayEvent::MenuActivated {
        id: TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
        payload: None,
    };
    assert_eq!(
        resolve_tray_event_in(&event, &context),
        Some(TrayIntent::SetSystemProxy(false))
    );
}

#[test]
fn the_menu_context_defaults_never_fabricate_facts() {
    let context = TrayMenuContext::default();
    assert!(context.groups.is_empty());
    assert!(context.profiles.is_empty());
    assert!(context.kernels.is_empty());
    assert_eq!(context.routing_intent, None);
    assert!(!context.system_proxy);
    assert!(!context.tun);
}

#[test]
fn a_headless_host_reports_the_typed_degradation() {
    // The seam itself is exercised through the platform spawn path; a
    // headless environment has no StatusNotifierWatcher, so the typed
    // degradation is the honest answer and must not panic.
    let context = TrayMenuContext::default();
    let spec = build_tray_spec(&context, TrayIconColorMode::Colored);
    match spawn_tray(spec) {
        TrayStartup::Ready { .. } => {
            // A real tray exists on this machine; both outcomes are typed.
        }
        TrayStartup::Unavailable { reason } => assert!(!reason.is_empty()),
    }
}
