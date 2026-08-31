//! Pure [`TraySpec`] builder: turns one [`TraySpecContext`] snapshot into the
//! complete localized menu tree (0.20 full-feature tray, aligned with the
//! retired Tauri tray minus the browser entries — see
//! `docs/TAURI_WEBUI_RETIREMENT_LEDGER.md` §1.1).
//!
//! Red line: a pure function of the snapshot — no app-state access, no I/O,
//! no backend calls — so the whole menu is testable headlessly.

use infiltrator_shared::locales::{Lang, Localizer};

use super::spec::{
    TRAY_ACTION_ACTIVATE_PROFILE, TRAY_ACTION_CANCEL_CORE_DOWNLOAD, TRAY_ACTION_CANCEL_SYNC,
    TRAY_ACTION_CHECK_CORE_UPDATE, TRAY_ACTION_FLUSH_FAKEIP, TRAY_ACTION_FACTORY_RESET,
    TRAY_ACTION_INFO_ADMIN, TRAY_ACTION_INFO_CONTROLLER, TRAY_ACTION_INFO_DOWNLOAD,
    TRAY_ACTION_INFO_KERNEL_DEFAULT, TRAY_ACTION_INFO_KERNEL_STATUS,
    TRAY_ACTION_INFO_KERNEL_VERSION, TRAY_ACTION_INFO_MODE, TRAY_ACTION_INFO_STATUS,
    TRAY_ACTION_INFO_SYNC, TRAY_ACTION_MODE_DIRECT, TRAY_ACTION_MODE_GLOBAL,
    TRAY_ACTION_MODE_RULE, TRAY_ACTION_MODE_SCRIPT, TRAY_ACTION_NAVIGATE_SYNC,
    TRAY_ACTION_NO_PROFILES, TRAY_ACTION_NO_PROXIES, TRAY_ACTION_QUIT, TRAY_ACTION_SELECT_PROXY,
    TRAY_ACTION_SET_DEFAULT_KERNEL, TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, TRAY_ACTION_SHOW,
    TRAY_ACTION_SYNC_DOWNLOAD, TRAY_ACTION_SYNC_UPLOAD, TRAY_ACTION_TOGGLE_AUTOSTART,
    TRAY_ACTION_TOGGLE_SYSTEM_PROXY, TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN,
    TRAY_ACTION_UNINSTALL_KERNEL, TRAY_ACTION_UPDATE_ALL_PROFILES, TRAY_MAX_NODES_PER_GROUP,
    TRAY_SUBMENU_INFO, TRAY_SUBMENU_KERNEL, TRAY_SUBMENU_KERNEL_VERSION_BASE, TRAY_SUBMENU_MODE,
    TRAY_SUBMENU_PROFILES, TRAY_SUBMENU_PROXIES, TRAY_SUBMENU_PROXY_GROUP_BASE,
    TRAY_SUBMENU_PROXY_MORE_BASE, TRAY_SUBMENU_SYNC, TrayActionId, TrayMenuSpec, TrayMenuItem,
    TrayProxyGroup, TrayProxyNode, TraySpec, TraySpecContext, encode_pair_payload,
    load_icon_rgba, tray_status_key,
};

/// Translation closure type shared by the section builders below.
type Tr<'a> = &'a (dyn Fn(&str) -> String + 'a);

/// Build the localized full-feature tray spec from one app-state snapshot.
pub fn build_tray_spec(ctx: &TraySpecContext<'_>) -> TraySpec {
    let lang = Lang(ctx.lang);
    let tr = move |key: &str| lang.tr(key).into_owned();
    let items = vec![
        TrayMenuItem::action(TRAY_ACTION_SHOW, tr("tray_show_window")),
        TrayMenuItem::Separator,
        mode_submenu(ctx, &tr),
        proxies_submenu(ctx, &tr),
        TrayMenuItem::Separator,
        TrayMenuItem::checkmark(
            TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
            tr("system_proxy"),
            ctx.system_proxy,
        ),
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_TUN, tr("tun_mode"), ctx.tun),
        TrayMenuItem::action(TRAY_ACTION_TOGGLE_THEME, tr("tray_toggle_theme")),
        TrayMenuItem::Separator,
        profiles_submenu(ctx, &tr),
        kernel_submenu(ctx, &tr),
        sync_submenu(ctx, &tr),
        TrayMenuItem::checkmark(TRAY_ACTION_TOGGLE_AUTOSTART, tr("tray_autostart"), ctx.autostart),
        TrayMenuItem::Separator,
        info_submenu(ctx, &tr),
        TrayMenuItem::Separator,
        TrayMenuItem::action(TRAY_ACTION_FACTORY_RESET, tr("tray_factory_reset")),
        TrayMenuItem::action(TRAY_ACTION_QUIT, tr("tray_quit")),
    ];
    TraySpec {
        icon: load_icon_rgba(),
        tooltip: tooltip(ctx, &tr),
        menu: TrayMenuSpec { items },
    }
}

