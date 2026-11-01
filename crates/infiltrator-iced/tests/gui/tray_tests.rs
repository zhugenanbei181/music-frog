//! Headless Iced tray tests: update-handler transitions through an injected
//! fake event receiver and the AppState -> shared menu context assembly.
//! No D-Bus session, no tray spawn, no network, no mihomo — red lines.
//! The pure menu/event/ksni/muda mappings live with their owners in
//! `infiltrator-presentation` and `infiltrator-desktop`.

use super::*;
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use infiltrator_contract::tray_menu::TRAY_ACTION_TOGGLE_COLOR_MODE;
use infiltrator_contract::tray_menu::{
    TRAY_ACTION_FACTORY_RESET, TRAY_ACTION_INFO_SYNC, TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY,
    TRAY_ACTION_SELECT_GLOBAL_PROXY, TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
    TRAY_ACTION_TOGGLE_AUTOSTART, TRAY_ACTION_TOGGLE_TUN, TrayActionId, TrayMenuItem,
};
use infiltrator_contract::tray_status::TrayIconColorMode;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::{
    details::OutboundSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
};
use lurker_core_contract::page::PageData;
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::mpsc;

/// App state with a fake tray event receiver installed (no controller, i.e.
/// exactly the window-only degradation a failed tray startup produces).
fn state_with_fake_receiver() -> (AppState, mpsc::Sender<TrayEvent>) {
    let (mut state, _) = AppState::new();
    assert!(state.shell.tray_controller.is_none());
    assert!(state.shell.tray_events.is_none());
    let (tx, rx) = mpsc::channel();
    state.shell.tray_events = Some(Arc::new(Mutex::new(rx)));
    (state, tx)
}

fn test_profile(name: &str, active: bool, auto_update: bool) -> ProfileInfo {
    let mut profile = ProfileInfo {
        name: name.to_string(),
        path: format!("/tmp/profiles/{name}.yaml"),
        active,
        ..Default::default()
    };
    profile.auto_update_enabled = auto_update;
    profile
}

fn activated(id: TrayActionId, payload: Option<&str>) -> TrayEvent {
    TrayEvent::MenuActivated {
        id,
        payload: payload.map(str::to_owned),
    }
}

#[test]
fn tray_menu_activation_drives_update_handlers() {
    let (mut state, tx) = state_with_fake_receiver();

    tx.send(activated(TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY, None))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    assert_eq!(events.len(), 1);
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    // Without a runtime the observed routing intent remains unchanged.
    assert_eq!(state.runtime.routing_intent, None);
}

#[test]
fn tray_tun_toggle_resolves_against_current_state() {
    let (mut state, tx) = state_with_fake_receiver();
    state.runtime.tun_enabled = Some(false);

    tx.send(activated(TRAY_ACTION_TOGGLE_TUN, None))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    assert_eq!(state.runtime.tun_enabled, Some(true));
}

#[test]
fn tray_autostart_toggle_flips_state_optimistically() {
    let (mut state, tx) = state_with_fake_receiver();
    state.runtime.autostart_enabled = false;

    tx.send(activated(TRAY_ACTION_TOGGLE_AUTOSTART, None))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    assert!(
        state.runtime.autostart_enabled,
        "click flips the checkmark immediately"
    );
}

#[test]
fn tray_factory_reset_routes_without_panicking() {
    let (mut state, tx) = state_with_fake_receiver();

    tx.send(activated(TRAY_ACTION_FACTORY_RESET, None))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        // The intent resolves to a follow-up RequestConfirmation message; the
        // task is lazy here so the dialog stages when iced runs it.
        let _ = state.update(Message::TrayEvent(event));
    }
    assert!(
        state.shell.confirmation.is_none(),
        "routing only, no direct state"
    );
    assert!(!state.shell.is_factory_resetting);
}

#[test]
fn tray_global_proxy_selection_targets_the_global_group() {
    let (mut state, tx) = state_with_fake_receiver();

    tx.send(activated(TRAY_ACTION_SELECT_GLOBAL_PROXY, Some("Proxy-B")))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    // With no runtime the request is a no-op task, but the handler must have
    // routed it (no panic) and the state stays untouched.
    assert!(state.runtime.runtime.is_none());
}

