//! Projection of the shared product snapshot into the imported Lumen widgets.
//!
//! This module is deliberately read-only: every value shown here comes from
//! `SurfaceSnapshot`, the shared presentation projectors, or an explicit
//! capability/status result. It does not recreate Lumen's former service
//! store.

pub(super) mod collections;
pub(super) mod lan;
pub(super) mod workbench;

use super::flag_assets;
use super::slint_model;
use super::{AppWindow, OverrideCard, OverviewCardSlot, ProfileCard, ProfileOverrideChoice};
use crate::surface::workbenches::Workbenches;
use crate::theme_util::apply_theme_color;
use crate::widget_vocabulary;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::overview_layout::{OverviewCardKind, OverviewLayoutSnapshot};
use infiltrator_contract::profile_editor_read::ProfileReadStatus;
use infiltrator_contract::public_ip::PublicIpProbeStatus;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use infiltrator_presentation::overview_page_model::{
    OverviewHost, OverviewNetworkStatus, OverviewPageModel,
};
use infiltrator_presentation::profile_preview::redact_profile_preview;
use infiltrator_presentation::shell_readout::profile_name;
use infiltrator_presentation::subscription_quota_projection::{QuotaGrade, project_quota};
use infiltrator_presentation::traffic_readout::{TrafficFreshness, project_traffic_readout};
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language, resolve_language_code};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use slint::ComponentHandle;
use std::env;

pub(super) const THIRD_PARTY_NOTICES: &str = include_str!("../../../THIRD-PARTY-NOTICES.md");

pub(super) fn project_initial(window: &AppWindow, language: &str, autostart: Option<bool>) {
    let lang = Lang(language);
    window.set_product_version(infiltrator_contract::ProductVersion::SEMVER.into());
    window.set_application_language("system".into());
    window.set_system_language(system_language_for_ui());
    window.set_application_dark_mode(false);
    window.set_auto_start(autostart.unwrap_or(false));
    window.set_active_profile(lang.tr("profile_no_active").into_owned().into());
    window.set_home_current_node(lang.tr("unknown").into_owned().into());
    window.set_home_current_accessible(lang.tr("unknown").into_owned().into());
    window.set_core_version(lang.tr("quota_value_unobserved").into_owned().into());
    window.set_runtime_detail(lang.tr("runtime_detail_waiting").into_owned().into());
    window.set_license_catalog_error("".into());
    window.set_license_rows(slint_model(collections::license_rows("", language)));
    window.set_stat_total_down("—".into());
    window.set_stat_total_up("—".into());
}

pub(super) fn snapshot_language(snapshot: &SurfaceSnapshot) -> String {
    if let Some(preference) = snapshot.language_settings.preference {
        return resolve_language_code(preference.as_setting());
    }
    env::var("INFILTRATOR_LANG")
        .map(|value| resolve_language_code(&value))
        .unwrap_or_else(|_| resolve_language_code("system"))
}

fn language_preference_for_ui(
    preference: Option<LanguagePreference>,
    fallback: &str,
) -> slint::SharedString {
    let setting = preference
        .map(LanguagePreference::as_setting)
        .unwrap_or(fallback);
    match LanguagePreference::parse(setting) {
        Ok(LanguagePreference::System) => "system",
        Ok(LanguagePreference::SimplifiedChinese) => "zh-CN",
        Ok(LanguagePreference::English) => "en",
        Err(_) => "system",
    }
    .into()
}

fn system_language_for_ui() -> slint::SharedString {
    if get_system_language().starts_with("en") {
        "en".into()
    } else {
        "zh-CN".into()
    }
}

fn public_ip_status_copy(snapshot: &SurfaceSnapshot, language: &str) -> Option<String> {
    let lang = Lang(language);
    Some(
        match snapshot.public_ip.status {
            PublicIpProbeStatus::Unknown => lang.tr("public_ip_unobserved"),
            PublicIpProbeStatus::Ready => lang.tr("public_ip_ready"),
            PublicIpProbeStatus::Probing => lang.tr("public_ip_probing"),
            PublicIpProbeStatus::Empty => lang.tr("public_ip_empty"),
            PublicIpProbeStatus::Unsupported => lang.tr("public_ip_unsupported"),
            PublicIpProbeStatus::Failed => lang.tr("public_ip_failed"),
        }
        .into_owned(),
    )
}

