//! The Overview page's projection-restamp seam: the typed markers and value
//! projections for the card texts, the single refresh observer that restamps
//! them in place, and the per-frame palette reskin.

#[path = "overview_restamp_query_access.rs"]
pub mod query_access;
use self::query_access::{OverviewDynamicTextItem, OverviewProjectionTargets};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::history::{TrafficHistory, chart_inputs};
use crate::pages::overview::{
    AccentContainerFill, AccentFill, BorderFill, LastOverviewProjection, OnAccentText,
    OverviewCardState, OverviewChipKind, OverviewLineKind, OverviewModeChip,
    OverviewProjectionUpdated, StatusDot, SurfaceElevatedFill, SurfaceFill, banner_note, card_fill,
    chart_dims, chip_label, format_cpu, format_memory, format_scale, format_total_traffic,
    state_ink, status_dot_color,
};
use crate::pages::overview_cards::quota_status_color;
use crate::pages::overview_public_ip::{PublicIpTextKind, public_ip_text_value};
use crate::pages::overview_topology::{topology_spec, topology_text_value};
use crate::surface::LatestSurfaceSnapshot;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, QueryData};
use bevy::ecs::system::SystemParam;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, Display, percent};
use bevy::ui_widgets::Activate;
use infiltrator_application::core_status_projection::{failure_copy, lifecycle_copy};
use infiltrator_application::proxy_mode_projection::mode_status_copy;
use infiltrator_application::shell_readout_projection::{rate_copy, rate_status};
use infiltrator_application::subscription_quota_projection::{QuotaPresentation, project_quota};
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_application::system_toggle_projection::{action_label, status_label};
use infiltrator_bevy_widgets::chart::ChartSpec;
use infiltrator_bevy_widgets::chart::bezier::ScaleMode;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::active_exit::{ActiveExitSnapshot, ActiveExitStatus};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::public_ip::PublicIpProbeStatus;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot, SystemToggleState};

/// Marker on the high-fidelity active-exit card's mutable facts.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveExitText(pub ActiveExitTextKind);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveExitTextKind {
    #[default]
    Flag,
    Name,
    Protocol,
    Delay,
    Group,
    Status,
}

/// Marker on mutable subscription-quota dashboard text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionQuotaText(pub SubscriptionQuotaTextKind);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubscriptionQuotaTextKind {
    #[default]
    Profile,
    Expiry,
    Metrics,
    Reset,
    Status,
}

/// Marker on the quota progress fill whose width follows the shared usage
/// fraction.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionQuotaProgress;

/// Mutable status/action text for one Overview master switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewMasterSwitchText {
    pub toggle: SystemToggle,
    pub kind: OverviewMasterSwitchTextKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewMasterSwitchTextKind {
    #[default]
    Status,
    Action,
}

/// Button target for the large Overview system proxy/TUN controls.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewMasterSwitchButton {
    pub toggle: SystemToggle,
    pub enabled: bool,
    pub can_toggle: bool,
}

/// Marker on the subscription quota card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionQuotaCard;

pub(crate) fn active_exit_text_value(
    snapshot: &ActiveExitSnapshot,
    kind: ActiveExitTextKind,
) -> String {
    match kind {
        ActiveExitTextKind::Flag => active_exit_flag(snapshot.country_code.as_deref()).to_owned(),
        ActiveExitTextKind::Name => snapshot.name.clone().unwrap_or_else(|| "—".to_owned()),
        ActiveExitTextKind::Protocol => snapshot
            .protocol
            .clone()
            .unwrap_or_else(|| "not reported".to_owned()),
        ActiveExitTextKind::Delay => snapshot
            .delay_ms
            .map(|delay| format!("{delay} ms"))
            .unwrap_or_else(|| "—".to_owned()),
        ActiveExitTextKind::Group => snapshot
            .group
            .as_ref()
            .map(|group| format!("group · {group}"))
            .unwrap_or_else(|| "group · not reported".to_owned()),
        ActiveExitTextKind::Status => match snapshot.status {
            ActiveExitStatus::Ready => match snapshot.alive {
                Some(true) => "selected · alive".to_owned(),
                Some(false) => "selected · offline".to_owned(),
                None => "selected · liveness unknown".to_owned(),
            },
            ActiveExitStatus::Empty => "no active exit".to_owned(),
            ActiveExitStatus::Unknown => "active exit pending".to_owned(),
            ActiveExitStatus::Unsupported => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "active exit unavailable".to_owned()),
            ActiveExitStatus::Failed => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "active exit read failed".to_owned()),
        },
    }
}

