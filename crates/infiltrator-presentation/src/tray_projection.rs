//! Shared tray-menu projection (FLPARITY-006).
//!
//! Every desktop surface renders the system tray from this module, so the
//! menu shape, the localized copy and the status icon are one product fact:
//!
//! * [`build_tray_spec`] turns one [`TrayMenuContext`] snapshot into the
//!   complete localized menu tree plus the status icon. It is a pure function
//!   of the snapshot — no app-state access, no I/O, no backend calls — so the
//!   whole menu is testable headlessly.
//! * [`generate_tray_icon_rgba`] renders the FlClash-parity status icon
//!   (colored by [`TrayServiceStatus`] or monochrome for the macOS menu bar).
//! * [`tray_context_from_snapshot`] projects the shared surface snapshot the
//!   Bevy and Slint hosts already publish into a [`TrayMenuContext`], so a
//!   host that owns no second business state still feeds the same builder.

use crate::routing_intent_projection::routing_intent_copy;
use infiltrator_contract::surface_snapshot::ProfilesPageSnapshot;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::tray_menu::{
    TRAY_ACTION_ACTIVATE_PROFILE, TRAY_ACTION_CANCEL_CORE_DOWNLOAD, TRAY_ACTION_CANCEL_SYNC,
    TRAY_ACTION_CHECK_CORE_UPDATE, TRAY_ACTION_FACTORY_RESET, TRAY_ACTION_FLUSH_FAKEIP,
    TRAY_ACTION_INFO_ADMIN, TRAY_ACTION_INFO_CONTROLLER, TRAY_ACTION_INFO_DOWNLOAD,
    TRAY_ACTION_INFO_KERNEL_DEFAULT, TRAY_ACTION_INFO_KERNEL_STATUS,
    TRAY_ACTION_INFO_KERNEL_VERSION, TRAY_ACTION_INFO_RATE, TRAY_ACTION_INFO_ROUTING_INTENT,
    TRAY_ACTION_INFO_STATUS, TRAY_ACTION_INFO_SYNC, TRAY_ACTION_NAVIGATE_SYNC,
    TRAY_ACTION_NO_PROFILES, TRAY_ACTION_NO_PROXIES, TRAY_ACTION_QUIT,
    TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY, TRAY_ACTION_ROUTING_BYPASS_PROXY,
    TRAY_ACTION_ROUTING_FOLLOW_RULES, TRAY_ACTION_SELECT_PROXY, TRAY_ACTION_SET_DEFAULT_KERNEL,
    TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, TRAY_ACTION_SHOW, TRAY_ACTION_SYNC_DOWNLOAD,
    TRAY_ACTION_SYNC_UPLOAD, TRAY_ACTION_TOGGLE_AUTOSTART, TRAY_ACTION_TOGGLE_COLOR_MODE,
    TRAY_ACTION_TOGGLE_SYSTEM_PROXY, TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN,
    TRAY_ACTION_UNINSTALL_KERNEL, TRAY_ACTION_UPDATE_ALL_PROFILES, TRAY_MAX_NODES_PER_GROUP,
    TRAY_SUBMENU_INFO, TRAY_SUBMENU_KERNEL, TRAY_SUBMENU_KERNEL_VERSION_BASE,
    TRAY_SUBMENU_PROFILES, TRAY_SUBMENU_PROXIES, TRAY_SUBMENU_PROXY_GROUP_BASE,
    TRAY_SUBMENU_PROXY_MORE_BASE, TRAY_SUBMENU_ROUTING_INTENT, TRAY_SUBMENU_SYNC, TrayActionId,
    TrayCoreStatus, TrayIconData, TrayKernelEntry, TrayMenuCapabilities, TrayMenuContext,
    TrayMenuItem, TrayMenuSpec, TrayOutboundGroup, TrayOutboundNode, TrayProfileEntry, TraySpec,
    encode_pair_payload,
};
use infiltrator_contract::tray_status::{
    TrayIconColorMode, TrayRateBadge, TrayServiceStatus, TraySupport,
};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::inventory::{
    OutboundInventoryState, OutboundSelectionState, SelectionAvailability,
};
use lurker_core_contract::routing_intent::RoutingIntent;