pub(super) fn project_snapshot(
    window: &AppWindow,
    snapshot: &SurfaceSnapshot,
    autostart: Option<bool>,
) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let page = window.get_page().to_string();
    window.set_snapshot_window_warning(snapshot_window_warning(snapshot, &page).into());
    let mut overview = OverviewPageModel::default();
    overview.observe(overview_host(), snapshot);
    let running = overview.is_running();
    let profile = profile_name(&snapshot.shell_readout, &lang);
    let current_profile = snapshot.shell_readout.profile.current_value().is_some();
    let active_node = if snapshot.active_exit.is_drawable() {
        snapshot
            .active_exit
            .name
            .clone()
            .unwrap_or_else(|| lang.tr("未知").into_owned())
    } else {
        lang.tr("未知").into_owned()
    };
    let traffic =
        project_traffic_readout(&snapshot.shell_readout, &snapshot.traffic_waveform, &lang);

    window.set_runtime_status(widget_vocabulary::runtime_status(
        snapshot.core.lifecycle.clone(),
    ));
    let runtime_detail = snapshot
        .core
        .failure
        .as_ref()
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            lang.tr(match &snapshot.core.lifecycle {
                CoreLifecycle::Starting => "runtime_detail_changing",
                CoreLifecycle::Ready | CoreLifecycle::Running => "runtime_detail_active",
                CoreLifecycle::Stopping => "runtime_detail_changing",
                CoreLifecycle::Stopped => "runtime_detail_stopped",
                CoreLifecycle::Failed => "runtime_detail_failed",
            })
            .into_owned()
        });
    window.set_runtime_detail(runtime_detail.into());
    window.set_active_profile(profile.into());
    window.set_has_current_profile(current_profile);
    window.set_route_mode(widget_vocabulary::route_mode(overview.routing_mode()));
    window.set_network_mode(widget_vocabulary::network_mode(overview.network_mode()));
    window.set_network_mode_status(network_status_copy(overview.network_status(), &lang).into());
    window.set_network_mode_status_warning(overview.network_warning().is_warned());
    let tun_availability = snapshot.capabilities.availability(Capability::Tun);
    window.set_tun_capable(snapshot.capabilities.supports(Capability::Tun));
    let tun_detail = match tun_availability {
        Availability::Supported => lang.tr("宿主支持 TUN").into_owned(),
        Availability::Experimental => lang.tr("TUN 为实验性能力").into_owned(),
        Availability::Unsupported { reason } | Availability::Unavailable { reason } => reason,
    };
    window.set_tun_capability_detail(tun_detail.into());
    window.set_home_current_node(active_node.clone().into());
    window.set_home_current_accessible(active_node.into());
    let active_exit_country = if snapshot.active_exit.is_drawable() {
        snapshot
            .active_exit
            .country_code
            .as_deref()
            .unwrap_or_default()
    } else {
        ""
    };
    window.set_home_current_flag_code(active_exit_country.to_ascii_lowercase().into());
    window.set_home_current_flag_index(
        flag_assets::index_for_code(active_exit_country).unwrap_or(-1),
    );
    window.set_home_current_delay(
        if snapshot.active_exit.is_drawable() {
            snapshot
                .active_exit
                .delay_ms
                .map(|delay| format!("{delay} ms"))
                .unwrap_or_else(|| "—".to_owned())
        } else {
            "—".to_owned()
        }
        .into(),
    );
    let public_ip_drawable = snapshot.public_ip.is_drawable();
    window.set_home_exit_ip(if public_ip_drawable {
        snapshot
            .public_ip
            .ip
            .clone()
            .unwrap_or_else(|| "—".to_owned())
            .into()
    } else {
        "—".into()
    });
    window.set_home_exit_country_code(
        snapshot
            .public_ip
            .country_code
            .as_deref()
            .filter(|_| public_ip_drawable)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .into(),
    );
    window.set_home_exit_flag_index(
        snapshot
            .public_ip
            .country_code
            .as_deref()
            .filter(|_| public_ip_drawable)
            .and_then(flag_assets::index_for_code)
            .unwrap_or(-1),
    );
    window.set_home_exit_status(
        snapshot
            .public_ip
            .failure
            .clone()
            .or_else(|| public_ip_status_copy(snapshot, &language))
            .unwrap_or_default()
            .into(),
    );
    window.set_core_version(
        snapshot
            .core
            .core_version
            .clone()
            .unwrap_or_else(|| lang.tr("quota_value_unobserved").into_owned())
            .into(),
    );
    window.set_stat_down(traffic.download.clone().into());
    window.set_stat_up(traffic.upload.clone().into());
    let (down_value, down_unit) = collections::split_rate(&traffic.download);
    window.set_stat_down_value(down_value.into());
    window.set_stat_down_unit(down_unit.into());
    window.set_stat_up_compact(traffic.upload.clone().into());
    window.set_stat_connections(snapshot.core.active_connections.to_string().into());
    window.set_traffic_stream_status(
        match traffic.freshness {
            TrafficFreshness::Current => lang.tr("已连接").into_owned(),
            TrafficFreshness::Stale => lang.tr("正在重连").into_owned(),
            TrafficFreshness::Failed => lang.tr("读取失败").into_owned(),
            TrafficFreshness::Unobserved => {
                if running {
                    lang.tr("正在连接").into_owned()
                } else {
                    lang.tr("未运行").into_owned()
                }
            }
        }
        .into(),
    );
    window.set_home_traffic_has_data(snapshot.traffic_waveform.is_drawable());
    window.set_stat_total_down("—".into());
    window.set_stat_total_up("—".into());
    if snapshot.failure.is_some() || collections::page_failure(window, snapshot).is_some() {
        window.set_notice(
            snapshot
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
                .or_else(|| collections::page_failure(window, snapshot))
                .unwrap_or_default()
                .into(),
        );
    }

    project_settings(window, snapshot, autostart);
    project_overview_layout(window, snapshot, &language);
    lan::project_lan_sharing(window, snapshot, &language);
    project_profiles(window, snapshot, &language);
    collections::project_proxies(window, snapshot);
    collections::project_connections(window, snapshot);
    collections::project_rules_and_providers(window, snapshot);
    collections::project_logs(window, snapshot);
    collections::project_traffic(window, snapshot);
    collections::project_license_catalog(window, &language);
    collections::project_connection_detail(
        window,
        snapshot,
        window.get_connection_detail_id().as_str(),
    );
    let preview_profile = window.get_content_preview_profile().to_string();
    if !preview_profile.is_empty() {
        if snapshot.profile_editor.read.profile.as_deref() != Some(preview_profile.as_str()) {
            window.set_content_preview(lang.tr("正在读取配置文档").into_owned().into());
        } else {
            match &snapshot.profile_editor.read.document {
                ProfileReadStatus::Ready => {
                    if let Some(document) = snapshot.profile_editor.document.as_ref() {
                        window
                            .set_content_preview(redact_profile_preview(&document.content).into());
                    } else {
                        window.set_content_preview("".into());
                    }
                }
                ProfileReadStatus::Failed(failure) => {
                    window.set_content_preview("".into());
                    window.set_notice(failure.message.clone().into());
                }
                ProfileReadStatus::Loading => {
                    window.set_content_preview(lang.tr("正在读取配置文档").into_owned().into());
                }
                ProfileReadStatus::Unobserved => window.set_content_preview("".into()),
            }
        }
    }
}