fn active_exit_flag(country_code: Option<&str>) -> &'static str {
    match country_code
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "HK" => "🇭🇰",
        "TW" => "🇹🇼",
        "JP" => "🇯🇵",
        "US" => "🇺🇸",
        "SG" => "🇸🇬",
        "KR" => "🇰🇷",
        "GB" => "🇬🇧",
        "DE" => "🇩🇪",
        "FR" => "🇫🇷",
        "CA" => "🇨🇦",
        "AU" => "🇦🇺",
        "RU" => "🇷🇺",
        "IN" => "🇮🇳",
        "NL" => "🇳🇱",
        "BR" => "🇧🇷",
        "TR" => "🇹🇷",
        "AR" => "🇦🇷",
        "PH" => "🇵🇭",
        "TH" => "🇹🇭",
        "MY" => "🇲🇾",
        "VN" => "🇻🇳",
        "AE" => "🇦🇪",
        "CN" => "🇨🇳",
        "DIRECT" => "⚡",
        "REJECT" => "🚫",
        _ => "🌐",
    }
}

pub(crate) fn subscription_quota_text_value(
    presentation: &QuotaPresentation,
    kind: SubscriptionQuotaTextKind,
) -> String {
    match kind {
        SubscriptionQuotaTextKind::Profile => presentation.profile.clone(),
        SubscriptionQuotaTextKind::Expiry => presentation.expiry.clone(),
        SubscriptionQuotaTextKind::Metrics => presentation.metrics.clone(),
        SubscriptionQuotaTextKind::Reset => presentation.reset.clone(),
        SubscriptionQuotaTextKind::Status => presentation.status.clone(),
    }
}

pub(crate) fn master_switch_text_value(
    snapshot: &SystemToggleSnapshot,
    toggle: SystemToggle,
    kind: OverviewMasterSwitchTextKind,
    locale: &str,
) -> String {
    match kind {
        OverviewMasterSwitchTextKind::Status => status_label(snapshot.state(toggle), locale),
        OverviewMasterSwitchTextKind::Action => action_label(snapshot.state(toggle), locale),
    }
}

pub(crate) fn master_switch_status_color(
    snapshot: &SystemToggleSnapshot,
    toggle: SystemToggle,
    palette: &UiPalette,
) -> Color {
    match snapshot.state(toggle) {
        SystemToggleState::Enabled => palette.success,
        SystemToggleState::Pending { .. } => palette.warning,
        SystemToggleState::Disabled => palette.ink_dim,
        SystemToggleState::Unknown
        | SystemToggleState::Unsupported { .. }
        | SystemToggleState::Failed { .. } => palette.danger,
    }
}

pub(crate) fn on_overview_master_switch_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewMasterSwitchButton>,
    latest: Res<LatestSurfaceSnapshot>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    if !button.can_toggle {
        return;
    }
    let shared = SystemToggleApplication::from_surface(&latest.0);
    let desired = !button.enabled;
    let Ok(intent) = SystemToggleApplication::intent(&shared, button.toggle, desired) else {
        return;
    };
    let command = match intent {
        CommandIntent::SetSystemProxy { enabled } => UiCommand::SetSystemProxy { enabled },
        CommandIntent::ToggleTun { enabled } => UiCommand::ToggleTun { enabled },
        _ => return,
    };
    handle.submit(command);
}

#[derive(SystemParam)]
pub struct OverviewAppearance<'w> {
    palette: Res<'w, UiPalette>,
    locale: Res<'w, UiLocale>,
}