/// Localized display text for the current proxy mode (`-` when unset).
fn mode_text(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> String {
    match ctx.mode {
        Some("rule") => tr("mode_rule"),
        Some("global") => tr("mode_global"),
        Some("direct") => tr("mode_direct"),
        Some("script") => tr("tray_mode_script"),
        Some(other) => other.to_string(),
        None => "-".to_string(),
    }
}

fn default_kernel_version<'a>(ctx: &TraySpecContext<'a>) -> Option<&'a str> {
    ctx.kernels
        .iter()
        .find(|kernel| kernel.is_default)
        .map(|kernel| kernel.version.as_str())
}

/// Localized tooltip carrying the static state lines (mode/status/version).
/// No live traffic figures here on purpose — a per-sample spec push would
/// spam D-Bus and the native menu event loops.
fn tooltip(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> String {
    format!(
        "{}\n{}: {} · {}: {}\n{}: {}",
        tr("app_title"),
        tr("tray_info_mode"),
        mode_text(ctx, tr),
        tr("tray_info_status"),
        tr(tray_status_key(ctx.status)),
        tr("tray_kernel_version"),
        default_kernel_version(ctx).unwrap_or("-"),
    )
}

/// The 代理模式 submenu: rule / global / direct / script, the active mode
/// marked with `● `; script stays disabled without a profile script block.
fn mode_submenu(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> TrayMenuItem {
    let marked = |id: TrayActionId, mode_key: &str, label: String| {
        let active = ctx.mode.is_some_and(|current| current == mode_key);
        TrayMenuItem::Action {
            id,
            label: if active { format!("● {label}") } else { label },
            enabled: true,
            payload: None,
        }
    };
    let script_label = if ctx.script_block_present {
        tr("tray_mode_script")
    } else {
        tr("tray_mode_script_unavailable")
    };
    let script_active = ctx.mode == Some("script") && ctx.script_block_present;
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_MODE,
        label: tr("tray_mode"),
        enabled: true,
        items: vec![
            marked(TRAY_ACTION_MODE_RULE, "rule", tr("mode_rule")),
            marked(TRAY_ACTION_MODE_GLOBAL, "global", tr("mode_global")),
            marked(TRAY_ACTION_MODE_DIRECT, "direct", tr("mode_direct")),
            TrayMenuItem::Action {
                id: TRAY_ACTION_MODE_SCRIPT,
                label: if script_active {
                    format!("● {script_label}")
                } else {
                    script_label
                },
                enabled: ctx.script_block_present,
                payload: None,
            },
        ],
    }
}

/// One switchable node entry: `● ` marks the group's active node, the latest
/// measured delay is appended when known, and the payload encodes
/// `group␁node` for [`super::spec::TRAY_ACTION_SELECT_PROXY`].
fn node_entry(group: &TrayProxyGroup, node: &TrayProxyNode) -> TrayMenuItem {
    let is_active = node.name == group.current;
    let mut label = if is_active {
        format!("● {}", node.name)
    } else {
        node.name.clone()
    };
    if let Some(ms) = node.delay_ms {
        label.push_str(&format!(" ({ms} ms)"));
    }
    TrayMenuItem::Action {
        id: TRAY_ACTION_SELECT_PROXY,
        label,
        enabled: true,
        payload: Some(encode_pair_payload(&group.name, &node.name)),
    }
}

fn no_nodes_placeholder(tr: Tr<'_>) -> TrayMenuItem {
    TrayMenuItem::info(TRAY_ACTION_NO_PROXIES, tr("tray_no_proxies"))
}

/// The 节点切换 submenu: one nested submenu per proxy group (GLOBAL first,
/// capped by the assembly side), each listing its nodes with the disabled
/// placeholder as fallback when there is nothing to switch.
fn proxies_submenu(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> TrayMenuItem {
    let items = if ctx.groups.is_empty() {
        vec![no_nodes_placeholder(tr)]
    } else {
        ctx.groups
            .iter()
            .enumerate()
            .map(|(index, group)| group_submenu(index, group, tr))
            .collect()
    };
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_PROXIES,
        label: tr("tray_proxies"),
        enabled: true,
        items,
    }
}

/// Per-group submenu: the first [`TRAY_MAX_NODES_PER_GROUP`] nodes inline,
/// any rest folded into one nested `… +N` submenu; an empty group keeps the
/// disabled placeholder so the menu never renders a dead leaf.
fn group_submenu(index: usize, group: &TrayProxyGroup, tr: Tr<'_>) -> TrayMenuItem {
    let mut items: Vec<TrayMenuItem> = group
        .nodes
        .iter()
        .take(TRAY_MAX_NODES_PER_GROUP)
        .map(|node| node_entry(group, node))
        .collect();
    let rest: Vec<TrayMenuItem> = group
        .nodes
        .iter()
        .skip(TRAY_MAX_NODES_PER_GROUP)
        .map(|node| node_entry(group, node))
        .collect();
    if !rest.is_empty() {
        items.push(TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_PROXY_MORE_BASE + index as TrayActionId,
            label: format!("… +{}", rest.len()),
            enabled: true,
            items: rest,
        });
    }
    if items.is_empty() {
        items.push(no_nodes_placeholder(tr));
    }
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_PROXY_GROUP_BASE + index as TrayActionId,
        label: group.name.clone(),
        enabled: true,
        items,
    }
}