/// Projects the workbench pages from the shared owner snapshot.
///
/// The workbench reducers live in the shared surface state, so this
/// projection is separate from the page projection the other surfaces share.
pub(super) fn project_workbench(
    window: &AppWindow,
    snapshot: &SurfaceSnapshot,
    workbenches: &Workbenches,
) {
    let language = snapshot_language(snapshot);
    workbench::project(window, snapshot, &language, workbenches);
}

pub(super) fn snapshot_window_warning(snapshot: &SurfaceSnapshot, page: &str) -> String {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let window = match page {
        "Connections" => snapshot.pages.connections.data.as_ref().and_then(|page| {
            (page.total_connections > page.connections.len()).then_some((
                page.connections.len(),
                page.total_connections,
                "active connections",
            ))
        }),
        "Rules" => snapshot.pages.rules.data.as_ref().and_then(|page| {
            page.is_truncated()
                .then_some((page.rules.len(), page.total_rules, "runtime rules"))
        }),
        "Logs" => snapshot.pages.logs.data.as_ref().and_then(|page| {
            (page.total_entries > page.entries.len()).then_some((
                page.entries.len(),
                page.total_entries,
                "log entries",
            ))
        }),
        _ => None,
    };
    let Some((shown, total, item_key)) = window else {
        return String::new();
    };
    let mut copy = lang
        .tr("The shared owner publishes a bounded window: {shown} of {total} {items}.")
        .into_owned();
    copy = copy.replace("{shown}", &shown.to_string());
    copy = copy.replace("{total}", &total.to_string());
    copy = copy.replace("{items}", &lang.tr(item_key));
    copy
}