/// Translation closure type shared by the section builders below.
type Tr<'a> = &'a (dyn Fn(&str) -> String + 'a);

/// The i18n key describing a [`TrayCoreStatus`].
pub fn tray_status_key(status: TrayCoreStatus) -> &'static str {
    match status {
        TrayCoreStatus::Running => "tray_status_running",
        TrayCoreStatus::Starting => "tray_status_starting",
        TrayCoreStatus::Stopped => "tray_status_stopped",
        TrayCoreStatus::Error => "tray_status_error",
    }
}

/// The tray status color axis for one core lifecycle state.
///
/// `Starting` is honest uncertainty: the tray shows the connecting tone, not
/// a green "running" mark the core has not earned yet.
pub fn service_status(status: TrayCoreStatus) -> TrayServiceStatus {
    match status {
        TrayCoreStatus::Stopped => TrayServiceStatus::Stopped,
        TrayCoreStatus::Starting => TrayServiceStatus::Connecting,
        TrayCoreStatus::Running => TrayServiceStatus::Running,
        TrayCoreStatus::Error => TrayServiceStatus::Warning,
    }
}

/// The capability this surface reports for the tray it hosts: the backend
/// carries the full menu tree and the live rate badge, in the given style.
pub fn hosted_support(color_mode: TrayIconColorMode) -> TraySupport {
    TraySupport::hosted_with_mode(true, color_mode)
}

/// Render the FlClash-parity tray icon: a status-coloured disc, or the
/// monochrome variant for macOS menu bars and dark taskbars.
///
/// Monochrome keeps the status readable by shape instead of hue: a filled
/// disc (running), a ring (stopped), a disc with a dark core (connecting) and
/// a disc with a warning bar (warning), all in one neutral tone that stays
/// visible on light and dark taskbars.
pub fn generate_tray_icon_rgba(
    status: TrayServiceStatus,
    mode: TrayIconColorMode,
    size: u32,
) -> TrayIconData {
    let size = size.max(1);
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = (size - 1) as f64 / 2.0;
    let radius = center - 1.0;
    let (r, g, b) = status.status_rgb();
    let mono = mode.is_monochrome();
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - center;
            let dy = y as f64 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let inside = distance <= radius;
            let border = distance > radius && distance <= radius + 1.0;
            if inside {
                let (pixel_r, pixel_g, pixel_b, pixel_a) =
                    monochrome_fill(mono, status, dx / radius.max(1.0), dy / radius.max(1.0));
                paint_pixel(&mut rgba, size, x, y, [pixel_r, pixel_g, pixel_b, pixel_a]);
            } else if border {
                let (pixel_r, pixel_g, pixel_b) = if mono { (0, 0, 0) } else { darken(r, g, b) };
                paint_pixel(&mut rgba, size, x, y, [pixel_r, pixel_g, pixel_b, 255]);
            }
        }
    }
    TrayIconData {
        width: size,
        height: size,
        rgba,
    }
}

/// The monochrome fill for one pixel of the status glyph, in relative
/// `(x, y)` coordinates in `[-1, 1]`. Coloured mode paints the status tone.
fn monochrome_fill(mono: bool, status: TrayServiceStatus, nx: f64, ny: f64) -> (u8, u8, u8, u8) {
    if !mono {
        let (r, g, b) = status.status_rgb();
        return (r, g, b, 255);
    }
    let core_radius = 0.55;
    let distance = (nx * nx + ny * ny).sqrt();
    match status {
        TrayServiceStatus::Running => (255, 255, 255, 255),
        TrayServiceStatus::Stopped => (0, 0, 0, 0),
        TrayServiceStatus::Connecting => {
            if distance <= core_radius {
                (255, 255, 255, 64)
            } else {
                (255, 255, 255, 255)
            }
        }
        TrayServiceStatus::Warning => {
            if ny.abs() <= 0.45 && nx.abs() <= 0.12 {
                (0, 0, 0, 255)
            } else {
                (255, 255, 255, 255)
            }
        }
    }
}

fn paint_pixel(rgba: &mut [u8], size: u32, x: u32, y: u32, [r, g, b, a]: [u8; 4]) {
    let stride = size as usize * 4;
    let offset = y as usize * stride + x as usize * 4;
    rgba[offset] = r;
    rgba[offset + 1] = g;
    rgba[offset + 2] = b;
    rgba[offset + 3] = a;
}

fn darken(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (
        (r as u16 * 3 / 4) as u8,
        (g as u16 * 3 / 4) as u8,
        (b as u16 * 3 / 4) as u8,
    )
}

