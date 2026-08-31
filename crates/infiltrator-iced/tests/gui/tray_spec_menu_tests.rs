//! Pure-spec tray tests: menu tree layout, localized labels, payload
//! codecs and event→intent resolution. Mounted as a nested module of
//! `tray_tests.rs` (same red lines: no D-Bus, no ksni/muda spawn, no
//! network, no mihomo).

use super::*;
use crate::tray::menu::build_tray_spec;
use crate::tray::spec::{
    decode_pair_payload, encode_pair_payload, resolve_tray_event_in,
    TrayCoreStatus, TrayEvent, TrayEventContext, TrayIntent, TrayMenuItem,
    TRAY_ACTION_ACTIVATE_PROFILE, TRAY_ACTION_CANCEL_CORE_DOWNLOAD, TRAY_ACTION_CANCEL_SYNC,
    TRAY_ACTION_CHECK_CORE_UPDATE, TRAY_ACTION_FLUSH_FAKEIP, TRAY_ACTION_FACTORY_RESET,
    TRAY_ACTION_INFO_ADMIN, TRAY_ACTION_INFO_CONTROLLER, TRAY_ACTION_INFO_DOWNLOAD,
    TRAY_ACTION_INFO_KERNEL_DEFAULT, TRAY_ACTION_INFO_KERNEL_STATUS,
    TRAY_ACTION_INFO_KERNEL_VERSION, TRAY_ACTION_INFO_MODE, TRAY_ACTION_INFO_STATUS,
    TRAY_ACTION_INFO_SYNC, TRAY_ACTION_MODE_DIRECT, TRAY_ACTION_MODE_GLOBAL,
    TRAY_ACTION_MODE_RULE, TRAY_ACTION_MODE_SCRIPT, TRAY_ACTION_NAVIGATE_SYNC,
    TRAY_ACTION_NO_PROFILES, TRAY_ACTION_NO_PROXIES, TRAY_ACTION_QUIT,
    TRAY_ACTION_SELECT_GLOBAL_PROXY, TRAY_ACTION_SELECT_PROXY, TRAY_ACTION_SET_DEFAULT_KERNEL,
    TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, TRAY_ACTION_SHOW, TRAY_ACTION_SYNC_DOWNLOAD,
    TRAY_ACTION_SYNC_UPLOAD, TRAY_ACTION_TOGGLE_AUTOSTART, TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
    TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN, TRAY_ACTION_UNINSTALL_KERNEL,
    TRAY_ACTION_UPDATE_ALL_PROFILES, TRAY_SUBMENU_INFO, TRAY_SUBMENU_KERNEL,
    TRAY_SUBMENU_MODE, TRAY_SUBMENU_PROFILES, TRAY_SUBMENU_PROXIES,
    TRAY_SUBMENU_PROXY_GROUP_BASE, TRAY_SUBMENU_PROXY_MORE_BASE, TRAY_SUBMENU_SYNC,
};

#[test]
fn payload_pair_codec_round_trips_and_rejects_malformed() {
    // The group/node pair survives names containing colons and spaces.
    let encoded = encode_pair_payload("GLOBAL", "HK node:01");
    assert_eq!(encoded, "GLOBAL\u{1}HK node:01");
    assert_eq!(
        decode_pair_payload(&encoded),
        Some(("GLOBAL", "HK node:01"))
    );
    assert_eq!(decode_pair_payload("no-separator"), None);
}