/// The host the overview page renders for.
///
/// The traffic mode depends on the host, and the host is a build fact of the
/// surface, so the shared model is told which one it renders for.
fn overview_host() -> OverviewHost {
    if cfg!(target_os = "android") {
        OverviewHost::Android
    } else {
        OverviewHost::Desktop
    }
}

/// The routing mode label the widget names.
///
/// The routing mode itself is the shared model's; only the vocabulary the
/// The traffic mode label the widget names.
///
/// The traffic mode itself is the shared model's; only the vocabulary the
/// imported buttons speak belongs to the surface.
/// The localized copy of the typed traffic mode status.
///
/// The status is the shared model's; only the wording belongs to the surface.
/// A status that carries a reason shows that reason, because the owner said
/// why the mode is not confirmed.
fn network_status_copy(status: &OverviewNetworkStatus, lang: &Lang<'_>) -> String {
    match status {
        OverviewNetworkStatus::VpnConnected => lang.tr("VPN 已连接").into_owned(),
        OverviewNetworkStatus::VpnStarting => lang.tr("VPN 正在启动").into_owned(),
        OverviewNetworkStatus::VpnStopping => lang.tr("VPN 正在停止").into_owned(),
        OverviewNetworkStatus::VpnStopped => lang.tr("VPN 未运行").into_owned(),
        OverviewNetworkStatus::VpnPermissionRequired => lang.tr("等待 VPN 授权").into_owned(),
        OverviewNetworkStatus::VpnRevoked => lang.tr("VPN 授权已撤销").into_owned(),
        OverviewNetworkStatus::VpnFailed { reason }
        | OverviewNetworkStatus::VpnUnavailable { reason }
        | OverviewNetworkStatus::VpnUnsupported { reason }
        | OverviewNetworkStatus::SystemProxyUnavailable { reason } => reason.clone(),
        OverviewNetworkStatus::VpnNotApplicable => lang.tr("VPN 未运行").into_owned(),
        OverviewNetworkStatus::TunEnabled => lang.tr("TUN 已启用").into_owned(),
        OverviewNetworkStatus::SystemProxyEnabled => lang.tr("系统代理已启用").into_owned(),
        OverviewNetworkStatus::SystemProxyDisabled => lang.tr("系统代理未启用").into_owned(),
        OverviewNetworkStatus::SystemProxyUnobserved => lang.tr("系统代理状态未观测").into_owned(),
        OverviewNetworkStatus::Unknown => lang.tr("状态未知").into_owned(),
    }
}

/// Projects the shared Overview card order into the card list of the surface.
///
/// The list is the owner's readback: the surface renders the order the owner
/// published and keeps no order of its own, so a refused or unanswered move
/// leaves the list exactly as the owner published it.
fn project_overview_layout(window: &AppWindow, snapshot: &SurfaceSnapshot, language: &str) {
    let lang = Lang(language);
    let order = published_overview_order(&snapshot.overview_layout);
    let total = order.len();
    let rows = order
        .iter()
        .enumerate()
        .map(|(index, kind)| OverviewCardSlot {
            id: kind.as_str().into(),
            title: overview_card_title(*kind, &lang).into(),
            // The owner owns the order, so a card can only move where the
            // owner's own order allows it to move.
            can_move_up: index > 0,
            can_move_down: index + 1 < total,
        })
        .collect::<Vec<_>>();
    window.set_overview_cards(slint_model(rows));
}

