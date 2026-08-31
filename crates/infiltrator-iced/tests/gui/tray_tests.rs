//! Headless tray tests: pure spec→menu mapping, event→intent resolution and
//! update-handler transitions through an injected fake event receiver.
//! No D-Bus session, no ksni/muda spawn, no network, no mihomo — red lines.
//!
//! The pure builder/resolver tests live in the nested [`spec_menu`] module
//! (`tray_spec_menu_tests.rs`); this file keeps the AppState-level routing
//! tests and the ksni mapping tests.

use super::spec::{
    TrayEventContext, TrayMenuItem,
    TRAY_ACTION_FACTORY_RESET, TRAY_ACTION_INFO_SYNC, TRAY_ACTION_MODE_GLOBAL,
    TRAY_ACTION_SELECT_GLOBAL_PROXY, TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
    TRAY_ACTION_TOGGLE_AUTOSTART, TRAY_ACTION_TOGGLE_TUN,
};
use super::*;
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use mihomo_api::proxy::types::{Proxy, ProxyBase, ProxyGroup, ProxyHistory, Shadowsocks};
use mihomo_config::profile::Profile;
use mihomo_version::manager::VersionInfo;
use std::path::PathBuf;
use std::sync::mpsc;

// Pure spec-side coverage (layout, labels, codecs, resolution).
#[path = "tray_spec_menu_tests.rs"]
mod spec_menu;

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

/// Minimal snapshot: empty data domains, script mode available, core stopped.
fn base_ctx<'a>() -> TraySpecContext<'a> {
    TraySpecContext {
        lang: "zh-CN",
        mode: Some("rule"),
        script_block_present: true,
        system_proxy: false,
        tun: false,
        groups: &[],
        profiles: &[],
        kernels: &[],
        status: TrayCoreStatus::Stopped,
        core_checking: false,
        core_downloading: false,
        core_download_percent: None,
        webdav_enabled: false,
        syncing: false,
        sync_step: None,
        autostart: false,
        controller: Some("http://127.0.0.1:9090"),
        admin_enabled: false,
        admin_port: 25210,
    }
}

fn proxy_group(name: &str, current: &str, nodes: &[(&str, Option<u32>)]) -> TrayProxyGroup {
    TrayProxyGroup {
        name: name.to_string(),
        current: current.to_string(),
        nodes: nodes
            .iter()
            .map(|(node, delay)| TrayProxyNode {
                name: (*node).to_string(),
                delay_ms: *delay,
            })
            .collect(),
    }
}

fn test_profile(name: &str, active: bool, auto_update: bool) -> Profile {
    let mut profile = Profile::new(
        name.to_string(),
        PathBuf::from(format!("/tmp/profiles/{name}.yaml")),
        active,
    );
    profile.auto_update_enabled = auto_update;
    profile
}

fn test_kernel(version: &str, is_default: bool) -> VersionInfo {
    VersionInfo {
        version: version.to_string(),
        path: PathBuf::from(format!("/tmp/kernels/{version}")),
        is_default,
    }
}

fn event_ctx<'a>(profiles: &'a [Profile]) -> TrayEventContext<'a> {
    TrayEventContext {
        system_proxy: true,
        tun: false,
        autostart: false,
        profiles,
    }
}

fn activated(id: crate::tray::spec::TrayActionId, payload: Option<&str>) -> TrayEvent {
    TrayEvent::MenuActivated {
        id,
        payload: payload.map(str::to_owned),
    }
}

