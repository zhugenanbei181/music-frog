//! Headless tray tests: pure spec→menu mapping, event→intent resolution and
//! update-handler transitions through an injected fake event receiver.
//! No D-Bus session, no ksni/muda spawn, no network, no mihomo — red lines.

use super::spec::build_tray_spec;
use super::*;
use crate::state::AppState;
use crate::types::{Message, Route};
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

#[test]
fn spec_layout_mirrors_the_menu_the_native_backend_built() {
    // `None` hides the Web admin entry (feature disabled), reproducing the
    // 10-item menu the native backend used to build imperatively.
    let spec = build_tray_spec(Some("global"), true, false, None, None);
    let items = &spec.menu.items;

    assert_eq!(
        items.len(),
        10,
        "full top-level layout: show, sep, mode, global, sep, sys, tun, theme, sep, quit"
    );
    assert_eq!(
        items[0],
        TrayMenuItem::action(TRAY_ACTION_SHOW, "显示主界面")
    );
    assert_eq!(items[1], TrayMenuItem::Separator);

    match &items[2] {
        TrayMenuItem::Submenu {
            id,
            label,
            items: mode_items,
            ..
        } => {
            assert_eq!(*id, TRAY_SUBMENU_MODE);
            assert_eq!(label, "代理模式");
            assert_eq!(mode_items.len(), 3);
            assert_eq!(mode_items[0].action_id(), Some(TRAY_ACTION_MODE_RULE));
            // Active mode carries the same `● ` marker convention as GLOBAL.
            assert_eq!(mode_items[1].action_label(), Some("● 全局模式"));
        }
        other => panic!("expected mode submenu, got {other:?}"),
    }

    match &items[3] {
        TrayMenuItem::Submenu {
            id,
            label,
            items: global_items,
            ..
        } => {
            assert_eq!(*id, TRAY_SUBMENU_GLOBAL);
            assert_eq!(label, "快速切换 (GLOBAL)");
            // No GLOBAL group yet: the disabled placeholder from before.
            assert_eq!(
                global_items,
                &vec![TrayMenuItem::Action {
                    id: TRAY_ACTION_NO_PROXIES,
                    label: "暂无节点 (请先启动)".to_string(),
                    enabled: false,
                    payload: None,
                }]
            );
        }
        other => panic!("expected GLOBAL submenu, got {other:?}"),
    }

    assert_eq!(items[4], TrayMenuItem::Separator);
    assert_eq!(
        items[5],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_SYSTEM_PROXY, "系统代理", true)
    );
    assert_eq!(
        items[6],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_TUN, "TUN 模式", false)
    );
    assert_eq!(
        items[7],
        TrayMenuItem::action(TRAY_ACTION_TOGGLE_THEME, "切换深/浅色模式")
    );
    assert_eq!(items[8], TrayMenuItem::Separator);
    assert_eq!(items[9], TrayMenuItem::action(TRAY_ACTION_QUIT, "退出应用"));

    // The icon is resolved from the crate's own icons directory.
    let icon = spec.icon.expect("spec embeds the shared RGBA icon");
    assert_eq!(icon.width, icon.height);
    assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    assert!(icon.rgba.as_chunks::<4>().0.iter().any(|px| px[3] != 0));
}

#[test]
fn spec_encodes_global_quick_switch_entries_and_active_marker() {
    let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let spec = build_tray_spec(
        None,
        false,
        false,
        Some(GlobalProxyMenu {
            current: "B",
            nodes: &nodes,
        }),
        None,
    );

    let TrayMenuItem::Submenu { items, .. } = &spec.menu.items[3] else {
        panic!("third top-level entry must be the GLOBAL submenu");
    };
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SELECT_GLOBAL_PROXY,
            label: "A".to_string(),
            enabled: true,
            payload: Some("A".to_string()),
        }
    );
    assert_eq!(items[1].action_label(), Some("● B"));
    assert_eq!(items[1].action_payload(), Some("B"));
}

