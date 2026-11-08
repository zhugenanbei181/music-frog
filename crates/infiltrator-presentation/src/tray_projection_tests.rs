use super::*;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::tray_menu::{
    TRAY_ACTION_FACTORY_RESET, TRAY_ACTION_INFO_SYNC, TRAY_ACTION_QUIT, TRAY_ACTION_SHOW,
    TRAY_ACTION_TOGGLE_AUTOSTART, TRAY_ACTION_TOGGLE_COLOR_MODE, TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
    TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN, TRAY_SUBMENU_INFO, TRAY_SUBMENU_KERNEL,
    TRAY_SUBMENU_PROFILES, TRAY_SUBMENU_PROXIES, TRAY_SUBMENU_ROUTING_INTENT, TRAY_SUBMENU_SYNC,
    TrayMenuCapabilities,
};

fn base_ctx() -> TrayMenuContext {
    TrayMenuContext {
        surface: SurfaceKind::IcedDesktop,
        lang: "zh-CN".to_string(),
        capabilities: TrayMenuCapabilities::full(),
        ..TrayMenuContext::default()
    }
}

#[test]
fn the_builder_renders_the_shared_tree_from_one_snapshot() {
    let spec = build_tray_spec(&base_ctx(), TrayIconColorMode::Colored);
    assert_eq!(spec.menu.items[0].action_label(), Some("显示主界面"));
    assert_eq!(
        spec.menu.items[13],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_AUTOSTART, "开机自启", false)
    );
    assert_eq!(spec.menu.items.len(), 19);
}

#[test]
fn the_color_mode_checkmark_reflects_the_preference() {
    let colored = build_tray_spec(&base_ctx(), TrayIconColorMode::Colored);
    let mono = build_tray_spec(&base_ctx(), TrayIconColorMode::Monochrome);
    let colored_mark = colored
        .menu
        .items
        .iter()
        .find(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_COLOR_MODE))
        .expect("color mode entry");
    let mono_mark = mono
        .menu
        .items
        .iter()
        .find(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_COLOR_MODE))
        .expect("color mode entry");
    assert_eq!(
        colored_mark,
        &TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_COLOR_MODE, "单色托盘图标", false)
    );
    assert_eq!(
        mono_mark,
        &TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_COLOR_MODE, "单色托盘图标", true)
    );
}

#[test]
fn the_status_icon_carries_the_status_colour() {
    let running =
        generate_tray_icon_rgba(TrayServiceStatus::Running, TrayIconColorMode::Colored, 16);
    let stopped =
        generate_tray_icon_rgba(TrayServiceStatus::Stopped, TrayIconColorMode::Colored, 16);
    assert_eq!(running.width, 16);
    assert_eq!(running.height, 16);
    assert_eq!(running.rgba.len(), 16 * 16 * 4);
    assert_ne!(running.rgba, stopped.rgba);
    // A fully transparent corner and an opaque centre.
    assert_eq!(&running.rgba[0..4], &[0, 0, 0, 0]);
    let center = (16 / 2) as usize * 16 * 4 + (16 / 2) as usize * 4;
    assert_eq!(&running.rgba[center..center + 4], &[76, 175, 80, 255]);
}

#[test]
fn monochrome_keeps_the_status_readable_by_shape() {
    let running = generate_tray_icon_rgba(
        TrayServiceStatus::Running,
        TrayIconColorMode::Monochrome,
        16,
    );
    let stopped = generate_tray_icon_rgba(
        TrayServiceStatus::Stopped,
        TrayIconColorMode::Monochrome,
        16,
    );
    let centre = (16 / 2) as usize * 16 * 4 + (16 / 2) as usize * 4;
    assert_eq!(&running.rgba[centre..centre + 4], &[255, 255, 255, 255]);
    assert_eq!(&stopped.rgba[centre..centre + 4], &[0, 0, 0, 0]);
}

#[test]
fn the_status_axis_is_honest_about_uncertainty() {
    assert_eq!(
        service_status(TrayCoreStatus::Stopped),
        TrayServiceStatus::Stopped
    );
    assert_eq!(
        service_status(TrayCoreStatus::Starting),
        TrayServiceStatus::Connecting
    );
    assert_eq!(
        service_status(TrayCoreStatus::Running),
        TrayServiceStatus::Running
    );
    assert_eq!(
        service_status(TrayCoreStatus::Error),
        TrayServiceStatus::Warning
    );
}

#[test]
fn the_sync_and_info_lines_come_from_the_shared_copy() {
    let mut ctx = base_ctx();
    ctx.webdav_enabled = true;
    let spec = build_tray_spec(&ctx, TrayIconColorMode::Colored);
    let TrayMenuItem::Submenu {
        items: sync_items, ..
    } = &spec.menu.items[12]
    else {
        panic!("entry 12 must be the sync submenu");
    };
    assert_eq!(
        sync_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "WebDAV 同步已启用")
    );
}

#[test]
fn shared_desktop_capabilities_omits_unsupported_families() {
    let mut ctx = base_ctx();
    ctx.capabilities = TrayMenuCapabilities::shared_desktop();
    let spec = build_tray_spec(&ctx, TrayIconColorMode::Colored);
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_SUBMENU_ROUTING_INTENT))
    );
    assert!(
        !spec
            .menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_SUBMENU_PROXIES))
    );
    assert!(
        !spec
            .menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_THEME))
    );
    assert!(
        !spec
            .menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_SUBMENU_KERNEL))
    );
    assert!(
        !spec
            .menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_SUBMENU_SYNC))
    );
    assert!(
        !spec
            .menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_AUTOSTART))
    );
    assert!(
        !spec
            .menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_FACTORY_RESET))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_SHOW))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_SYSTEM_PROXY))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_TUN))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_TOGGLE_COLOR_MODE))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_SUBMENU_PROFILES))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_SUBMENU_INFO))
    );
    assert!(
        spec.menu
            .items
            .iter()
            .any(|item| item.action_id() == Some(TRAY_ACTION_QUIT))
    );
}