/// The page's only data-refresh path: restamp texts, inks, pill
/// selection, the banner's stored state, the semantic labels, the chip
/// values and the trend chart's plate from the carried projection, and
/// mirror the projection into [`LastOverviewProjection`] (the theme
/// switch's replay source). The chart series re-derive through
/// [`chart_series`] — the synthetic fixture trend for a demo projection,
/// the pump-recorded ring (appended at the drain, `controller.rs`) for a
/// live one — and restamp as a compare-and-set component swap, which the
/// widget layer's `sync_charts` rasterizes into the *same* image handle.
/// Structurally inert — no spawn, no despawn, no tree rebuild.
pub(crate) fn apply_overview_projection(
    update: On<OverviewProjectionUpdated>,
    appearance: OverviewAppearance,
    history: Res<TrafficHistory>,
    mut last: ResMut<LastOverviewProjection>,
    targets: OverviewProjectionTargets,
) {
    let OverviewProjectionTargets {
        mut lines,
        mut values,
        mut dynamic_texts,
        mut quota_progress,
        mut master_buttons,
        mut topology_buttons,
        mut cards,
        mut chips,
        mut reload_masks,
        mut reload_mask_texts,
        groups,
        mut charts,
        mut topology_charts,
    } = targets;

    let projection = &update.0;
    let palette = &appearance.palette;
    let language = appearance.locale.code();
    let quota_presentation = project_quota(&projection.subscription_quota, language);
    for (mut text, mut ink, line, semantic) in &mut lines {
        match line.0 {
            OverviewLineKind::State => {
                text.0 = lifecycle_copy(&projection.lifecycle, language);
                ink.0 = state_ink(&projection.lifecycle, palette);
                if let Some(mut node) = semantic {
                    node.0
                        .set_label(lifecycle_copy(&projection.lifecycle, language));
                }
            }
            OverviewLineKind::Upload => {
                text.0 = format!("↑ {}", rate_copy(&projection.readout.upload_bps, language));
                ink.0 = palette.success;
            }
            OverviewLineKind::Download => {
                text.0 = format!(
                    "↓ {}",
                    rate_copy(&projection.readout.download_bps, language)
                );
            }
            OverviewLineKind::TelemetryFailure => {
                text.0 = rate_status(&projection.readout, language)
            }
            OverviewLineKind::Failure => {
                text.0 = failure_copy(
                    &projection.lifecycle,
                    projection.failure.as_deref(),
                    language,
                )
            }
            OverviewLineKind::ModeChip => {
                text.0 = mode_status_copy(&projection.proxy_mode, language)
            }
            OverviewLineKind::BannerNote => text.0 = banner_note(projection, language),
            OverviewLineKind::Scale => text.0 = format_scale(&projection.traffic_scale),
        }
    }
    for mut card in &mut cards {
        card.0 = Some(projection.lifecycle.clone());
    }
    for (chip_entity, chip, semantic) in &mut chips {
        let value = match chip.0 {
            OverviewChipKind::Connections => projection.active_connections.to_string(),
            OverviewChipKind::Memory => format_memory(projection.memory_bytes),
            OverviewChipKind::Cpu => format_cpu(projection.cpu_percent),
            OverviewChipKind::Upload => rate_copy(&projection.readout.upload_bps, language),
            OverviewChipKind::Download => rate_copy(&projection.readout.download_bps, language),
            OverviewChipKind::TotalTraffic => format_total_traffic(projection.total_traffic_bytes),
        };
        if let Some(mut node) = semantic {
            node.0
                .set_label(format!("{} {value}", chip_label(chip.0, language)));
        }
        for descendant in groups.iter_descendants(chip_entity) {
            if let Ok(mut text) = values.get_mut(descendant) {
                text.0 = value.clone();
            }
        }
    }
    for OverviewDynamicTextItem {
        mut text,
        mut ink,
        topology,
        exit,
        quota,
        master,
        public_ip,
    } in &mut dynamic_texts
    {
        // Each branch keeps the exclusions of its original native query. A
        // shared entity with conflicting roles cannot become a new write target.
        if public_ip.is_none() {
            if let Some(marker) = topology {
                let value = topology_text_value(&projection.traffic_topology, marker, language);
                if text.0 != value {
                    text.0 = value;
                }
            } else if let (Some(marker), Some(ink)) = (exit, ink.as_deref_mut()) {
                let value = active_exit_text_value(&projection.active_exit, marker.0);
                if text.0 != value {
                    text.0 = value;
                }
                if marker.0 == ActiveExitTextKind::Delay {
                    ink.0 = if projection.active_exit.delay_ms.is_some() {
                        palette.success
                    } else {
                        palette.ink_dim
                    };
                }
            } else if let (Some(marker), Some(ink)) = (quota, ink.as_deref_mut()) {
                let value = subscription_quota_text_value(&quota_presentation, marker.0);
                if text.0 != value {
                    text.0 = value;
                }
                if marker.0 == SubscriptionQuotaTextKind::Status {
                    ink.0 = quota_status_color(quota_presentation.grade, palette);
                }
            } else if let (Some(marker), Some(ink)) = (master, ink.as_deref_mut()) {
                let value = master_switch_text_value(
                    &projection.system_toggles,
                    marker.toggle,
                    marker.kind,
                    language,
                );
                if text.0 != value {
                    text.0 = value;
                }
                if marker.kind == OverviewMasterSwitchTextKind::Status {
                    ink.0 = master_switch_status_color(
                        &projection.system_toggles,
                        marker.toggle,
                        palette,
                    );
                }
            }
        } else if topology.is_none()
            && exit.is_none()
            && quota.is_none()
            && master.is_none()
            && let (Some(marker), Some(ink)) = (public_ip, ink.as_deref_mut())
        {
            let value = public_ip_text_value(&projection.public_ip, marker.0);
            if text.0 != value {
                text.0 = value;
            }
            if marker.0 == PublicIpTextKind::Status {
                ink.0 = match projection.public_ip.status {
                    PublicIpProbeStatus::Ready => palette.success,
                    PublicIpProbeStatus::Failed => palette.danger,
                    _ => palette.accent,
                };
            }
        }
    }
    for mut progress in &mut quota_progress {
        progress.width = percent(quota_presentation.fraction.unwrap_or(0.0) * 100.0);
    }
    for mut button in &mut master_buttons {
        let state = projection.system_toggles.state(button.toggle);
        button.enabled = state.is_enabled();
        button.can_toggle = state.can_toggle();
    }
    for mut button in &mut topology_buttons {
        button.enabled = projection.traffic_topology.is_drawable();
    }
    // The trend chart: re-derive the series for this projection's origin
    // and restamp only on an actual change (an unchanged spec must not pay
    // the raster cost every tick — sync_charts keys off `is_changed`).
    let is_mask_active = projection.reconnect_mask.is_active();
    for mut node in &mut reload_masks {
        let desired = if is_mask_active {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != desired {
            node.display = desired;
        }
    }
    if let (true, Some(msg)) = (is_mask_active, &projection.reconnect_mask.message) {
        for (mut text, mut copy) in &mut reload_mask_texts {
            *copy = LocalizedText::new("common_message", vec![("message", msg.clone())]);
            if text.0 != *msg {
                text.0 = msg.clone();
            }
        }
    }

    let (up, down, smooth, scale) = chart_inputs(projection, &history);
    let (width, height) = chart_dims();
    let spec = ChartSpec::new(up, down, width, height)
        .with_smooth(smooth)
        .with_scale_mode(ScaleMode::Fixed(scale.max_bps as f32));
    for mut plate in &mut charts {
        if plate.0 != spec {
            plate.0 = spec.clone();
        }
    }
    for mut plate in &mut topology_charts {
        let phase = plate.0.flow_phase;
        let spec = topology_spec(&projection.traffic_topology)
            .with_flow(phase, projection.traffic_topology.flow_speed_hz());
        if plate.0 != spec {
            plate.0 = spec;
        }
    }
    last.0 = Some(projection.clone());
}

/// The page's token reskin: every filled node re-derives its fill from
/// the live palette and its stored state, compare-and-set, every frame —
/// a `ThemeSwitch` repaints the banner, dot, mode chip and stop button
/// with no switch hook and no remount.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct OverviewFill {
    fill: &'static mut BackgroundColor,
    card: Option<&'static OverviewCardState>,
    dot: Has<StatusDot>,
    chip: Has<OverviewModeChip>,
    elevated: Has<SurfaceElevatedFill>,
    accent_container: Has<AccentContainerFill>,
    surface: Has<SurfaceFill>,
    border: Has<BorderFill>,
    accent: Has<AccentFill>,
}
pub(crate) fn reskin_overview_tokens(
    palette: Res<UiPalette>,
    last: Res<LastOverviewProjection>,
    mut fills: Query<OverviewFill>,
    mut inks: Query<(&mut TextColor, Has<OnAccentText>)>,
) {
    for mut row in &mut fills {
        let want = if let Some(state) = row.card {
            state
                .0
                .as_ref()
                .map_or(palette.accent_container, |state| card_fill(state, &palette))
        } else if row.dot {
            status_dot_color(
                last.0.as_ref().map(|projection| &projection.lifecycle),
                &palette,
            )
        } else if row.chip || row.accent {
            palette.accent
        } else if row.elevated {
            palette.surface_elevated
        } else if row.accent_container {
            palette.accent_container
        } else if row.surface {
            palette.surface
        } else if row.border {
            palette.border
        } else {
            continue;
        };
        if row.fill.0 != want {
            row.fill.0 = want;
        }
    }
    for (mut ink, on_accent) in &mut inks {
        if on_accent && ink.0 != palette.on_accent {
            ink.0 = palette.on_accent;
        }
    }
}