#[test]
fn resolve_tray_event_covers_every_menu_action_and_rejects_unknowns() {
    let menu = |id, payload: Option<&str>| TrayEvent::MenuActivated {
        id,
        payload: payload.map(str::to_owned),
    };

    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_SHOW, None), false, false),
        Some(TrayIntent::ShowWindow)
    );
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_QUIT, None), false, false),
        Some(TrayIntent::Exit)
    );
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_TOGGLE_THEME, None), false, false),
        Some(TrayIntent::ToggleTheme)
    );
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_MODE_RULE, None), false, false),
        Some(TrayIntent::SetMode("rule".to_string()))
    );
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_MODE_GLOBAL, None), false, false),
        Some(TrayIntent::SetMode("global".to_string()))
    );
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_MODE_DIRECT, None), false, false),
        Some(TrayIntent::SetMode("direct".to_string()))
    );
    // Toggles are resolved against the current state.
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_TOGGLE_SYSTEM_PROXY, None), true, false),
        Some(TrayIntent::SetSystemProxy(false))
    );
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_TOGGLE_TUN, None), false, false),
        Some(TrayIntent::SetTunEnabled(true))
    );
    assert_eq!(
        resolve_tray_event(
            &menu(TRAY_ACTION_SELECT_GLOBAL_PROXY, Some("X")),
            false,
            false
        ),
        Some(TrayIntent::SelectGlobalProxy("X".to_string()))
    );
    // Payload-less proxy entry and unknown ids resolve to nothing.
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_SELECT_GLOBAL_PROXY, None), false, false),
        None
    );
    // The disabled placeholder and unknown ids resolve to nothing.
    assert_eq!(
        resolve_tray_event(&menu(TRAY_ACTION_NO_PROXIES, None), false, false),
        None
    );
    assert_eq!(resolve_tray_event(&menu(999, None), false, false), None);
    // Icon activation shows the window (old left-click behavior).
    assert_eq!(
        resolve_tray_event(&TrayEvent::IconActivated, false, false),
        Some(TrayIntent::ShowWindow)
    );
}

#[test]
fn every_spec_action_id_resolves_to_an_intent() {
    // The Web admin entry rides along (server running) so every fixed action
    // is exercised; no GLOBAL group on purpose: the spec then contains the
    // disabled placeholder in addition to every fixed action.
    let spec = build_tray_spec(
        Some("rule"),
        false,
        false,
        None,
        Some(WebAdminMenu { running: true }),
    );

    fn walk(items: &[TrayMenuItem], hits: &mut usize, misses: &mut usize) {
        for item in items {
            match item {
                TrayMenuItem::Action { id, payload, .. } => {
                    let resolved = resolve_tray_event(
                        &TrayEvent::MenuActivated {
                            id: *id,
                            payload: payload.clone(),
                        },
                        false,
                        false,
                    );
                    if *id == TRAY_ACTION_NO_PROXIES {
                        assert!(resolved.is_none(), "placeholder must not resolve");
                        *misses += 1;
                    } else {
                        assert!(resolved.is_some(), "action {id} must resolve");
                        *hits += 1;
                    }
                }
                TrayMenuItem::Checkmark { id, .. } => {
                    assert!(
                        resolve_tray_event(
                            &TrayEvent::MenuActivated {
                                id: *id,
                                payload: None
                            },
                            false,
                            false,
                        )
                        .is_some(),
                        "checkmark {id} must resolve"
                    );
                    *hits += 1;
                }
                TrayMenuItem::Submenu { items, .. } => walk(items, hits, misses),
                TrayMenuItem::Separator => {}
            }
        }
    }

    let (mut hits, mut misses) = (0, 0);
    walk(&spec.menu.items, &mut hits, &mut misses);
    // 9 fixed actions (show, web admin, rule/global/direct, sys-proxy, tun,
    // theme, quit) plus the single placeholder.
    assert_eq!(hits, 9);
    assert_eq!(misses, 1);
}