/// The 配置 submenu: one activation entry per profile (active marked `● `),
/// 全部更新订阅, then one auto-update checkmark per profile.
fn profiles_submenu(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> TrayMenuItem {
    let mut items = Vec::new();
    if ctx.profiles.is_empty() {
        items.push(TrayMenuItem::info(
            TRAY_ACTION_NO_PROFILES,
            tr("tray_no_profiles"),
        ));
    } else {
        items.extend(ctx.profiles.iter().map(|profile| TrayMenuItem::Action {
            id: TRAY_ACTION_ACTIVATE_PROFILE,
            label: if profile.active {
                format!("● {}", profile.name)
            } else {
                profile.name.clone()
            },
            enabled: true,
            payload: Some(profile.name.clone()),
        }));
        items.push(TrayMenuItem::Separator);
        items.push(TrayMenuItem::action(
            TRAY_ACTION_UPDATE_ALL_PROFILES,
            tr("tray_update_all_profiles"),
        ));
        items.push(TrayMenuItem::Separator);
        items.extend(ctx.profiles.iter().map(|profile| {
            TrayMenuItem::checkmark_with_payload(
                TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
                format!("{} · {}", tr("tray_auto_update"), profile.name),
                profile.auto_update_enabled,
                profile.name.clone(),
            )
        }));
    }
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_PROFILES,
        label: tr("tray_profiles"),
        enabled: true,
        items,
    }
}

/// The 内核 submenu: default-version + run-state info lines, one nested
/// submenu per installed version (设为默认 / 卸载, both no-ops on the
/// default), the update entry (morphing into progress + cancel while
/// downloading and a disabled line while checking), and Fake-IP cache flush.
fn kernel_submenu(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> TrayMenuItem {
    let mut items = vec![
        TrayMenuItem::info(
            TRAY_ACTION_INFO_KERNEL_DEFAULT,
            format!(
                "{}: {}",
                tr("tray_kernel_version"),
                default_kernel_version(ctx).unwrap_or("-")
            ),
        ),
        TrayMenuItem::info(
            TRAY_ACTION_INFO_KERNEL_STATUS,
            format!(
                "{}: {}",
                tr("tray_info_status"),
                tr(tray_status_key(ctx.status))
            ),
        ),
        TrayMenuItem::Separator,
    ];
    items.extend(ctx.kernels.iter().enumerate().map(|(index, kernel)| {
        TrayMenuItem::Submenu {
            id: TRAY_SUBMENU_KERNEL_VERSION_BASE + index as TrayActionId,
            label: kernel.version.clone(),
            enabled: true,
            items: vec![
                TrayMenuItem::Action {
                    id: TRAY_ACTION_SET_DEFAULT_KERNEL,
                    label: tr("tray_kernel_set_default"),
                    enabled: !kernel.is_default,
                    payload: Some(kernel.version.clone()),
                },
                TrayMenuItem::Action {
                    id: TRAY_ACTION_UNINSTALL_KERNEL,
                    label: tr("tray_kernel_uninstall"),
                    enabled: !kernel.is_default,
                    payload: Some(kernel.version.clone()),
                },
            ],
        }
    }));
    items.push(TrayMenuItem::Separator);
    if ctx.core_downloading {
        let label = match ctx.core_download_percent {
            Some(percent) => format!("{} ({percent}%)", tr("tray_kernel_downloading")),
            None => tr("tray_kernel_downloading"),
        };
        items.push(TrayMenuItem::info(TRAY_ACTION_INFO_DOWNLOAD, label));
        items.push(TrayMenuItem::action(
            TRAY_ACTION_CANCEL_CORE_DOWNLOAD,
            tr("tray_kernel_cancel_download"),
        ));
    } else if ctx.core_checking {
        items.push(TrayMenuItem::info(
            TRAY_ACTION_CHECK_CORE_UPDATE,
            tr("tray_kernel_checking"),
        ));
    } else {
        items.push(TrayMenuItem::action(
            TRAY_ACTION_CHECK_CORE_UPDATE,
            tr("tray_kernel_check_update"),
        ));
    }
    items.push(TrayMenuItem::action(
        TRAY_ACTION_FLUSH_FAKEIP,
        tr("tray_flush_fakeip"),
    ));
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_KERNEL,
        label: tr("tray_kernel"),
        enabled: true,
        items,
    }
}

