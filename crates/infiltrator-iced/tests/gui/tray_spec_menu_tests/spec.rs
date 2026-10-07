//! Behavior cases for spec.
//! test-intent: behavior

use super::*;

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
            assert!(
                mode_items
                    .iter()
                    .all(|item| matches!(item, TrayMenuItem::Action { enabled: true, .. }))
            );
        }
        other => panic!("expected mode submenu, got {other:?}"),
    }

    match &items[3] {
        TrayMenuItem::Submenu {
            id, label, items, ..
        } => {
            assert_eq!(*id, TRAY_SUBMENU_PROXIES);
            assert_eq!(label, "节点切换");
            assert_eq!(
                items,
                &vec![TrayMenuItem::info(
                    TRAY_ACTION_NO_PROXIES,
                    "暂无节点 (请先启动)"
                )],
                "no groups: the disabled placeholder"
            );
        }
        other => panic!("expected proxies submenu, got {other:?}"),
    }

    assert_eq!(items[4], TrayMenuItem::Separator);
    assert_eq!(
        items[5],
        TrayMenuItem::checkmark(
            TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
            "系统代理 (System Proxy)",
            false
        )
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
    assert!(matches!(
        &items[9],
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_PROFILES,
            ..
        }
    ));
    assert!(matches!(
        &items[10],
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_KERNEL,
            ..
        }
    ));
    assert!(matches!(
        &items[11],
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_SYNC,
            ..
        }
    ));
    assert_eq!(
        items[12],
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_AUTOSTART, "开机自启", false)
    );
    assert_eq!(items[13], TrayMenuItem::Separator);
    assert!(matches!(
        &items[14],
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_INFO,
            ..
        }
    ));
    assert_eq!(items[15], TrayMenuItem::Separator);
    assert_eq!(
        items[16],
        TrayMenuItem::action(TRAY_ACTION_FACTORY_RESET, "恢复出厂设置…")
    );
    assert_eq!(
        items[17],
        TrayMenuItem::action(TRAY_ACTION_QUIT, "退出应用")
    );

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

    let TrayMenuItem::Submenu {
        items: mode_items, ..
    } = &items[2]
    else {
        panic!("entry 2 must be the mode submenu");
    };
    let TrayMenuItem::Action { label, enabled, .. } = &mode_items[3] else {
        panic!("script entry must be an action");
    };
    assert_eq!(label, "脚本模式（配置未启用）");
    assert!(!*enabled, "script entry disabled without a script block");

    // With the block present the plain localized label is used instead.
    let items = &build_tray_spec(&base_ctx()).menu.items;
    let TrayMenuItem::Submenu {
        items: mode_items, ..
    } = &items[2]
    else {
        panic!("entry 2 must be the mode submenu");
    };
    assert_eq!(mode_items[3].action_label(), Some("脚本模式"));
}