#[test]
fn spec_layout_mirrors_the_full_feature_menu() {
    // 18 top-level entries: show, sep, mode, proxies, sep, sys, tun, theme,
    // sep, profiles, kernel, sync, autostart, sep, info, sep, factory, quit.
    let spec = build_tray_spec(&base_ctx());
    let items = &spec.menu.items;

    assert_eq!(items.len(), 18, "full top-level layout per the 0.20 tree");
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
            assert_eq!(
                mode_items
                    .iter()
                    .map(|item| item.action_label().unwrap_or_default())
                    .collect::<Vec<_>>(),
                vec!["● 规则模式", "全局模式", "直连模式", "脚本模式"],
                "active mode carries the `● ` marker; script enabled with a script block"
            );
            assert!(mode_items.iter().all(
                |item| matches!(item, TrayMenuItem::Action { enabled: true, .. })
            ));
        }
        other => panic!("expected mode submenu, got {other:?}"),
    }

    match &items[3] {
        TrayMenuItem::Submenu { id, label, items, .. } => {
            assert_eq!(*id, TRAY_SUBMENU_PROXIES);
            assert_eq!(label, "节点切换");
            assert_eq!(
                items,
                &vec![TrayMenuItem::info(TRAY_ACTION_NO_PROXIES, "暂无节点 (请先启动)")],
                "no groups: the disabled placeholder"
            );
        }
        other => panic!("expected proxies submenu, got {other:?}"),
    }

    assert_eq!(items[4], TrayMenuItem::Separator);
    assert_eq!(
        items[5],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_SYSTEM_PROXY, "系统代理 (System Proxy)", false)
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
    assert!(matches!(&items[9], TrayMenuItem::Submenu { id: TRAY_SUBMENU_PROFILES, .. }));
    assert!(matches!(&items[10], TrayMenuItem::Submenu { id: TRAY_SUBMENU_KERNEL, .. }));
    assert!(matches!(&items[11], TrayMenuItem::Submenu { id: TRAY_SUBMENU_SYNC, .. }));
    assert_eq!(
        items[12],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_AUTOSTART, "开机自启", false)
    );
    assert_eq!(items[13], TrayMenuItem::Separator);
    assert!(matches!(&items[14], TrayMenuItem::Submenu { id: TRAY_SUBMENU_INFO, .. }));
    assert_eq!(items[15], TrayMenuItem::Separator);
    assert_eq!(
        items[16],
        TrayMenuItem::action(TRAY_ACTION_FACTORY_RESET, "恢复出厂设置…")
    );
    assert_eq!(items[17], TrayMenuItem::action(TRAY_ACTION_QUIT, "退出应用"));

    // The icon is resolved from the crate's own icons directory.
    let icon = spec.icon.expect("spec embeds the shared RGBA icon");
    assert_eq!(icon.width, icon.height);
    assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    assert!(icon.rgba.as_chunks::<4>().0.iter().any(|px| px[3] != 0));

    // The tooltip is localized and carries mode/status/version lines.
    assert!(spec.tooltip.contains("MusicFrog Infiltrator"));
    assert!(spec.tooltip.contains("运行模式: 规则模式"));
    assert!(spec.tooltip.contains("运行状态: 已停止"));
    assert!(spec.tooltip.contains("内核版本: -"));
}

#[test]
fn spec_script_mode_entry_tracks_script_block_presence() {
    let mut ctx = base_ctx();
    ctx.script_block_present = false;
    ctx.mode = Some("script");
    let items = &build_tray_spec(&ctx).menu.items;

    let TrayMenuItem::Submenu { items: mode_items, .. } = &items[2] else {
        panic!("entry 2 must be the mode submenu");
    };
    let TrayMenuItem::Action { label, enabled, .. } = &mode_items[3] else {
        panic!("script entry must be an action");
    };
    assert_eq!(label, "脚本模式（配置未启用）");
    assert!(!*enabled, "script entry disabled without a script block");

    // With the block present the plain localized label is used instead.
    let items = &build_tray_spec(&base_ctx()).menu.items;
    let TrayMenuItem::Submenu { items: mode_items, .. } = &items[2] else {
        panic!("entry 2 must be the mode submenu");
    };
    assert_eq!(mode_items[3].action_label(), Some("脚本模式"));
}