/// The 同步 submenu: a disabled state line (未启用 / 已启用 / 同步中 i/n),
/// upload + download (download morphs into 取消同步 while syncing; both
/// need WebDAV), and the sync-settings entry.
fn sync_submenu(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> TrayMenuItem {
    let status = if ctx.syncing {
        match ctx.sync_step {
            Some((current, total)) => format!("{} ({current}/{total})", tr("tray_sync_syncing")),
            None => tr("tray_sync_syncing"),
        }
    } else if ctx.webdav_enabled {
        tr("tray_sync_idle")
    } else {
        tr("tray_sync_disabled")
    };
    let mut items = vec![
        TrayMenuItem::info(TRAY_ACTION_INFO_SYNC, status),
        TrayMenuItem::Separator,
        TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_UPLOAD,
            label: tr("tray_sync_upload"),
            enabled: ctx.webdav_enabled && !ctx.syncing,
            payload: None,
        },
    ];
    if ctx.syncing {
        items.push(TrayMenuItem::action(
            TRAY_ACTION_CANCEL_SYNC,
            tr("tray_sync_cancel"),
        ));
    } else {
        items.push(TrayMenuItem::Action {
            id: TRAY_ACTION_SYNC_DOWNLOAD,
            label: tr("tray_sync_download"),
            enabled: ctx.webdav_enabled,
            payload: None,
        });
    }
    items.push(TrayMenuItem::Separator);
    items.push(TrayMenuItem::action(
        TRAY_ACTION_NAVIGATE_SYNC,
        tr("tray_sync_settings"),
    ));
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_SYNC,
        label: tr("tray_sync"),
        enabled: true,
        items,
    }
}

/// The 信息 submenu: five always-disabled state lines (mode, run status,
/// controller URL, admin port, kernel version). Static on purpose — no live
/// traffic, so a spec push never churns D-Bus.
fn info_submenu(ctx: &TraySpecContext<'_>, tr: Tr<'_>) -> TrayMenuItem {
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_INFO,
        label: tr("tray_info"),
        enabled: true,
        items: vec![
            TrayMenuItem::info(
                TRAY_ACTION_INFO_MODE,
                format!("{}: {}", tr("tray_info_mode"), mode_text(ctx, tr)),
            ),
            TrayMenuItem::info(
                TRAY_ACTION_INFO_STATUS,
                format!(
                    "{}: {}",
                    tr("tray_info_status"),
                    tr(tray_status_key(ctx.status))
                ),
            ),
            TrayMenuItem::info(
                TRAY_ACTION_INFO_CONTROLLER,
                format!(
                    "{}: {}",
                    tr("tray_info_controller"),
                    ctx.controller.unwrap_or("-")
                ),
            ),
            TrayMenuItem::info(
                TRAY_ACTION_INFO_ADMIN,
                format!(
                    "{}: {}",
                    tr("tray_info_admin"),
                    if ctx.admin_enabled {
                        ctx.admin_port.to_string()
                    } else {
                        "-".to_string()
                    }
                ),
            ),
            TrayMenuItem::info(
                TRAY_ACTION_INFO_KERNEL_VERSION,
                format!(
                    "{}: {}",
                    tr("tray_kernel_version"),
                    default_kernel_version(ctx).unwrap_or("-")
                ),
            ),
        ],
    }
}