#[test]
fn web_admin_entry_is_rendered_exactly_when_the_feature_is_enabled() {
    // Disabled feature: no entry at all.
    let spec = build_tray_spec(None, false, false, None, None);
    assert_eq!(spec.menu.items.len(), 10);
    assert!(
        spec.menu
            .items
            .iter()
            .all(|item| item.action_id() != Some(TRAY_ACTION_OPEN_WEB_ADMIN))
    );

    // Enabled + running: second entry, right under 显示主界面, clickable.
    let spec = build_tray_spec(
        None,
        false,
        false,
        None,
        Some(WebAdminMenu { running: true }),
    );
    assert_eq!(spec.menu.items.len(), 11);
    assert_eq!(
        spec.menu.items[1],
        TrayMenuItem::Action {
            id: TRAY_ACTION_OPEN_WEB_ADMIN,
            label: "打开 Web 管理端".to_string(),
            enabled: true,
            payload: None,
        }
    );
    assert_eq!(
        resolve_tray_event(
            &TrayEvent::MenuActivated {
                id: TRAY_ACTION_OPEN_WEB_ADMIN,
                payload: None,
            },
            false,
            false
        ),
        Some(TrayIntent::OpenWebAdmin)
    );

    // Enabled but not started yet: visible but greyed out.
    let spec = build_tray_spec(
        None,
        false,
        false,
        None,
        Some(WebAdminMenu { running: false }),
    );
    assert_eq!(spec.menu.items.len(), 11);
    assert_eq!(
        spec.menu.items[1].action_id(),
        Some(TRAY_ACTION_OPEN_WEB_ADMIN)
    );
    match &spec.menu.items[1] {
        TrayMenuItem::Action { enabled, .. } => assert!(!enabled),
        other => panic!("expected an action entry, got {other:?}"),
    }
}

#[test]
fn tray_menu_activation_drives_update_handlers() {
    let (mut state, tx) = state_with_fake_receiver();

    tx.send(TrayEvent::MenuActivated {
        id: TRAY_ACTION_MODE_GLOBAL,
        payload: None,
    })
    .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    assert_eq!(events.len(), 1);
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("global"));
}

#[test]
fn tray_tun_toggle_resolves_against_current_state() {
    let (mut state, tx) = state_with_fake_receiver();
    state.runtime.tun_enabled = Some(false);

    tx.send(TrayEvent::MenuActivated {
        id: TRAY_ACTION_TOGGLE_TUN,
        payload: None,
    })
    .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    assert_eq!(state.runtime.tun_enabled, Some(true));
}