#[test]
fn spec_encodes_proxy_groups_nodes_delays_and_overflow() {
    let hk_names: Vec<String> = (0..22).map(|index| format!("HK-{index:02}")).collect();
    let hk_nodes: Vec<(&str, Option<u32>)> =
        hk_names.iter().map(|name| (name.as_str(), Some(300))).collect();
    let groups = vec![
        proxy_group("GLOBAL", "B", &[("A", Some(120)), ("B", None)]),
        proxy_group("🇭🇰 HK", "HK-01", &hk_nodes),
    ];
    let mut ctx = base_ctx();
    ctx.groups = &groups;
    ctx.status = TrayCoreStatus::Running;
    let spec = build_tray_spec(&ctx);
    let items = &spec.menu.items;

    let TrayMenuItem::Submenu { items: group_subs, .. } = &items[3] else {
        panic!("entry 3 must be the proxies submenu");
    };
    assert_eq!(group_subs.len(), 2);
    assert!(matches!(
        &group_subs[0],
        TrayMenuItem::Submenu { id: TRAY_SUBMENU_PROXY_GROUP_BASE, .. }
    ));
    let TrayMenuItem::Submenu { id: hk_group_id, .. } = &group_subs[1] else {
        panic!("group 1 must be a submenu");
    };
    assert_eq!(*hk_group_id, TRAY_SUBMENU_PROXY_GROUP_BASE + 1);

    let TrayMenuItem::Submenu { label, items: nodes, .. } = &group_subs[0] else {
        panic!("group 0 must be a submenu");
    };
    assert_eq!(label, "GLOBAL");
    // Active node marker plus the delay suffix; untested nodes stay bare.
    assert_eq!(
        nodes[1],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SELECT_PROXY,
            label: "● B".to_string(),
            enabled: true,
            payload: Some(encode_pair_payload("GLOBAL", "B")),
        }
    );
    assert_eq!(
        nodes[0],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SELECT_PROXY,
            label: "A (120 ms)".to_string(),
            enabled: true,
            payload: Some(encode_pair_payload("GLOBAL", "A")),
        }
    );

    // 22 nodes fold into 20 inline entries plus a nested `… +N` submenu.
    let TrayMenuItem::Submenu { items: hk_items, .. } = &group_subs[1] else {
        panic!("group 1 must be a submenu");
    };
    assert_eq!(hk_items.len(), 21);
    let TrayMenuItem::Submenu {
        id, label, items: rest, ..
    } = &hk_items[20]
    else {
        panic!("overflow must fold into a nested submenu");
    };
    assert_eq!(*id, TRAY_SUBMENU_PROXY_MORE_BASE + 1);
    assert_eq!(label, "… +2");
    assert_eq!(rest.len(), 2);
    assert_eq!(
        rest[0].action_payload(),
        Some(encode_pair_payload("🇭🇰 HK", "HK-20").as_str())
    );
}

#[test]
fn spec_profiles_submenu_marks_active_auto_update_and_empty_state() {
    let profiles = vec![
        test_profile("Paid", true, false),
        test_profile("Free", false, true),
    ];
    let mut ctx = base_ctx();
    ctx.profiles = &profiles;
    let spec = build_tray_spec(&ctx);
    let items = &spec.menu.items;

    let TrayMenuItem::Submenu { items: profile_items, .. } = &items[9] else {
        panic!("entry 9 must be the profiles submenu");
    };
    assert_eq!(profile_items.len(), 7, "2 profiles, sep, update-all, sep, 2 checkmarks");
    assert_eq!(
        profile_items[0],
        TrayMenuItem::Action {
            id: TRAY_ACTION_ACTIVATE_PROFILE,
            label: "● Paid".to_string(),
            enabled: true,
            payload: Some("Paid".to_string()),
        }
    );
    assert_eq!(profile_items[1].action_label(), Some("Free"));
    assert_eq!(profile_items[2], TrayMenuItem::Separator);
    assert_eq!(
        profile_items[3],
        TrayMenuItem::action(TRAY_ACTION_UPDATE_ALL_PROFILES, "全部更新订阅")
    );
    assert_eq!(
        profile_items[5],
        TrayMenuItem::Checkmark {
            id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
            label: "自动更新 · Paid".to_string(),
            checked: false,
            enabled: true,
            payload: Some("Paid".to_string()),
        }
    );
    assert!(matches!(
        &profile_items[6],
        TrayMenuItem::Checkmark { checked: true, payload: Some(name), .. } if name == "Free"
    ));

    // Empty profiles keep the disabled placeholder.
    let spec = build_tray_spec(&base_ctx());
    let TrayMenuItem::Submenu { items: profile_items, .. } = &spec.menu.items[9] else {
        panic!("entry 9 must be the profiles submenu");
    };
    assert_eq!(
        profile_items,
        &vec![TrayMenuItem::info(TRAY_ACTION_NO_PROFILES, "暂无配置")]
    );
}