/// The card order the shared owner published.
///
/// The owner's order comes first; a card the owner did not list is appended
/// in the contract's default order, so every card is rendered exactly once
/// and no card disappears because a layout listed only some of them.
fn published_overview_order(layout: &OverviewLayoutSnapshot) -> Vec<OverviewCardKind> {
    let mut order: Vec<OverviewCardKind> =
        Vec::with_capacity(OverviewCardKind::DEFAULT_ORDER.len());
    for kind in &layout.order {
        if !order.contains(kind) {
            order.push(*kind);
        }
    }
    for kind in OverviewCardKind::DEFAULT_ORDER {
        if !order.contains(&kind) {
            order.push(kind);
        }
    }
    order
}

/// The title the surface names one shared Overview card with.
///
/// The card identity is the shared contract's; only the wording belongs to
/// the surface. Every title is a copy the shared product catalog already
/// carries, so the card list reads the same in every locale.
fn overview_card_title(kind: OverviewCardKind, lang: &Lang<'_>) -> String {
    match kind {
        OverviewCardKind::RoutingIntentSegment => lang.tr("代理模式").into_owned(),
        OverviewCardKind::Traffic => lang.tr("实时流量").into_owned(),
        OverviewCardKind::Metrics => lang.tr("运行概况").into_owned(),
        OverviewCardKind::MasterSwitches => lang.tr("网络接管").into_owned(),
        OverviewCardKind::ActiveExit => lang.tr("当前节点").into_owned(),
        OverviewCardKind::PublicIp => lang.tr("出口 IP").into_owned(),
        OverviewCardKind::Topology => lang.tr("代理链路").into_owned(),
        OverviewCardKind::Quota => lang.tr("订阅信息").into_owned(),
    }
}

fn project_settings(window: &AppWindow, snapshot: &SurfaceSnapshot, autostart: Option<bool>) {
    let Some(settings) = snapshot.pages.settings.data.as_ref() else {
        return;
    };
    window.set_auto_start(autostart.unwrap_or(settings.autostart));
    window.set_application_language(language_preference_for_ui(
        snapshot.language_settings.preference,
        &settings.language,
    ));
    window.set_system_language(system_language_for_ui());
    let theme = ThemePreference::from_setting(&settings.theme);
    let mode = match theme {
        ThemePreference::System => "system",
        ThemePreference::Fixed(ThemeSkin::Light) => "light",
        ThemePreference::Fixed(ThemeSkin::Dark) => "dark",
        ThemePreference::Fixed(ThemeSkin::Forest) => "forest",
        ThemePreference::Fixed(ThemeSkin::Amoled) => "amoled",
    };
    window.set_theme_mode(widget_vocabulary::theme_mode(mode));
    match theme {
        ThemePreference::System => {}
        ThemePreference::Fixed(skin) => window.set_application_dark_mode(skin.is_dark()),
    }
    window.set_refresh_current_profile_on_start(settings.refresh_current_profile_on_start);
    if let Some(color_hex) = &settings.custom_accent_color {
        apply_theme_color(&window.as_weak(), color_hex);
    }
}