#[test]
fn tray_global_proxy_selection_targets_the_global_group() {
    let (mut state, tx) = state_with_fake_receiver();

    tx.send(TrayEvent::MenuActivated {
        id: TRAY_ACTION_SELECT_GLOBAL_PROXY,
        payload: Some("Proxy-B".to_string()),
    })
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
fn tray_unavailable_startup_degrades_to_a_functional_window_only_app() {
    // AppState::new() in tests never spawns a tray — the Unavailable path.
    let (mut state, _) = AppState::new();
    assert!(state.shell.tray_controller.is_none());
    assert!(state.shell.tray_events.is_none());

    // The app stays fully functional window-only.
    let _ = state.update(Message::Navigate(Route::Settings));
    assert_eq!(state.shell.current_route, Route::Settings);

    // refresh_tray is a safe no-op without a controller.
    state.refresh_tray();

    // A stray tray event is handled without a tray and without panicking.
    let _ = state.update(Message::TrayEvent(TrayEvent::IconActivated));
}

// The spec→ksni menu mapping only exists in the ksni backend configuration.
#[cfg(all(unix, not(target_os = "macos"), not(feature = "native-tray-backend")))]
mod ksni_mapping {
    use super::*;
    use crate::tray::ksni_backend::{KsniTray, map_items, to_ksni_icon};
    use ksni::MenuItem;
    use ksni::Tray as _;
    use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
    use std::collections::HashMap;

    fn sample_spec() -> (TraySpec, Vec<String>) {
        let nodes = vec!["A".to_string(), "B".to_string()];
        let spec = build_tray_spec(
            Some("direct"),
            false,
            true,
            Some(GlobalProxyMenu {
                current: "A",
                nodes: &nodes,
            }),
            Some(WebAdminMenu { running: true }),
        );
        (spec, nodes)
    }

    #[test]
    fn ksni_menu_mapping_preserves_spec_structure_and_state() {
        let (spec, _nodes) = sample_spec();
        let (tx, _rx) = mpsc::channel::<TrayEvent>();
        let menu = map_items(&spec.menu.items, &HashMap::new(), &tx);

        assert_eq!(menu.len(), 11);
        match &menu[0] {
            MenuItem::Standard(StandardItem { label, .. }) => {
                assert_eq!(label, "显示主界面")
            }
            _ => panic!("entry 0 must be a standard item"),
        }
        match &menu[1] {
            MenuItem::Standard(StandardItem { label, enabled, .. }) => {
                assert_eq!(label, "打开 Web 管理端");
                assert!(*enabled);
            }
            _ => panic!("entry 1 must be the Web admin standard item"),
        }
        assert!(matches!(&menu[2], MenuItem::Separator));

        match &menu[3] {
            MenuItem::SubMenu(SubMenu { label, submenu, .. }) => {
                assert_eq!(label, "代理模式");
                assert_eq!(submenu.len(), 3);
                match &submenu[2] {
                    MenuItem::Standard(StandardItem { label, .. }) => {
                        assert_eq!(label, "● 直连模式")
                    }
                    _ => panic!("active mode must be a standard item"),
                }
            }
            _ => panic!("entry 3 must be a submenu"),
        }

        match &menu[4] {
            MenuItem::SubMenu(SubMenu { submenu, .. }) => {
                assert_eq!(submenu.len(), 2);
                match &submenu[0] {
                    MenuItem::Standard(StandardItem { label, .. }) => {
                        assert_eq!(label, "● A")
                    }
                    _ => panic!("proxy entry must be a standard item"),
                }
            }
            _ => panic!("entry 4 must be a submenu"),
        }

        match &menu[6] {
            MenuItem::Checkmark(CheckmarkItem { checked, .. }) => assert!(!checked),
            _ => panic!("entry 6 must be a checkmark"),
        }
        match &menu[7] {
            MenuItem::Checkmark(CheckmarkItem { checked, .. }) => assert!(checked),
            _ => panic!("entry 7 must be a checkmark"),
        }
        assert!(matches!(&menu[10], MenuItem::Standard(_)));
    }

    #[test]
    fn ksni_checkmark_activation_flips_local_state_and_emits_event() {
        let (spec, _nodes) = sample_spec();
        let (tx, rx) = mpsc::channel::<TrayEvent>();
        let mut tray = KsniTray {
            spec,
            checked_overrides: HashMap::new(),
            events: tx,
        };

        let menu = tray.menu();
        let MenuItem::Checkmark(item) = &menu[7] else {
            panic!("entry 7 must be the TUN checkmark");
        };
        assert!(item.checked);
        // Simulate the user clicking the TUN entry in the shell menu.
        (item.activate)(&mut tray);
        assert_eq!(
            rx.try_recv(),
            Ok(TrayEvent::MenuActivated {
                id: TRAY_ACTION_TOGGLE_TUN,
                payload: None,
            })
        );

        // The flip is visible immediately, before the app pushes a new spec.
        let menu = tray.menu();
        let MenuItem::Checkmark(item) = &menu[7] else {
            panic!("entry 7 must still be the TUN checkmark");
        };
        assert!(!item.checked);
    }

    #[test]
    fn ksni_icon_conversion_maps_rgba_to_argb32_network_byte_order() {
        let icon = TrayIconData {
            width: 2,
            height: 1,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let converted = to_ksni_icon(&icon);
        assert_eq!(converted.width, 2);
        assert_eq!(converted.height, 1);
        assert_eq!(converted.data, vec![4, 1, 2, 3, 8, 5, 6, 7]);
    }
}