#[test]
fn spec_kernel_submenu_states() {
    let kernels = vec![test_kernel("v1.18.0", true), test_kernel("v1.19.0", false)];
    let mut ctx = base_ctx();
    ctx.kernels = &kernels;
    ctx.status = TrayCoreStatus::Running;
    let spec = build_tray_spec(&ctx);
    let items = &spec.menu.items;

    let TrayMenuItem::Submenu { label, items: kernel_items, .. } = &items[10] else {
        panic!("entry 10 must be the kernel submenu");
    };
    assert_eq!(label, "内核");
    assert_eq!(kernel_items.len(), 8, "2 info, sep, 2 versions, sep, update, flush");
    assert_eq!(
        kernel_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_KERNEL_DEFAULT, "内核版本: v1.18.0")
    );
    assert_eq!(
        kernel_items[1],
        TrayMenuItem::info(TRAY_ACTION_INFO_KERNEL_STATUS, "运行状态: 运行中")
    );
    assert_eq!(kernel_items[2], TrayMenuItem::Separator);

    // Per-version submenus: the default version's entries are disabled no-ops.
    for (index, version) in ["v1.18.0", "v1.19.0"].iter().enumerate() {
        let TrayMenuItem::Submenu {
            items: version_items,
            ..
        } = &kernel_items[3 + index]
        else {
            panic!("kernel entry must be a per-version submenu");
        };
        let default_entry = version_items[0].clone();
        assert!(matches!(
            &default_entry,
            TrayMenuItem::Action { id: TRAY_ACTION_SET_DEFAULT_KERNEL, payload: Some(v), enabled, .. }
                if *v == *version && *enabled == (*version == "v1.19.0")
        ));
        assert!(matches!(
            &version_items[1],
            TrayMenuItem::Action { id: TRAY_ACTION_UNINSTALL_KERNEL, payload: Some(v), enabled, .. }
                if *v == *version && *enabled == (*version == "v1.19.0")
        ));
    }

    assert_eq!(
        kernel_items[6],
        TrayMenuItem::action(TRAY_ACTION_CHECK_CORE_UPDATE, "检查并更新内核")
    );
    assert_eq!(
        kernel_items[7],
        TrayMenuItem::action(TRAY_ACTION_FLUSH_FAKEIP, "清理 Fake-IP 缓存")
    );
}

#[test]
fn spec_kernel_update_entry_morphs_while_checking_and_downloading() {
    let kernels = vec![test_kernel("v1.18.0", true), test_kernel("v1.19.0", false)];
    let mut ctx = base_ctx();
    ctx.kernels = &kernels;
    ctx.core_checking = true;
    let items = &build_tray_spec(&ctx).menu.items;
    let TrayMenuItem::Submenu { items: kernel_items, .. } = &items[10] else {
        panic!("entry 10 must be the kernel submenu");
    };
    assert_eq!(
        kernel_items[6],
        TrayMenuItem::info(TRAY_ACTION_CHECK_CORE_UPDATE, "正在检查更新…")
    );

    ctx.core_checking = false;
    ctx.core_downloading = true;
    ctx.core_download_percent = Some(45);
    let items = &build_tray_spec(&ctx).menu.items;
    let TrayMenuItem::Submenu { items: kernel_items, .. } = &items[10] else {
        panic!("entry 10 must be the kernel submenu");
    };
    assert_eq!(
        kernel_items[6],
        TrayMenuItem::info(TRAY_ACTION_INFO_DOWNLOAD, "下载中 (45%)")
    );
    assert_eq!(
        kernel_items[7],
        TrayMenuItem::action(TRAY_ACTION_CANCEL_CORE_DOWNLOAD, "取消下载")
    );
}