fn project_profiles(window: &AppWindow, snapshot: &SurfaceSnapshot, language: &str) {
    let lang = Lang(language);
    let Some(page) = snapshot.pages.profiles.data.as_ref() else {
        window.set_profiles(slint_model(Vec::<ProfileCard>::new()));
        window.set_user_agent("".into());
        window.set_user_agent_draft("".into());
        window.set_overrides(slint_model(Vec::<OverrideCard>::new()));
        window.set_profile_settings_overrides(slint_model(Vec::<ProfileOverrideChoice>::new()));
        return;
    };
    let total_profiles = page.profiles.len();
    let quota = &snapshot.subscription_quota;
    let quota_drawable = quota.is_drawable();
    let quota_pres = if quota_drawable {
        Some(project_quota(quota, &lang))
    } else {
        None
    };
    let rows = page
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let (has_quota, used_text, total_text, expire_text, usage_fraction, quota_grade) =
                if profile.is_active && quota_drawable {
                    let pres = quota_pres.as_ref().unwrap();
                    let grade_str = match pres.grade {
                        QuotaGrade::Danger => "danger",
                        QuotaGrade::Warning => "warning",
                        _ => "observed",
                    };
                    (
                        true,
                        pres.used.clone(),
                        pres.total.clone(),
                        pres.expiry.clone(),
                        pres.fraction.unwrap_or(0.0),
                        grade_str,
                    )
                } else if let Some(total) = profile.total_bytes {
                    let used =
                        profile.upload_bytes.unwrap_or(0) + profile.download_bytes.unwrap_or(0);
                    let fraction = if total > 0 {
                        (used as f32 / total as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let grade_str = if fraction >= 0.9 {
                        "danger"
                    } else if fraction >= 0.75 {
                        "warning"
                    } else {
                        "observed"
                    };
                    (
                        true,
                        format_bytes(used),
                        format_bytes(total),
                        String::new(),
                        fraction,
                        grade_str,
                    )
                } else {
                    (
                        false,
                        String::new(),
                        String::new(),
                        String::new(),
                        0.0,
                        "observed",
                    )
                };
            ProfileCard {
                id: profile.id.clone().into(),
                name: profile.name.clone().into(),
                kind: if profile.write_protection == ProfileWriteProtection::RemoteSubscription {
                    lang.tr("订阅链接").into_owned().into()
                } else {
                    lang.tr("本地 YAML").into_owned().into()
                },
                location: if profile.write_protection.is_protected() {
                    lang.tr("订阅配置").into_owned().into()
                } else {
                    lang.tr("本地配置文件").into_owned().into()
                },
                updated: profile.updated_at.clone().into(),
                current: profile.is_active,
                can_move_up: index > 0,
                can_move_down: index + 1 < total_profiles,
                has_quota,
                used_text: used_text.into(),
                total_text: total_text.into(),
                expire_text: expire_text.into(),
                usage_fraction,
                quota_grade: quota_grade.into(),
            }
        })
        .collect::<Vec<_>>();
    window.set_profiles(slint_model(rows));
    window.set_user_agent(
        page.profiles
            .iter()
            .find(|profile| profile.is_active)
            .map(|profile| profile.user_agent.as_str())
            .unwrap_or_default()
            .into(),
    );
    window.set_user_agent_draft(
        page.profiles
            .iter()
            .find(|profile| profile.is_active)
            .map(|profile| profile.user_agent.as_str())
            .unwrap_or_default()
            .into(),
    );

    let override_cards: Vec<OverrideCard> = page
        .named_overrides
        .overrides
        .iter()
        .map(|item| OverrideCard {
            id: item.id.as_str().into(),
            name: item.name.clone().into(),
            kind: "yaml".into(),
            format: "YAML".into(),
            bound_count: page
                .named_overrides
                .bound_profile_count(&item.id)
                .to_string()
                .into(),
        })
        .collect();
    window.set_overrides(slint_model(override_cards));

    let active_profile_id = page
        .profiles
        .iter()
        .find(|p| p.is_active)
        .map(|p| p.id.as_str())
        .unwrap_or_default();
    let bindings = page.named_overrides.bindings_for_profile(active_profile_id);
    let total_bindings = bindings.len();
    let choices: Vec<ProfileOverrideChoice> = bindings
        .iter()
        .enumerate()
        .filter_map(|(idx, b)| {
            let item = page.named_overrides.get_override(&b.override_id)?;
            Some(ProfileOverrideChoice {
                id: item.id.as_str().into(),
                name: item.name.clone().into(),
                format: "YAML".into(),
                selected: b.enabled,
                can_move_up: idx > 0,
                can_move_down: idx + 1 < total_bindings,
            })
        })
        .collect();
    window.set_profile_settings_overrides(slint_model(choices));
}
