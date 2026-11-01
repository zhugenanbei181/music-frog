//! Headless ksni mapping tests: the neutral spec renders the same SNI menu
//! structure, checkmark states and payload encoding on every Linux session.
//! No D-Bus session, no tray spawn, no network — red lines.

use super::*;
use infiltrator_contract::tray_menu::{
    TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, TRAY_ACTION_TOGGLE_TUN, TrayMenuContext,
    TrayOutboundGroup, TrayOutboundNode, TrayProfileEntry,
};
use infiltrator_contract::tray_status::TrayIconColorMode;
use infiltrator_presentation::tray_projection::build_tray_spec;
use infiltrator_shared::locales::{Lang, Localizer};
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{MenuItem, Tray as _};
use std::collections::HashMap;
use std::sync::mpsc;

fn sample_tray(events: mpsc::Sender<TrayEvent>) -> KsniTray {
    let mut context = TrayMenuContext {
        lang: "zh-CN".to_string(),
        ..TrayMenuContext::default()
    };
    context.groups.push(TrayOutboundGroup {
        name: "GLOBAL".to_string(),
        current: "A".to_string(),
        nodes: vec![TrayOutboundNode {
            name: "A".to_string(),
            delay_ms: Some(64),
        }],
    });
    context.tun = true;
    KsniTray {
        spec: build_tray_spec(&context, TrayIconColorMode::Colored),
        checked_overrides: HashMap::new(),
        events,
    }
}

fn two_profiles() -> Vec<TrayProfileEntry> {
    vec![
        TrayProfileEntry {
            name: "Paid".to_string(),
            active: true,
            auto_update_enabled: true,
        },
        TrayProfileEntry {
            name: "Free".to_string(),
            active: false,
            auto_update_enabled: false,
        },
    ]
}

#[test]
fn ksni_menu_mapping_preserves_spec_structure_and_state() {
    let (tx, _rx) = mpsc::channel::<TrayEvent>();
    let tray = sample_tray(tx);
    let menu = map_items(&tray.spec.menu.items, &tray.checked_overrides, &tray.events);

    assert_eq!(menu.len(), tray.spec.menu.items.len());
    match &menu[0] {
        MenuItem::Standard(StandardItem { label, .. }) => assert_eq!(label, "显示主界面"),
        _ => panic!("entry 0 must be a standard item"),
    }
    assert!(matches!(&menu[1], MenuItem::Separator));

    match &menu[2] {
        MenuItem::SubMenu(SubMenu { label, submenu, .. }) => {
            assert_eq!(label, Lang("zh-CN").tr("routing_intent_title").as_ref());
            assert_eq!(submenu.len(), 3);
            match &submenu[0] {
                MenuItem::Standard(StandardItem { label, .. }) => {
                    assert!(
                        label.contains("规则"),
                        "the active intent carries the marker: {label}"
                    )
                }
                _ => panic!("each routing choice renders as a standard item"),
            }
        }
        _ => panic!("entry 2 must be a submenu"),
    }

    match &menu[3] {
        MenuItem::SubMenu(SubMenu { label, .. }) => {
            assert_eq!(label, Lang("zh-CN").tr("tray_proxies").as_ref())
        }
        _ => panic!("entry 3 must be the proxies submenu"),
    }

    match &menu[6] {
        MenuItem::Checkmark(CheckmarkItem { checked, .. }) => assert!(*checked),
        _ => panic!("entry 6 must be the TUN checkmark"),
    }
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
    (item.activate)(&mut tray);
    assert_eq!(
        rx.try_recv(),
        Ok(TrayEvent::MenuActivated {
            id: TRAY_ACTION_TOGGLE_TUN,
            payload: None,
        })
    );

    // The flip is visible immediately, before the host pushes a new spec.
    let menu = tray.menu();
    let MenuItem::Checkmark(item) = &menu[6] else {
        panic!("entry 6 must still be the TUN checkmark");
    };
    assert!(!item.checked);
}

#[test]
fn ksni_checkmark_overrides_are_keyed_per_payload() {
    let mut context = TrayMenuContext {
        lang: "zh-CN".to_string(),
        ..TrayMenuContext::default()
    };
    context.profiles = two_profiles();
    let (tx, rx) = mpsc::channel::<TrayEvent>();
    let mut tray = KsniTray {
        spec: build_tray_spec(&context, TrayIconColorMode::Colored),
        checked_overrides: HashMap::new(),
        events: tx,
    };

    let menu = tray.menu();
    let MenuItem::SubMenu(SubMenu {
        submenu: profile_items,
        ..
    }) = &menu[10]
    else {
        panic!("entry 10 must be the profiles submenu");
    };
    let MenuItem::Checkmark(paid) = &profile_items[5] else {
        panic!("entry 5 must be the Paid auto-update checkmark");
    };
    assert!(paid.checked);
    let MenuItem::Checkmark(free) = &profile_items[6] else {
        panic!("entry 6 must be the Free auto-update checkmark");
    };
    assert!(!free.checked);

    (paid.activate)(&mut tray);
    assert_eq!(
        rx.try_recv(),
        Ok(TrayEvent::MenuActivated {
            id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
            payload: Some("Paid".to_string()),
        })
    );
    assert_eq!(
        tray.checked_overrides.get(&(
            TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
            Some("Paid".to_string())
        )),
        Some(&false),
        "the override key is (id, payload)"
    );

    let menu = tray.menu();
    let MenuItem::SubMenu(SubMenu {
        submenu: profile_items,
        ..
    }) = &menu[10]
    else {
        panic!("entry 10 must be the profiles submenu");
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