#[test]
fn tray_color_mode_toggle_flips_the_shared_preference() {
    let (mut state, tx) = state_with_fake_receiver();
    assert_eq!(state.shell.tray_icon_color_mode, TrayIconColorMode::Colored);

    tx.send(activated(TRAY_ACTION_TOGGLE_COLOR_MODE, None))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    assert_eq!(
        state.shell.tray_icon_color_mode,
        TrayIconColorMode::Monochrome,
        "the tray icon preference follows the shared toggle"
    );
    assert_eq!(
        Lang("zh-CN").tr("tray_toggle_color_mode").as_ref(),
        "单色托盘图标"
    );
}

#[test]
fn tray_auto_update_activation_forwards_the_profile_flip() {
    let (mut state, tx) = state_with_fake_receiver();
    state.profile.profiles = vec![test_profile("Paid", true, false)];

    tx.send(activated(TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, Some("Paid")))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        // The follow-up task performs persistence; nothing runs in the test
        // (iced tasks are lazy), so this only proves the routing is safe.
        let _ = state.update(Message::TrayEvent(event));
    }
    assert_eq!(state.profile.profiles.len(), 1);
}

#[test]
fn current_tray_spec_assembles_the_state_domains_into_the_shared_context() {
    let (mut state, _) = AppState::new();
    // Pin the language: AppState::new() detects it from the environment.
    state.shell.lang = "zh-CN".to_string();
    state.runtime.routing_intent = Some(RoutingIntent::FollowRules);
    state.runtime.autostart_enabled = true;
    state.profile.webdav_enabled = true;
    let engine = EngineId::new("mihomo").expect("valid engine");
    let scope = CoreScope {
        identity: CoreIdentity::new(
            engine.clone(),
            CoreInstanceId::new("tray-test").expect("valid instance"),
            Some(CoreVersion::new("1.19.12").expect("valid version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let global = OutboundId::new(engine.clone(), "GLOBAL").expect("valid collection ID");
    let a = OutboundId::new(engine.clone(), "A").expect("valid outbound ID");
    let b = OutboundId::new(engine, "B").expect("valid outbound ID");
    state.runtime.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![
                    OutboundSnapshot {
                        id: a.clone(),
                        display_name: Some("A".into()),
                    },
                    OutboundSnapshot {
                        id: b.clone(),
                        display_name: Some("B".into()),
                    },
                ],
                collections: vec![OutboundSelectionSet {
                    id: global,
                    member_ids: vec![a.clone(), b],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready { selected: a },
                }],
            },
        )
        .expect("valid tray inventory"),
    );

    let spec = state.current_tray_spec();
    let items = &spec.menu.items;

    // Autostart checkmark reflects the runtime domain.
    assert_eq!(
        items[13],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_AUTOSTART, "开机自启", true)
    );
    // The submenu is built from typed inventory; legacy delay facts are not
    // invented when the outbound contract has no probe observation.
    let TrayMenuItem::Submenu {
        items: group_subs, ..
    } = &items[3]
    else {
        panic!("entry 3 must be the proxies submenu");
    };
    let TrayMenuItem::Submenu {
        label,
        items: nodes,
        ..
    } = &group_subs[0]
    else {
        panic!("GLOBAL group must render as a submenu");
    };
    assert_eq!(label, "GLOBAL");
    assert_eq!(nodes[0].action_label(), Some("● A"));
    assert_eq!(nodes[0].action_payload(), Some("GLOBAL\u{1}A"));
    // The sync submenu reflects the profile domain (WebDAV enabled).
    let TrayMenuItem::Submenu {
        items: sync_items, ..
    } = &items[12]
    else {
        panic!("entry 12 must be the sync submenu");
    };
    assert_eq!(
        sync_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "WebDAV 同步已启用")
    );
}

#[test]
fn tray_unavailable_startup_degrades_to_a_functional_window_only_app() {
    // AppState::new() in tests never spawns a tray — the Unavailable path.
    let (mut state, _) = AppState::new();
    assert!(state.shell.tray_controller.is_none());
    assert!(state.shell.tray_events.is_none());

    // The app stays fully functional window-only.
    let _ = state.update(Message::Navigate(Route::Sync));
    assert_eq!(state.shell.current_route, Route::Sync);

    // refresh_tray is a safe no-op without a controller.
    state.refresh_tray();

    // A stray tray event is handled without a tray and without panicking.
    let _ = state.update(Message::TrayEvent(TrayEvent::IconActivated));
}