/// Build the localized full-feature tray spec from one owned snapshot.
pub fn build_tray_spec(ctx: &TrayMenuContext, color_mode: TrayIconColorMode) -> TraySpec {
    let lang = Lang(ctx.lang.as_str());
    let tr = move |key: &str| lang.tr(key).into_owned();
    let icon = generate_tray_icon_rgba(service_status(ctx.status), color_mode, 32);

    let mut items = Vec::new();
    items.push(TrayMenuItem::action(
        TRAY_ACTION_SHOW,
        tr("tray_show_window"),
    ));

    let mut routing_items = Vec::new();
    if ctx.capabilities.routing_intent {
        routing_items.push(routing_intent_submenu(ctx, &tr));
    }
    if ctx.capabilities.outbound_selection {
        routing_items.push(proxies_submenu(ctx, &tr));
    }
    if !routing_items.is_empty() {
        items.push(TrayMenuItem::Separator);
        items.extend(routing_items);
    }

    let mut mode_items = Vec::new();
    if ctx.capabilities.system_proxy {
        mode_items.push(TrayMenuItem::checkmark(
            TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
            tr("system_proxy"),
            ctx.system_proxy,
        ));
    }
    if ctx.capabilities.tun {
        mode_items.push(TrayMenuItem::checkmark(
            TRAY_ACTION_TOGGLE_TUN,
            tr("tun_mode"),
            ctx.tun,
        ));
    }
    if ctx.capabilities.theme_toggle {
        mode_items.push(TrayMenuItem::action(
            TRAY_ACTION_TOGGLE_THEME,
            tr("tray_toggle_theme"),
        ));
    }
    if ctx.capabilities.icon_color_mode {
        mode_items.push(TrayMenuItem::checkmark(
            TRAY_ACTION_TOGGLE_COLOR_MODE,
            tr("tray_toggle_color_mode"),
            color_mode.is_monochrome(),
        ));
    }
    if !mode_items.is_empty() {
        items.push(TrayMenuItem::Separator);
        items.extend(mode_items);
    }

    let mut management_items = Vec::new();
    if ctx.capabilities.profile_activation {
        management_items.push(profiles_submenu(ctx, &tr));
    }
    if ctx.capabilities.kernel_management {
        management_items.push(kernel_submenu(ctx, &tr));
    }
    if ctx.capabilities.sync {
        management_items.push(sync_submenu(ctx, &tr));
    }
    if ctx.capabilities.autostart {
        management_items.push(TrayMenuItem::checkmark(
            TRAY_ACTION_TOGGLE_AUTOSTART,
            tr("tray_autostart"),
            ctx.autostart,
        ));
    }
    if !management_items.is_empty() {
        items.push(TrayMenuItem::Separator);
        items.extend(management_items);
    }

    items.push(TrayMenuItem::Separator);
    items.push(info_submenu(ctx, &tr));

    items.push(TrayMenuItem::Separator);
    if ctx.capabilities.factory_reset {
        items.push(TrayMenuItem::action(
            TRAY_ACTION_FACTORY_RESET,
            tr("tray_factory_reset"),
        ));
    }
    items.push(TrayMenuItem::action(TRAY_ACTION_QUIT, tr("tray_quit")));

    TraySpec {
        icon: Some(icon),
        tooltip: tooltip(ctx, &tr, ctx.surface.product_name()),
        menu: TrayMenuSpec { items },
    }
}

/// Localized display text for the current routing intent (`-` when unset).
fn routing_intent_text(ctx: &TrayMenuContext) -> String {
    ctx.routing_intent
        .as_ref()
        .map(|intent| routing_intent_copy(intent, &Lang(ctx.lang.as_str())))
        .unwrap_or_else(|| "-".to_string())
}

/// The default kernel version label (`-` when nothing is installed).
fn default_kernel_version(ctx: &TrayMenuContext) -> Option<&str> {
    ctx.kernels
        .iter()
        .find(|kernel| kernel.is_default)
        .map(|kernel| kernel.version.as_str())
}

/// Localized tooltip carrying the static state lines (routing intent/status/version)
/// plus the shared live rate badge when the host has delivered a real sample.
fn tooltip(ctx: &TrayMenuContext, tr: Tr<'_>, product_name: &'static str) -> String {
    let mut tooltip = format!(
        "{}\n{}: {} · {}: {}\n{}: {}",
        product_name,
        tr("routing_intent_title"),
        routing_intent_text(ctx),
        tr("tray_info_status"),
        tr(tray_status_key(ctx.status)),
        tr("tray_kernel_version"),
        default_kernel_version(ctx).unwrap_or("-"),
    );
    if let Some(badge) = ctx.rate_badge {
        tooltip.push_str(&format!(
            "\n{}: {}",
            tr("tray_info_rate"),
            badge.badge_text()
        ));
    }
    tooltip
}