#[test]
fn spec_encodes_proxy_groups_nodes_delays_and_overflow() {
    let hk_names: Vec<String> = (0..22).map(|index| format!("HK-{index:02}")).collect();
    let hk_nodes: Vec<(&str, Option<u32>)> = hk_names
        .iter()
        .map(|name| (name.as_str(), Some(300)))
        .collect();
    let groups = vec![
        proxy_group("GLOBAL", "B", &[("A", Some(120)), ("B", None)]),
        proxy_group("🇭🇰 HK", "HK-01", &hk_nodes),
    ];
    let mut ctx = base_ctx();
    ctx.groups = &groups;
    ctx.status = TrayCoreStatus::Running;
    let spec = build_tray_spec(&ctx);
    let items = &spec.menu.items;

    let TrayMenuItem::Submenu {
        items: group_subs, ..
    } = &items[3]
    else {
        panic!("entry 3 must be the proxies submenu");
    };
    assert_eq!(group_subs.len(), 2);
    assert!(matches!(
        &group_subs[0],
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_PROXY_GROUP_BASE,
            ..
        }
    ));
    let TrayMenuItem::Submenu {
        id: hk_group_id, ..
    } = &group_subs[1]
    else {
        panic!("group 1 must be a submenu");
    };
    assert_eq!(*hk_group_id, TRAY_SUBMENU_PROXY_GROUP_BASE + 1);

    let TrayMenuItem::Submenu {
        label,
        items: nodes,
        ..
    } = &group_subs[0]
    else {
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
    let TrayMenuItem::Submenu {
        items: hk_items, ..
    } = &group_subs[1]
    else {
        panic!("group 1 must be a submenu");
    };
    assert_eq!(hk_items.len(), 21);
    let TrayMenuItem::Submenu {
        id,
        label,
        items: rest,
        ..
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

    let TrayMenuItem::Submenu {
        items: profile_items,
        ..
    } = &items[9]
    else {
        panic!("entry 9 must be the profiles submenu");
    };
    assert_eq!(
        profile_items.len(),
        7,
        "2 profiles, sep, update-all, sep, 2 checkmarks"
    );
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
    let TrayMenuItem::Submenu {
        items: profile_items,
        ..
    } = &spec.menu.items[9]
    else {
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

    let TrayMenuItem::Submenu {
        label,
        items: kernel_items,
        ..
    } = &items[10]
    else {
        panic!("entry 10 must be the kernel submenu");
    };
    assert_eq!(label, "内核");
    assert_eq!(
        kernel_items.len(),
        8,
        "2 info, sep, 2 versions, sep, update, flush"
    );
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
    let TrayMenuItem::Submenu {
        items: kernel_items,
        ..
    } = &items[10]
    else {
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
    let TrayMenuItem::Submenu {
        items: kernel_items,
        ..
    } = &items[10]
    else {
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
    let TrayMenuItem::Submenu {
        items: sync_items, ..
    } = &items[11]
    else {
        panic!("entry 11 must be the sync submenu");
    };
    assert_eq!(
        sync_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "未启用 WebDAV 同步")
    );
    assert!(matches!(
        &sync_items[2],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_UPLOAD,
            enabled: false,
            ..
        }
    ));
    assert!(matches!(
        &sync_items[3],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_DOWNLOAD,
            enabled: false,
            ..
        }
    ));
    assert_eq!(
        sync_items[5],
        TrayMenuItem::action(TRAY_ACTION_NAVIGATE_SYNC, "同步设置…")
    );

    // Enabled: upload/download go live.
    let mut ctx = base_ctx();
    ctx.webdav_enabled = true;
    let items = &build_tray_spec(&ctx).menu.items;
    let TrayMenuItem::Submenu {
        items: sync_items, ..
    } = &items[11]
    else {
        panic!("entry 11 must be the sync submenu");
    };
    assert_eq!(
        sync_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "WebDAV 同步已启用")
    );
    assert!(matches!(
        &sync_items[2],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_UPLOAD,
            enabled: true,
            ..
        }
    ));
    assert!(matches!(
        &sync_items[3],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_DOWNLOAD,
            enabled: true,
            ..
        }
    ));

    // Syncing: status line carries the step counters, download becomes cancel.
    ctx.syncing = true;
    ctx.sync_step = Some((2, 5));
    let items = &build_tray_spec(&ctx).menu.items;
    let TrayMenuItem::Submenu {
        items: sync_items, ..
    } = &items[11]
    else {
        panic!("entry 11 must be the sync submenu");
    };
    assert_eq!(
        sync_items[0],
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, "同步中 (2/5)")
    );
    assert!(matches!(
        &sync_items[2],
        TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_UPLOAD,
            enabled: false,
            ..
        }
    ));
    assert!(matches!(
        &sync_items[3],
        TrayMenuItem::Action {
            id: TRAY_ACTION_CANCEL_SYNC,
            enabled: true,
            ..
        }
    ));
}

#[test]
fn spec_info_lines_are_disabled_and_localized() {
    let mut ctx = base_ctx();
    ctx.status = TrayCoreStatus::Error;
    ctx.admin_enabled = true;
    ctx.mode = Some("global");
    let items = &build_tray_spec(&ctx).menu.items;

    let TrayMenuItem::Submenu {
        label,
        items: info_items,
        ..
    } = &items[14]
    else {
        panic!("entry 14 must be the info submenu");
    };
    assert_eq!(label, "信息");
    assert_eq!(
        info_items,
        &vec![
            TrayMenuItem::info(TRAY_ACTION_INFO_MODE, "运行模式: 全局模式"),
            TrayMenuItem::info(TRAY_ACTION_INFO_STATUS, "运行状态: 异常"),
            TrayMenuItem::info(
                TRAY_ACTION_INFO_CONTROLLER,
                "控制接口: http://127.0.0.1:9090"
            ),
            TrayMenuItem::info(TRAY_ACTION_INFO_ADMIN, "管理端口: 25210"),
            TrayMenuItem::info(TRAY_ACTION_INFO_KERNEL_VERSION, "内核版本: -"),
        ]
    );
    // Informational lines are read-only: disabled and never resolvable.
    assert!(
        info_items
            .iter()
            .all(|item| matches!(item, TrayMenuItem::Action { enabled: false, .. }))
    );
}
