//! The Overview page's projection-restamp seam: the typed markers and value
//! projections for the card texts, the single refresh observer that restamps
//! them in place, and the per-frame palette reskin.

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::system::{ParamSet, Query, Res, ResMut};
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, Display, Node, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_bevy_widgets::chart::ChartSpec;
use infiltrator_bevy_widgets::chart::topology::TopologyPlate;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot, SystemToggleState};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::history::{TrafficHistory, chart_inputs};
use crate::pages::overview::{
    AccentContainerFill, AccentFill, BorderFill, LastOverviewProjection, OnAccentText,
    OverviewCardState, OverviewChip, OverviewChipKind, OverviewLine, OverviewLineKind,
    OverviewModeChip, OverviewModePill, OverviewProjectionUpdated, OverviewReloadMask,
    OverviewReloadMaskText, OverviewStatusCard, StatusDot, StopButton, SurfaceElevatedFill,
    SurfaceFill, banner_note, card_fill, chart_dims, chip_label, format_byte_count, format_cpu,
    format_memory, format_rate, format_scale, format_total_traffic, mode_label, state_ink,
    state_label,
};
use crate::pages::overview_public_ip::{PublicIpText, PublicIpTextKind, public_ip_text_value};
use crate::pages::overview_topology::{
    TopologyStageButton, TopologyText, topology_spec, topology_text_value,
};

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
    snapshot: &infiltrator_contract::active_exit::ActiveExitSnapshot,
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
            infiltrator_contract::active_exit::ActiveExitStatus::Ready => match snapshot.alive {
                Some(true) => "selected · alive".to_owned(),
                Some(false) => "selected · offline".to_owned(),
                None => "selected · liveness unknown".to_owned(),
            },
            infiltrator_contract::active_exit::ActiveExitStatus::Empty => {
                "no active exit".to_owned()
            }
            infiltrator_contract::active_exit::ActiveExitStatus::Unknown => {
                "active exit pending".to_owned()
            }
            infiltrator_contract::active_exit::ActiveExitStatus::Unsupported => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "active exit unavailable".to_owned()),
            infiltrator_contract::active_exit::ActiveExitStatus::Failed => snapshot
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
    snapshot: &infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot,
    kind: SubscriptionQuotaTextKind,
) -> String {
    match kind {
        SubscriptionQuotaTextKind::Profile => snapshot
            .profile_name
            .clone()
            .unwrap_or_else(|| "no active subscription".to_owned()),
        SubscriptionQuotaTextKind::Expiry => {
            let date = snapshot
                .expires_at_label
                .clone()
                .unwrap_or_else(|| "expiry not reported".to_owned());
            match snapshot.remaining_days {
                Some(days) if days >= 0 => format!("{date} · {days}d left"),
                _ => date,
            }
        }
        SubscriptionQuotaTextKind::Metrics => {
            let used = snapshot
                .used_bytes
                .map(format_byte_count)
                .unwrap_or_else(|| "—".to_owned());
            let total = snapshot
                .total_bytes
                .map(format_byte_count)
                .unwrap_or_else(|| "—".to_owned());
            let percent = snapshot
                .usage_percent
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| "usage unknown".to_owned());
            format!("used {used} / total {total} · {percent}")
        }
        SubscriptionQuotaTextKind::Reset => snapshot
            .reset_days
            .map(|days| format!("reset in {days}d"))
            .unwrap_or_else(|| "reset not reported".to_owned()),
        SubscriptionQuotaTextKind::Status => match snapshot.status {
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Unknown => {
                "quota pending".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Ready => {
                "healthy".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Empty => {
                "quota not reported".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Warning => {
                "warning · above 80%".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Critical => {
                "critical · above 90%".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Exhausted => {
                "exhausted".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Expired => {
                "expired".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::ExpiringSoon => {
                "expiring soon".to_owned()
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Unsupported => {
                snapshot
                    .failure
                    .clone()
                    .unwrap_or_else(|| "quota unavailable".to_owned())
            }
            infiltrator_contract::subscription_quota::SubscriptionQuotaStatus::Failed => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "quota read failed".to_owned()),
        },
    }
}

pub(crate) fn master_switch_text_value(
    snapshot: &SystemToggleSnapshot,
    toggle: SystemToggle,
    kind: OverviewMasterSwitchTextKind,
) -> String {
    let state = snapshot.state(toggle);
    match kind {
        OverviewMasterSwitchTextKind::Status => match state {
            SystemToggleState::Enabled => "已开启".to_owned(),
            SystemToggleState::Disabled => "已关闭".to_owned(),
            SystemToggleState::Pending { .. } => "切换中".to_owned(),
            SystemToggleState::Unknown => "状态未知".to_owned(),
            SystemToggleState::Unsupported { failure } | SystemToggleState::Failed { failure } => {
                failure.message.clone()
            }
        },
        OverviewMasterSwitchTextKind::Action => match state {
            SystemToggleState::Enabled => "关闭".to_owned(),
            SystemToggleState::Disabled => "开启".to_owned(),
            SystemToggleState::Pending { .. } => "切换中".to_owned(),
            SystemToggleState::Unknown
            | SystemToggleState::Unsupported { .. }
            | SystemToggleState::Failed { .. } => "不可用".to_owned(),
        },
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

pub(crate) fn on_overview_mode_segment_activated(
    activate: On<Activate>,
    buttons: Query<&crate::pages::overview_cards::OverviewModeSegmentPill>,
    latest: Res<crate::surface::LatestSurfaceSnapshot>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let shared =
        infiltrator_application::proxy_mode_application::ProxyModeApplication::from_surface(
            &latest.0,
        );
    let Ok(intent) = infiltrator_application::proxy_mode_application::ProxyModeApplication::intent(
        &shared, button.0,
    ) else {
        return;
    };
    let command = match intent {
        CommandIntent::SetProxyMode { mode } => UiCommand::SetProxyMode(mode),
        _ => return,
    };
    handle.submit(command);
}

pub(crate) fn on_overview_master_switch_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewMasterSwitchButton>,
    latest: Res<crate::surface::LatestSurfaceSnapshot>,
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
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // observer params: disjoint queries are the API
pub(crate) fn apply_overview_projection(
    update: On<OverviewProjectionUpdated>,
    palette: Res<UiPalette>,
    history: Res<TrafficHistory>,
    mut last: ResMut<LastOverviewProjection>,
    // `Without<OverviewChip>`: line texts never sit on a chip root, so the
    // two `AccessibilityNode`-mutating queries stay provably disjoint.
    mut lines: Query<
        (
            &mut Text,
            &mut TextColor,
            &OverviewLine,
            Option<&mut AccessibilityNode>,
        ),
        Without<OverviewChip>,
    >,
    mut pills: Query<(&OverviewModePill, &mut ControlVisual)>,
    mut mode_segment_pills: Query<(
        &crate::pages::overview_cards::OverviewModeSegmentPill,
        &mut BackgroundColor,
    )>,
    mut cards: Query<&mut OverviewCardState, With<OverviewStatusCard>>,
    mut chips: Query<(Entity, &OverviewChip, Option<&mut AccessibilityNode>)>,
    // `Without<OverviewLine>`: chip value texts never carry a line marker,
    // so the two `Text`-mutable queries stay provably disjoint.
    mut values: Query<&mut Text, (With<StatChipValue>, Without<OverviewLine>)>,
    mut reload_masks: Query<
        &mut Node,
        (With<OverviewReloadMask>, Without<SubscriptionQuotaProgress>),
    >,
    mut reload_mask_texts: Query<
        &mut Text,
        (
            With<OverviewReloadMaskText>,
            Without<OverviewLine>,
            Without<TopologyText>,
            Without<ActiveExitText>,
            Without<SubscriptionQuotaText>,
            Without<OverviewMasterSwitchText>,
            Without<PublicIpText>,
            Without<StatChipValue>,
        ),
    >,
    mut dynamic: ParamSet<(
        Query<
            (&mut Text, &TopologyText),
            (
                With<TopologyText>,
                Without<OverviewLine>,
                Without<PublicIpText>,
                Without<OverviewReloadMaskText>,
                Without<StatChipValue>,
            ),
        >,
        Query<
            (&mut Text, &mut TextColor, &ActiveExitText),
            (
                With<ActiveExitText>,
                Without<OverviewLine>,
                Without<TopologyText>,
                Without<PublicIpText>,
                Without<OverviewReloadMaskText>,
                Without<StatChipValue>,
            ),
        >,
        Query<
            (&mut Text, &mut TextColor, &SubscriptionQuotaText),
            (
                With<SubscriptionQuotaText>,
                Without<OverviewLine>,
                Without<TopologyText>,
                Without<ActiveExitText>,
                Without<PublicIpText>,
                Without<OverviewReloadMaskText>,
                Without<StatChipValue>,
            ),
        >,
        Query<&mut Node, (With<SubscriptionQuotaProgress>, Without<OverviewReloadMask>)>,
        Query<
            (&mut Text, &mut TextColor, &OverviewMasterSwitchText),
            (
                With<OverviewMasterSwitchText>,
                Without<OverviewLine>,
                Without<TopologyText>,
                Without<ActiveExitText>,
                Without<SubscriptionQuotaText>,
                Without<PublicIpText>,
                Without<OverviewReloadMaskText>,
                Without<StatChipValue>,
            ),
        >,
        Query<&mut OverviewMasterSwitchButton>,
        Query<&mut TopologyStageButton>,
        Query<
            (&mut Text, &mut TextColor, &PublicIpText),
            (
                With<PublicIpText>,
                Without<OverviewLine>,
                Without<TopologyText>,
                Without<ActiveExitText>,
                Without<SubscriptionQuotaText>,
                Without<OverviewMasterSwitchText>,
                Without<OverviewReloadMaskText>,
                Without<StatChipValue>,
            ),
        >,
    )>,
    groups: Query<&Children>,
    mut charts: Query<&mut ChartPlate>,
    mut topology_charts: Query<&mut TopologyPlate>,
) {
    let projection = &update.0;
    for (mut text, mut ink, line, semantic) in &mut lines {
        match line.0 {
            OverviewLineKind::State => {
                text.0 = state_label(projection.state).to_owned();
                ink.0 = state_ink(projection.state, &palette);
                if let Some(mut node) = semantic {
                    node.0.set_label(state_label(projection.state));
                }
            }
            OverviewLineKind::Upload => {
                text.0 = format!("↑ {}", format_rate(projection.upload_bps));
                ink.0 = palette.success;
            }
            OverviewLineKind::Download => {
                text.0 = format!("↓ {}", format_rate(projection.download_bps));
            }
            OverviewLineKind::Failure => text.0 = projection.failure_text().to_owned(),
            OverviewLineKind::ModeChip => text.0 = mode_label(projection.mode).to_owned(),
            OverviewLineKind::BannerNote => text.0 = banner_note(projection),
            OverviewLineKind::Scale => text.0 = format_scale(&projection.traffic_scale),
        }
    }
    for (pill, mut visual) in &mut pills {
        visual.0 = pill.0 == projection.mode;
    }
    for (pill, mut bg) in &mut mode_segment_pills {
        let is_current = pill.0 == projection.proxy_mode.current;
        let selectable = projection.proxy_mode.is_mode_selectable(pill.0);
        *bg = if is_current {
            palette.accent.into()
        } else if selectable {
            palette.accent_container.into()
        } else {
            palette.surface_elevated.into()
        };
    }
    for mut card in &mut cards {
        card.0 = projection.state;
    }
    for (chip_entity, chip, semantic) in &mut chips {
        let value = match chip.0 {
            OverviewChipKind::Connections => projection.active_connections.to_string(),
            OverviewChipKind::Memory => format_memory(projection.memory_bytes),
            OverviewChipKind::Cpu => format_cpu(projection.cpu_percent),
            OverviewChipKind::Upload => format_rate(projection.upload_bps),
            OverviewChipKind::Download => format_rate(projection.download_bps),
            OverviewChipKind::TotalTraffic => format_total_traffic(projection.total_traffic_bytes),
        };
        if let Some(mut node) = semantic {
            node.0.set_label(format!("{} {value}", chip_label(chip.0)));
        }
        for descendant in groups.iter_descendants(chip_entity) {
            if let Ok(mut text) = values.get_mut(descendant) {
                text.0 = value.clone();
            }
        }
    }
    {
        let mut topology_texts = dynamic.p0();
        for (mut text, marker) in &mut topology_texts {
            let value = topology_text_value(&projection.traffic_topology, marker);
            if text.0 != value {
                text.0 = value;
            }
        }
    }
    {
        let mut active_exit_texts = dynamic.p1();
        for (mut text, mut ink, marker) in &mut active_exit_texts {
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
        }
    }
    {
        let mut quota_texts = dynamic.p2();
        for (mut text, mut ink, marker) in &mut quota_texts {
            let value = subscription_quota_text_value(&projection.subscription_quota, marker.0);
            if text.0 != value {
                text.0 = value;
            }
            if marker.0 == SubscriptionQuotaTextKind::Status {
                ink.0 = crate::pages::overview_cards::quota_status_color(
                    projection.subscription_quota.status,
                    &palette,
                );
            }
        }
    }
    {
        let mut quota_progress = dynamic.p3();
        for mut progress in &mut quota_progress {
            progress.width = percent(projection.subscription_quota.usage_fraction() * 100.0);
        }
    }
    {
        let mut master_texts = dynamic.p4();
        for (mut text, mut ink, marker) in &mut master_texts {
            let value =
                master_switch_text_value(&projection.system_toggles, marker.toggle, marker.kind);
            if text.0 != value {
                text.0 = value;
            }
            if marker.kind == OverviewMasterSwitchTextKind::Status {
                ink.0 =
                    master_switch_status_color(&projection.system_toggles, marker.toggle, &palette);
            }
        }
    }
    {
        let mut master_buttons = dynamic.p5();
        for mut button in &mut master_buttons {
            let state = projection.system_toggles.state(button.toggle);
            button.enabled = state.is_enabled();
            button.can_toggle = state.can_toggle();
        }
    }
    {
        let mut topology_buttons = dynamic.p6();
        for mut button in &mut topology_buttons {
            button.enabled = projection.traffic_topology.is_drawable();
        }
    }
    {
        let mut public_ip_texts = dynamic.p7();
        for (mut text, mut ink, marker) in &mut public_ip_texts {
            let value = public_ip_text_value(&projection.public_ip, marker.0);
            if text.0 != value {
                text.0 = value;
            }
            if marker.0 == PublicIpTextKind::Status {
                ink.0 = match projection.public_ip.status {
                    infiltrator_contract::public_ip::PublicIpProbeStatus::Ready => palette.success,
                    infiltrator_contract::public_ip::PublicIpProbeStatus::Failed => palette.danger,
                    _ => palette.accent,
                };
            }
        }
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
        for mut text in &mut reload_mask_texts {
            if text.0 != *msg {
                text.0 = msg.clone();
            }
        }
    }

    let (up, down, smooth, scale) = chart_inputs(projection, &history);
    let (width, height) = chart_dims();
    let spec = ChartSpec::new(up, down, width, height)
        .with_smooth(smooth)
        .with_scale_mode(infiltrator_bevy_widgets::chart::bezier::ScaleMode::Fixed(
            scale.max_bps as f32,
        ));
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
#[allow(clippy::type_complexity)]
pub(crate) fn reskin_overview_tokens(
    palette: Res<UiPalette>,
    mut fills: Query<(
        &mut BackgroundColor,
        Option<&OverviewCardState>,
        Has<StatusDot>,
        Has<OverviewModeChip>,
        Has<StopButton>,
        Has<SurfaceElevatedFill>,
        Has<AccentContainerFill>,
        Has<SurfaceFill>,
        Has<BorderFill>,
        Has<AccentFill>,
    )>,
    mut inks: Query<(&mut TextColor, Has<OnAccentText>)>,
) {
    for (mut fill, card, dot, chip, stop, elevated, acc_container, surface, border, accent) in
        &mut fills
    {
        let want = if let Some(state) = card {
            card_fill(state.0, &palette)
        } else if dot {
            palette.success
        } else if chip || accent {
            palette.accent
        } else if stop {
            palette.danger
        } else if elevated {
            palette.surface_elevated
        } else if acc_container {
            palette.accent_container
        } else if surface {
            palette.surface
        } else if border {
            palette.border
        } else {
            continue;
        };
        if fill.0 != want {
            fill.0 = want;
        }
    }
    for (mut ink, on_accent) in &mut inks {
        if on_accent && ink.0 != palette.on_accent {
            ink.0 = palette.on_accent;
        }
    }
}