/// The three common routing choices, with the observed choice marked.
fn routing_intent_submenu(ctx: &TrayMenuContext, tr: Tr<'_>) -> TrayMenuItem {
    let choices = [
        (TRAY_ACTION_ROUTING_FOLLOW_RULES, RoutingIntent::FollowRules),
        (
            TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY,
            RoutingIntent::RouteAllThroughProxy,
        ),
        (TRAY_ACTION_ROUTING_BYPASS_PROXY, RoutingIntent::BypassProxy),
    ];
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_ROUTING_INTENT,
        label: tr("routing_intent_title"),
        enabled: true,
        items: choices
            .into_iter()
            .map(|(id, intent)| {
                let active = ctx.routing_intent.as_ref() == Some(&intent);
                let label = routing_intent_copy(&intent, &Lang(ctx.lang.as_str()));
                TrayMenuItem::Action {
                    id,
                    label: if active {
                        format!("● {label}")
                    } else {
                        label
                    },
                    enabled: true,
                    payload: None,
                }
            })
            .collect(),
    }
}

/// One switchable node entry: `● ` marks the group's active node, the latest
/// measured delay is appended when known, and the payload encodes
/// `group␁node` for [`TRAY_ACTION_SELECT_PROXY`].
fn node_entry(group: &TrayOutboundGroup, node: &TrayOutboundNode) -> TrayMenuItem {
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
fn proxies_submenu(ctx: &TrayMenuContext, tr: Tr<'_>) -> TrayMenuItem {
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
fn group_submenu(index: usize, group: &TrayOutboundGroup, tr: Tr<'_>) -> TrayMenuItem {
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
/// 全部更新订阅， then one auto-update checkmark per profile.
fn profiles_submenu(ctx: &TrayMenuContext, tr: Tr<'_>) -> TrayMenuItem {
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
        if ctx.capabilities.profile_auto_update {
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
    }
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_PROFILES,
        label: tr("tray_profiles"),
        enabled: true,
        items,
    }
}

/// The 内核 submenu: default-version + run-state info lines, one nested
/// submenu per installed version (设为默认 / 卸载， both no-ops on the
/// default), the update entry (morphing into progress + cancel while
/// downloading and a disabled line while checking), and Fake-IP cache flush.
fn kernel_submenu(ctx: &TrayMenuContext, tr: Tr<'_>) -> TrayMenuItem {
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
    items.extend(
        ctx.kernels
            .iter()
            .enumerate()
            .map(|(index, kernel)| TrayMenuItem::Submenu {
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
            }),
    );
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
fn sync_submenu(ctx: &TrayMenuContext, tr: Tr<'_>) -> TrayMenuItem {
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

/// The 信息 submenu: always-disabled state lines (routing intent, run status,
/// controller URL, admin port, kernel version) plus the shared live rate
/// badge while a real traffic sample exists.
fn info_submenu(ctx: &TrayMenuContext, tr: Tr<'_>) -> TrayMenuItem {
    let mut items = vec![
        TrayMenuItem::info(
            TRAY_ACTION_INFO_ROUTING_INTENT,
            format!(
                "{}: {}",
                tr("routing_intent_title"),
                routing_intent_text(ctx)
            ),
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
                ctx.controller.as_deref().unwrap_or("-")
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
    ];
    if let Some(badge) = ctx.rate_badge {
        items.insert(
            0,
            TrayMenuItem::info(
                TRAY_ACTION_INFO_RATE,
                format!("{}: {}", tr("tray_info_rate"), badge.badge_text()),
            ),
        );
    }
    TrayMenuItem::Submenu {
        id: TRAY_SUBMENU_INFO,
        label: tr("tray_info"),
        enabled: true,
        items,
    }
}

/// Project the shared surface snapshot into the owned tray context, so the
/// Bevy and Slint desktop hosts feed the same builder from the same facts.
///
/// Facts the snapshot does not observe (installed kernel set, core download
/// progress, host autostart) stay at their typed "unobserved" defaults: the
/// menu shows the honest placeholders instead of a fabricated value.
pub fn tray_context_from_snapshot(snapshot: &SurfaceSnapshot, lang: &str) -> TrayMenuContext {
    TrayMenuContext {
        surface: snapshot.surface,
        lang: lang.to_string(),
        routing_intent: snapshot.core.routing_intent.current.clone(),
        system_proxy: matches!(snapshot.system_proxy.status, SystemProxyStatus::Enabled),
        tun: snapshot.runtime_control.tun_enabled.unwrap_or(false),
        groups: outbound_groups(snapshot),
        profiles: profiles(snapshot),
        kernels: installed_kernels(snapshot),
        status: core_status(snapshot.core.lifecycle.clone()),
        webdav_enabled: webdav_enabled(snapshot),
        syncing: matches!(sync_status(snapshot), SyncStatus::Syncing),
        // The surface snapshot publishes no controller URL: the host's own
        // runtime port owns that fact, so the info line reads the honest
        // placeholder instead of a guessed endpoint.
        controller: None,
        rate_badge: TrayRateBadge::from_waveform(&snapshot.traffic_waveform),
        capabilities: TrayMenuCapabilities::shared_desktop(),
        ..TrayMenuContext::default()
    }
}

fn core_status(lifecycle: CoreLifecycle) -> TrayCoreStatus {
    match lifecycle {
        CoreLifecycle::Stopped | CoreLifecycle::Stopping => TrayCoreStatus::Stopped,
        CoreLifecycle::Starting => TrayCoreStatus::Starting,
        CoreLifecycle::Ready | CoreLifecycle::Running => TrayCoreStatus::Running,
        CoreLifecycle::Failed => TrayCoreStatus::Error,
    }
}

fn outbound_groups(snapshot: &SurfaceSnapshot) -> Vec<TrayOutboundGroup> {
    let Some(page) = snapshot.outbound_inventory.data.as_ref() else {
        return Vec::new();
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = page.inventory()
    else {
        return Vec::new();
    };
    collections
        .iter()
        .filter(|collection| collection.availability == SelectionAvailability::Selectable)
        .take(5)
        .map(|collection| TrayOutboundGroup {
            name: collection.id.value().to_owned(),
            current: match &collection.selection {
                OutboundSelectionState::Ready { selected } => selected.value().to_owned(),
                OutboundSelectionState::Unknown
                | OutboundSelectionState::Empty
                | OutboundSelectionState::Unsupported
                | OutboundSelectionState::Failed { .. } => String::new(),
            },
            nodes: collection
                .member_ids
                .iter()
                .filter(|member_id| entries.iter().any(|entry| &entry.id == *member_id))
                .map(|member_id| TrayOutboundNode {
                    name: member_id.value().to_owned(),
                    delay_ms: None,
                })
                .collect(),
        })
        .collect()
}

fn profiles(snapshot: &SurfaceSnapshot) -> Vec<TrayProfileEntry> {
    snapshot
        .pages
        .profiles
        .data
        .as_ref()
        .map(profile_entries)
        .unwrap_or_default()
}

/// The tray profile vocabulary from one profiles page readback.
pub fn profile_entries(page: &ProfilesPageSnapshot) -> Vec<TrayProfileEntry> {
    page.profiles
        .iter()
        .map(|profile| TrayProfileEntry {
            name: profile.name.clone(),
            active: profile.is_active,
            auto_update_enabled: profile.auto_update_enabled,
        })
        .collect()
}

fn installed_kernels(snapshot: &SurfaceSnapshot) -> Vec<TrayKernelEntry> {
    let rollback = &snapshot.versions.rollback;
    let Some(current) = rollback.current.as_ref() else {
        return Vec::new();
    };
    let mut kernels = vec![TrayKernelEntry {
        version: current.clone(),
        is_default: true,
    }];
    for version in &rollback.history {
        if version != current {
            kernels.push(TrayKernelEntry {
                version: version.clone(),
                is_default: false,
            });
        }
    }
    kernels
}

fn webdav_enabled(snapshot: &SurfaceSnapshot) -> bool {
    snapshot
        .pages
        .sync
        .data
        .as_ref()
        .is_some_and(|page| !page.server_url.is_empty())
}

fn sync_status(snapshot: &SurfaceSnapshot) -> SyncStatus {
    snapshot
        .pages
        .sync
        .data
        .as_ref()
        .map(|page| page.status)
        .unwrap_or(SyncStatus::Unknown)
}

#[cfg(test)]
#[path = "tray_projection_tests.rs"]
mod tests;