#[test]
fn tray_menu_activation_drives_update_handlers() {
    let (mut state, tx) = state_with_fake_receiver();

    tx.send(activated(TRAY_ACTION_MODE_GLOBAL, None))
        .expect("fake receiver is live");
    let events = drain_tray_events(state.shell.tray_events.as_ref().expect("injected"));
    assert_eq!(events.len(), 1);
    for event in events {
        let _ = state.update(Message::TrayEvent(event));
    }
    // Without a runtime the mode is NOT flipped optimistically — the tray
    // keeps reflecting reality while SetProxyMode reports "unavailable".
    assert_eq!(state.runtime.proxy_mode.as_deref(), None);
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
    assert!(state.runtime.autostart_enabled, "click flips the checkmark immediately");
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
    assert!(state.shell.confirmation.is_none(), "routing only, no direct state");
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
fn current_tray_spec_assembles_the_five_state_domains() {
    let (mut state, _) = AppState::new();
    // Pin the language: AppState::new() detects it from the environment.
    state.shell.lang = "zh-CN".to_string();
    state.runtime.proxy_mode = Some("rule".to_string());
    state.runtime.autostart_enabled = true;
    state.profile.webdav_enabled = true;
    state.runtime.proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "A".to_string(),
            all: vec!["A".to_string(), "B".to_string()],
            history: Vec::new(),
        }),
    );
    state.runtime.proxies.insert(
        "A".to_string(),
        Proxy::Shadowsocks(Shadowsocks {
            base: ProxyBase {
                name: "A".to_string(),
                history: vec![ProxyHistory {
                    time: String::new(),
                    delay: 123,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    let spec = state.current_tray_spec();
    let items = &spec.menu.items;

    // Autostart checkmark reflects the runtime domain.
    assert_eq!(
        items[12],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_AUTOSTART, "开机自启", true)
    );
    // 节点切换 picks up the GLOBAL group with its active node and delay tag.
    let TrayMenuItem::Submenu { items: group_subs, .. } = &items[3] else {
        panic!("entry 3 must be the proxies submenu");
    };
    let TrayMenuItem::Submenu { label, items: nodes, .. } = &group_subs[0] else {
        panic!("GLOBAL group must render as a submenu");
    };
    assert_eq!(label, "GLOBAL");
    assert_eq!(nodes[0].action_label(), Some("● A (123 ms)"));
    assert_eq!(nodes[0].action_payload(), Some("GLOBAL\u{1}A"));
    // The sync submenu reflects the profile domain (WebDAV enabled).
    let TrayMenuItem::Submenu { items: sync_items, .. } = &items[11] else {
        panic!("entry 11 must be the sync submenu");
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

// The spec→ksni menu mapping only exists in the ksni backend configuration.
#[cfg(all(unix, not(target_os = "macos"), not(feature = "native-tray-backend")))]
mod ksni_mapping {
    use super::*;
    use crate::tray::ksni_backend::{KsniTray, map_items, to_ksni_icon};
    use crate::tray::spec::{TrayIconData, TRAY_ACTION_TOGGLE_TUN};
    use ksni::MenuItem;
    use ksni::Tray as _;
    use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
    use std::collections::HashMap;

    fn sample_tray(events: std::sync::mpsc::Sender<TrayEvent>) -> KsniTray {
        let groups = vec![proxy_group("GLOBAL", "A", &[("A", None), ("B", Some(64))])];
        let mut ctx = base_ctx();
        ctx.mode = Some("direct");
        ctx.tun = true;
        ctx.groups = &groups;
        let spec = build_tray_spec(&ctx);
        KsniTray {
            spec,
            checked_overrides: HashMap::new(),
            events,
        }
    }

    #[test]
    fn ksni_menu_mapping_preserves_spec_structure_and_state() {
        let (tx, _rx) = mpsc::channel::<TrayEvent>();
        let tray = sample_tray(tx);
        let menu = map_items(&tray.spec.menu.items, &tray.checked_overrides, &tray.events);

        assert_eq!(menu.len(), 18);
        match &menu[0] {
            MenuItem::Standard(StandardItem { label, .. }) => {
                assert_eq!(label, "显示主界面")
            }
            _ => panic!("entry 0 must be a standard item"),
        }
        assert!(matches!(&menu[1], MenuItem::Separator));

        match &menu[2] {
            MenuItem::SubMenu(SubMenu { label, submenu, .. }) => {
                assert_eq!(label, "代理模式");
                assert_eq!(submenu.len(), 4);
                match &submenu[2] {
                    MenuItem::Standard(StandardItem { label, .. }) => {
                        assert_eq!(label, "● 直连模式")
                    }
                    _ => panic!("active mode must be a standard item"),
                }
            }
            _ => panic!("entry 2 must be a submenu"),
        }

        match &menu[3] {
            MenuItem::SubMenu(SubMenu { label, submenu, .. }) => {
                assert_eq!(label, "节点切换");
                assert_eq!(submenu.len(), 1, "one group submenu");
                match &submenu[0] {
                    MenuItem::SubMenu(SubMenu { label, submenu, .. }) => {
                        assert_eq!(label, "GLOBAL");
                        assert_eq!(submenu.len(), 2);
                        match &submenu[0] {
                            MenuItem::Standard(StandardItem { label, .. }) => {
                                assert_eq!(label, "● A")
                            }
                            _ => panic!("proxy entry must be a standard item"),
                        }
                    }
                    _ => panic!("group entry must be a nested submenu"),
                }
            }
            _ => panic!("entry 3 must be a submenu"),
        }

        match &menu[5] {
            MenuItem::Checkmark(CheckmarkItem { checked, .. }) => assert!(!checked),
            _ => panic!("entry 5 must be a checkmark"),
        }
        match &menu[6] {
            MenuItem::Checkmark(CheckmarkItem { checked, .. }) => assert!(checked),
            _ => panic!("entry 6 must be a checkmark"),
        }
        match &menu[14] {
            MenuItem::SubMenu(SubMenu { label, .. }) => assert_eq!(label, "信息"),
            _ => panic!("entry 14 must be the info submenu"),
        }
        assert!(matches!(&menu[17], MenuItem::Standard(_)));
    }

    #[test]
    fn ksni_checkmark_activation_flips_local_state_and_emits_event() {
        let (tx, rx) = mpsc::channel::<TrayEvent>();
        let mut tray = sample_tray(tx);

        let menu = tray.menu();
        let MenuItem::Checkmark(item) = &menu[6] else {
            panic!("entry 6 must be the TUN checkmark");
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
        let MenuItem::Checkmark(item) = &menu[6] else {
            panic!("entry 6 must still be the TUN checkmark");
        };
        assert!(!item.checked);
    }

    #[test]
    fn ksni_checkmark_overrides_are_keyed_per_payload() {
        let profiles = vec![test_profile("Paid", true, true), test_profile("Free", false, false)];
        let mut ctx = base_ctx();
        ctx.profiles = &profiles;
        let spec = build_tray_spec(&ctx);
        let (tx, rx) = mpsc::channel::<TrayEvent>();
        let mut tray = KsniTray {
            spec,
            checked_overrides: HashMap::new(),
            events: tx,
        };

        // Profiles submenu: [P, P, sep, update-all, sep, check Paid, check Free].
        let menu = tray.menu();
        let MenuItem::SubMenu(SubMenu { submenu: profile_items, .. }) = &menu[9] else {
            panic!("entry 9 must be the profiles submenu");
        };
        let MenuItem::Checkmark(paid) = &profile_items[5] else {
            panic!("entry 5 must be the Paid auto-update checkmark");
        };
        assert!(paid.checked);
        let MenuItem::Checkmark(free) = &profile_items[6] else {
            panic!("entry 6 must be the Free auto-update checkmark");
        };
        assert!(!free.checked);

        // Click the Paid entry: its own state flips, the sibling stays put,
        // and the event carries the profile-name payload.
        (paid.activate)(&mut tray);
        assert_eq!(
            rx.try_recv(),
            Ok(TrayEvent::MenuActivated {
                id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
                payload: Some("Paid".to_string()),
            })
        );
        assert_eq!(
            tray.checked_overrides
                .get(&(TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, Some("Paid".to_string()))),
            Some(&false),
            "the override key is (id, payload)"
        );

        let menu = tray.menu();
        let MenuItem::SubMenu(SubMenu { submenu: profile_items, .. }) = &menu[9] else {
            panic!("entry 9 must be the profiles submenu");
        };
        let MenuItem::Checkmark(paid) = &profile_items[5] else {
            panic!("entry 5 must be the Paid auto-update checkmark");
        };
        let MenuItem::Checkmark(free) = &profile_items[6] else {
            panic!("entry 6 must be the Free auto-update checkmark");
        };
        assert!(!paid.checked, "clicked entry flips immediately");
        assert!(!free.checked, "sibling keeps the spec state (no crosstalk)");
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