#[test]
fn spec_sync_submenu_states() {
    // Disabled WebDAV: status line plus inert upload/download.
    let items = &build_tray_spec(&base_ctx()).menu.items;
    let TrayMenuItem::Submenu { items: sync_items, .. } = &items[11] else {
        panic!("entry 11 must be the sync submenu");
    };
    assert_eq!(sync_items[0], TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "未启用 WebDAV 同步"));
    assert!(matches!(
        &sync_items[2],
        TrayMenuItem::Action { id: TRAY_ACTION_SYNC_UPLOAD, enabled: false, .. }
    ));
    assert!(matches!(
        &sync_items[3],
        TrayMenuItem::Action { id: TRAY_ACTION_SYNC_DOWNLOAD, enabled: false, .. }
    ));
    assert_eq!(sync_items[5], TrayMenuItem::action(TRAY_ACTION_NAVIGATE_SYNC, "同步设置…"));

    // Enabled: upload/download go live.
    let mut ctx = base_ctx();
    ctx.webdav_enabled = true;
    let items = &build_tray_spec(&ctx).menu.items;
    let TrayMenuItem::Submenu { items: sync_items, .. } = &items[11] else {
        panic!("entry 11 must be the sync submenu");
    };
    assert_eq!(sync_items[0], TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "WebDAV 同步已启用"));
    assert!(matches!(
        &sync_items[2],
        TrayMenuItem::Action { id: TRAY_ACTION_SYNC_UPLOAD, enabled: true, .. }
    ));
    assert!(matches!(
        &sync_items[3],
        TrayMenuItem::Action { id: TRAY_ACTION_SYNC_DOWNLOAD, enabled: true, .. }
    ));

    // Syncing: status line carries the step counters, download becomes cancel.
    ctx.syncing = true;
    ctx.sync_step = Some((2, 5));
    let items = &build_tray_spec(&ctx).menu.items;
    let TrayMenuItem::Submenu { items: sync_items, .. } = &items[11] else {
        panic!("entry 11 must be the sync submenu");
    };
    assert_eq!(
        sync_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "同步中 (2/5)")
    );
    assert!(matches!(
        &sync_items[2],
        TrayMenuItem::Action { id: TRAY_ACTION_SYNC_UPLOAD, enabled: false, .. }
    ));
    assert!(matches!(
        &sync_items[3],
        TrayMenuItem::Action { id: TRAY_ACTION_CANCEL_SYNC, enabled: true, .. }
    ));
}

#[test]
fn spec_info_lines_are_disabled_and_localized() {
    let mut ctx = base_ctx();
    ctx.status = TrayCoreStatus::Error;
    ctx.admin_enabled = true;
    ctx.mode = Some("global");
    let items = &build_tray_spec(&ctx).menu.items;

    let TrayMenuItem::Submenu { label, items: info_items, .. } = &items[14] else {
        panic!("entry 14 must be the info submenu");
    };
    assert_eq!(label, "信息");
    assert_eq!(
        info_items,
        &vec![
            TrayMenuItem::info(TRAY_ACTION_INFO_MODE, "运行模式: 全局模式"),
            TrayMenuItem::info(TRAY_ACTION_INFO_STATUS, "运行状态: 异常"),
            TrayMenuItem::info(TRAY_ACTION_INFO_CONTROLLER, "控制接口: http://127.0.0.1:9090"),
            TrayMenuItem::info(TRAY_ACTION_INFO_ADMIN, "管理端口: 25210"),
            TrayMenuItem::info(TRAY_ACTION_INFO_KERNEL_VERSION, "内核版本: -"),
        ]
    );
    // Informational lines are read-only: disabled and never resolvable.
    assert!(info_items
        .iter()
        .all(|item| matches!(item, TrayMenuItem::Action { enabled: false, .. })));
}

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
            &activated(TRAY_ACTION_SELECT_PROXY, Some(&encode_pair_payload("GLOBAL", "X"))),
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
        resolve_tray_event_in(&activated(TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, Some("Ghost")), &ctx),
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

#[test]
fn every_spec_action_id_resolves_to_an_intent() {
    // Fully populated snapshot so every fixed action appears; disabled
    // informational lines are the expected misses.
    let groups = vec![
        proxy_group("GLOBAL", "A", &[("A", None), ("B", Some(80))]),
        proxy_group("HK", "HK-1", &[("HK-1", None)]),
    ];
    let profiles = vec![test_profile("Paid", true, false), test_profile("Free", false, true)];
    let kernels = vec![test_kernel("v1.18.0", true), test_kernel("v1.19.0", false)];
    let mut ctx = base_ctx();
    ctx.groups = &groups;
    ctx.profiles = &profiles;
    ctx.kernels = &kernels;
    ctx.webdav_enabled = true;
    ctx.admin_enabled = true;
    let spec = build_tray_spec(&ctx);
    let event_ctx = TrayEventContext {
        system_proxy: false,
        tun: false,
        autostart: false,
        profiles: &profiles,
    };

    const EXPECTED_MISSES: &[super::super::spec::TrayActionId] = &[
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
    ];

    fn walk(
        items: &[TrayMenuItem],
        ctx: &TrayEventContext<'_>,
        hits: &mut usize,
        misses: &mut Vec<super::super::spec::TrayActionId>,
    ) {
        for item in items {
            match item {
                TrayMenuItem::Action { id, payload, .. } => {
                    let resolved = resolve_tray_event_in(
                        &TrayEvent::MenuActivated {
                            id: *id,
                            payload: payload.clone(),
                        },
                        ctx,
                    );
                    if EXPECTED_MISSES.contains(id) {
                        assert!(resolved.is_none(), "informational entry {id} must not resolve");
                        misses.push(*id);
                    } else {
                        assert!(resolved.is_some(), "action {id} must resolve");
                        *hits += 1;
                    }
                }
                TrayMenuItem::Checkmark { id, payload, .. } => {
                    assert!(
                        resolve_tray_event_in(
                            &TrayEvent::MenuActivated {
                                id: *id,
                                payload: payload.clone(),
                            },
                            ctx,
                        )
                        .is_some(),
                        "checkmark {id} must resolve"
                    );
                    *hits += 1;
                }
                TrayMenuItem::Submenu { items, .. } => walk(items, ctx, hits, misses),
                TrayMenuItem::Separator => {}
            }
        }
    }

    let (mut hits, mut misses) = (0, Vec::new());
    walk(&spec.menu.items, &event_ctx, &mut hits, &mut misses);
    // 28 clickable entries: show, 4 modes, 3 nodes, sys/tun/theme, 2 profile
    // activations, update-all, 2 auto-update checkmarks, 2×(set-default +
    // uninstall), check-update, flush-fakeip, upload, download, sync
    // settings, autostart, factory reset, quit.
    assert_eq!(hits, 28);
    // 8 read-only lines rendered by this snapshot: the two kernel info
    // entries, the sync status line and the five-entry info submenu. (The
    // two placeholders and the download-progress line only appear in their
    // degraded snapshots, covered by the dedicated tests above.)
    misses.sort_unstable();
    assert_eq!(misses, vec![80, 81, 82, 83, 84, 85, 86, 87]);
    assert!(misses.iter().all(|id| EXPECTED_MISSES.contains(id)));
}
