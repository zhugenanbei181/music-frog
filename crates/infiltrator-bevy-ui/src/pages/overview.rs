//! The Overview page: run state banner, live traffic card and the four
//! stat chips, rendered from the shared [`OverviewProjection`].
//!
//! **Update seam**: mutable nodes carry typed markers ([`OverviewLine`],
//! [`OverviewChip`] + the widget layer's `StatChipValue`,
//! [`OverviewStatusCard`] / [`OverviewCardState`], [`OverviewModePill`]).
//! [`OverviewPagePlugin`] registers [`apply_overview_projection`] once at product assembly, and the router fires the first paint with the mounted
//! projection right after `spawn_scene`. From then on, an
//! [`OverviewProjectionUpdated`] trigger restamps texts, inks, pill
//! selection bits and the banner's stored state *in place*: entity ids
//! never change, nothing is re-mounted, nothing polls (charter law:
//! observers change components, never rebuild trees).
//!
//! **Theme seam**: every filled node of the page (banner, status dot,
//! mode chip, stop button) carries a marker and is repainted every frame
//! by [`reskin_overview_tokens`] — a compare-and-set projection from the
//! live palette (the checkbox/slider sync idiom), so a `ThemeSwitch`
//! rethemes them with no switch-specific hook and no remount. The widget
//! layer owns the same contract for its stat chips / surfaces / icon
//! tiles / nav items. What a per-frame reskin *cannot* recover is the
//! state-specific text ink (the unavailable danger ink, the uplink
//! success ink) that the widget layer's `apply_theme` restamps to plain
//! role ink — so the page mirrors the last projection it rendered
//! ([`LastOverviewProjection`]) and [`replay_projection_after_theme`]
//! re-fires it once the switch's own observer dispatch has finished:
//! state semantics win the same frame, with zero remounts.
//!
//! **Accessibility seam**: the banner's state word (a `Status` role) and
//! each stat chip (a `Group` role labeled "name value") carry AccessKit
//! nodes seeded in the scenes and restamped by the refresh observer —
//! the same in-place restamp the visible texts get.
//!
//! Remaining localized status folds are tracked by the shared-copy quality gate.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::history::{ScrubberAction, TrafficHistory, TrafficReplay, chart_inputs};
use crate::localized_widgets::localized_button_scene;
use crate::pages::overview_lifecycle::core_control_scene;
use crate::pages::overview_public_ip::on_overview_public_ip_refresh_activated;
use crate::pages::overview_quota_copy;
use crate::pages::overview_rates;
use crate::pages::overview_restamp::{
    apply_overview_projection, on_overview_master_switch_activated, sync_overview_replay,
};
use crate::pages::overview_speedtest::{
    on_overview_speedtest_activated, on_overview_speedtest_concurrency_stepped,
    on_overview_speedtest_detail_activated, speedtest_button_scene,
};
use crate::pages::overview_topology::on_topology_stage_activated;
use crate::pages::{overview_cards, overview_public_ip, overview_topology};
use crate::projection::{OverviewOrigin, OverviewProjection};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::PositionType;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, ComputedNode, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_application::byte_format::format_bytes;
use infiltrator_application::core_status_projection::{failure_copy, lifecycle_copy, source_copy};
use infiltrator_application::proxy_mode_projection::mode_status_copy;
use infiltrator_application::shell_readout_projection::{observed_rate, rate_copy, rate_status};
use infiltrator_bevy_widgets::button::{ButtonSize, ButtonVariant, button_sized_scene};
use infiltrator_bevy_widgets::chart::chart_scene_with_scale;
use infiltrator_bevy_widgets::fluid_grid::{FluidCardGrid, compute_ideal_column_layout};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::stat_chip::stat_chip_scene;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{Breakpoint, space};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

/// The trend chart's raster box (ui-side tokens — the widget's pixel box
/// is fixed at mount; a resize is a remount, chart.rs). Height ~140px per
/// the reference card. Width is the capture card's interior: 1180 window
/// − 240 sidebar − 2×16 content padding = 908 card, − 2×16 card padding
/// = 876 — so the chart fills the card edge to edge at the capture size.
pub const CHART_WIDTH_PX: f32 = 876.0;
/// The trend chart's raster box height (~140px, the reference card's
/// plot band).
pub const CHART_HEIGHT_PX: f32 = 140.0;

/// The [`ChartSpec`] dimensions for the page's chart box — the same
/// round-to-px math [`chart_scene`] applies, so a mount spec and a
/// refresh restamp always agree on the extent (a rewrite never changes
/// the extent, chart.rs).
pub(crate) fn chart_dims() -> (u32, u32) {
    (
        CHART_WIDTH_PX.round().max(1.0) as u32,
        CHART_HEIGHT_PX.round().max(1.0) as u32,
    )
}

/// The Overview page root. Stamps [`PageRoot`]`(`[`Route::Overview`]`)`
/// next to it; the page plugin owns refresh wiring.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct OverviewPageRoot;

#[derive(Component, Clone, Copy, Default)]
pub struct OverviewTrafficCard;

/// Marker on the Overview traffic card's trend chart plate. The page's
/// projection restamp and the replay scrubber both key off it, so the
/// proxy-inspection chart is never touched.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewTrafficChart;

/// Marker on one time-travel scrubber button; carries its typed action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewScrubberButton(pub ScrubberAction);

/// Marker on the scrubber's status line (live / empty / unsupported /
/// historical position).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewScrubberStatus;

/// Which mutable line of the page a text node is. One enum marker keeps
/// the refresh observer to a single (conflict-free) query.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewLine(pub OverviewLineKind);

/// The lines the refresh observer rewrites.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewLineKind {
    /// The run-state word on the banner (also carries the state ink).
    #[default]
    State,
    /// Uplink rate (arrow prefix + mono value, success ink).
    Upload,
    /// Downlink rate (arrow prefix + mono value, ordinary ink).
    Download,
    TelemetryFailure,
    /// The failure reason (empty unless unavailable).
    Failure,
    /// The mode chip's label.
    ModeChip,
    /// The banner's data-origin note: 演示数据 for the fixture, the live
    /// core's real version for the pump (BEVY-005).
    BannerNote,
    /// The shared dynamic max and human-readable tick labels.
    Scale,
}

/// Which stat a chip in the metrics band stands for; the refresh observer
/// routes projection values into each chip's marked value text through
/// the chip's children.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewChip(pub OverviewChipKind);

/// The four chips of the metrics band.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewChipKind {
    /// Active connection count.
    #[default]
    Connections,
    /// Core memory footprint.
    Memory,
    /// CPU utilization.
    Cpu,
    /// Uplink rate.
    Upload,
    /// Downlink rate.
    Download,
    /// Cumulative session total traffic.
    TotalTraffic,
}

/// Marker on the status banner root: carries [`OverviewCardState`] (the
/// projection state the fill derives from) so the per-frame reskin system
/// can re-derive the token fill from the live palette.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct OverviewStatusCard;

/// Marker on a reorderable card slot on the Overview page (DUAL-03-12).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardSlot(pub OverviewCardKind);

/// Marker on the reload / reconnect graceful degradation overlay mask (DUAL-03-13).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReloadMask;

/// Marker on the text rendered within the reload overlay mask.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReloadMaskText;

/// Reorder button action on an Overview card slot (Move Up).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardMoveUpButton(pub OverviewCardKind);

/// Reorder button action on an Overview card slot (Move Down).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardMoveDownButton(pub OverviewCardKind);

/// Marker for the Overview metrics chip band container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewMetricsBand;

/// Marker on nodes filled with the `surface_elevated` token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceElevatedFill;

/// Marker on nodes filled with the `accent_container` token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccentContainerFill;

/// Marker on nodes filled with the `surface` token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceFill;

/// Marker on nodes filled with the `border` token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BorderFill;

/// Marker on nodes filled with the `accent` token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccentFill;

/// The banner's stored projection state; restamped by the refresh
/// observer, read by [`reskin_overview_tokens`].
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct OverviewCardState(pub Option<CoreLifecycle>);

/// Marker on the banner's status dot (success token fill).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusDot;

/// Marker on the banner's mode chip (accent fill, `on_accent` label).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewModeChip;

/// Marker on text drawn over an accent/danger fill: its ink is the
/// `on_accent` token, restamped by the reskin system.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OnAccentText;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewModePill(pub ProxyMode);

/// The typed data-refresh event: carry the new projection, the observer
/// does the rest. Zero polling — whoever produces a fresh projection
/// (the future core pump, a test, a demo tick) triggers this.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct OverviewProjectionUpdated(pub OverviewProjection);

/// The page's mirror of the projection it last rendered. The theme
/// switch's replay source: `apply_theme` (the widget layer) restamps role
/// ink over state ink, so [`replay_projection_after_theme`] re-fires this
/// projection once the switch's dispatch has finished and the state
/// semantics (danger ink, success ink, banner state) win again. `None`
/// only before the router's first paint.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastOverviewProjection(pub Option<OverviewProjection>);

/// The theme-switch replay: re-fire the last rendered projection. Runs as
/// a `ThemeSwitch` observer next to the widget layer's `apply_theme`, but
/// the trigger it queues is deferred until the *whole* switch dispatch has
/// finished — so the replay always applies on top of the freshly restamped
/// role inks, regardless of observer registration order.
pub(crate) fn replay_projection_after_theme(
    _switch: On<ThemeSwitch>,
    last: Res<LastOverviewProjection>,
    mut commands: Commands,
) {
    if let Some(projection) = last.0.clone() {
        commands.trigger(OverviewProjectionUpdated(projection));
    }
}

// ---- pure functions (headless-testable without any app) --------------------

/// Format a byte-per-second rate for display: the shared [`format_bytes`]
/// ladder used by both peer products with an
/// an explicit unknown for invalid observations. Pure function.
pub fn format_rate(bytes_per_second: f64) -> String {
    rate_copy(&observed_rate(Some(bytes_per_second)), "en-US")
}

/// Format a byte count for the memory chip: the shared [`format_bytes`]
/// ladder; `None` renders an honest em-dash placeholder (the value is not
/// known — never a fabricated zero). Pure function.
pub fn format_memory(bytes: Option<u64>) -> String {
    bytes.map(format_bytes).unwrap_or_else(|| "—".to_owned())
}

/// Format CPU utilization percentage for the CPU chip.
pub fn format_cpu(percent: Option<f32>) -> String {
    percent
        .map(|p| format!("{:.1}%", p))
        .unwrap_or_else(|| "—".to_owned())
}

/// Format cumulative session total traffic for the TotalTraffic chip.
pub fn format_total_traffic(bytes: Option<u64>) -> String {
    bytes.map(format_bytes).unwrap_or_else(|| "—".to_owned())
}

/// A metrics chip's stat name (zh-CN literals — see the module copy
/// note). The scene and the a11y group label both spell it through this
/// one function, so they can never drift apart.
pub(crate) fn chip_label_key(kind: OverviewChipKind) -> &'static str {
    match kind {
        OverviewChipKind::Connections => "overview_connections",
        OverviewChipKind::Memory => "overview_memory",
        OverviewChipKind::Cpu => "overview_cpu",
        OverviewChipKind::Upload => "overview_upload",
        OverviewChipKind::Download => "overview_download",
        OverviewChipKind::TotalTraffic => "overview_total_traffic",
    }
}

pub(crate) fn chip_label(kind: OverviewChipKind, language: &str) -> String {
    Lang(language).tr(chip_label_key(kind)).into_owned()
}

/// The banner's status-word semantic node: a `Status` role carrying the
/// run-state word, so assistive technology hears the live verdict (the
/// refresh observer restamps the label as the state changes).
pub fn status_semantic_node(label: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Status);
    node.set_label(label);
    AccessibilityNode(node)
}

/// A stat chip's semantic node: a `Group` role labeled "name value"
/// (e.g. 内存 96.00 MB), so the whole metric reads as one utterance. The
/// refresh observer restamps the label with each new value.
pub fn stat_group_semantic_node(label: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Group);
    node.set_label(label);
    AccessibilityNode(node)
}

/// The state line's ink: readable on the accent container while
/// running/stopped, `on_accent` on the danger fill while unavailable.
/// Tokens only.
pub(crate) fn state_ink(state: &CoreLifecycle, palette: &UiPalette) -> Color {
    match state {
        CoreLifecycle::Running | CoreLifecycle::Ready => palette.ink,
        CoreLifecycle::Stopped => palette.ink_dim,
        CoreLifecycle::Starting | CoreLifecycle::Stopping => palette.warning,
        CoreLifecycle::Failed => palette.on_accent,
    }
}
pub(crate) fn status_dot_color(state: Option<&CoreLifecycle>, palette: &UiPalette) -> Color {
    match state {
        Some(CoreLifecycle::Ready | CoreLifecycle::Running) => palette.success,
        Some(CoreLifecycle::Starting | CoreLifecycle::Stopping) => palette.warning,
        Some(CoreLifecycle::Failed) => palette.danger,
        Some(CoreLifecycle::Stopped) | None => palette.ink_dim,
    }
}

/// The banner's data-origin note. The demo fixture owns 演示数据; a live
/// core names the version it actually reported (an empty or unread
/// version stays honest as 版本读取中 — the failure line, not this note,
/// carries the failure verdict). Pure function.
pub(crate) fn banner_note(projection: &OverviewProjection, language: &str) -> String {
    source_copy(
        match projection.origin {
            OverviewOrigin::Demo => SurfaceOrigin::Demo,
            OverviewOrigin::LiveCore => SurfaceOrigin::Live,
        },
        projection.core_version.as_deref(),
        language,
    )
}

/// The banner's fill: the accent container token, or the danger token
/// while unavailable — the whole-banner failure projection.
pub(crate) fn card_fill(state: &CoreLifecycle, palette: &UiPalette) -> Color {
    match state {
        CoreLifecycle::Failed => palette.danger,
        _ => palette.accent_container,
    }
}

// ---- scene adapters ---------------------------------------------------------

/// Reload / reconnect graceful degradation overlay mask (DUAL-03-13).
pub(crate) fn reload_mask_scene(palette: &UiPalette) -> impl Scene + use<> {
    let scrim = palette.scrim;
    bsn! {
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            BackgroundColor({ scrim })
            OverviewReloadMask
            Children [
                @{ icon_scene(IconId::Activity, 24.0, palette.accent) }
                --
                LocalizedText::plain("overview_core_reloading") OverviewReloadMaskText TextRole(Role::BodyStrong) TextColor({ palette.on_accent })
            ]
    }
}

/// The Overview page: status banner, live traffic card and the metrics
/// chip band, filling the shell's content slot. The traffic card's trend
/// chart seeds from [`chart_series`] — the demo fixture's synthetic waves
/// at mount, the live pump's recorded ring for a live source.
pub fn overview_page(
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::Overview)
            OverviewPageRoot
            ScrollArea
            Children [
                @{ banner_scene(projection, palette) }
                --
                @{ overview_cards::mode_segmented_controller_scene_with_snapshot(&projection.proxy_mode, palette) }
                --
                @{ traffic_card_scene(projection, history, palette) } OverviewTrafficCard
                --
                @{ chips_row_scene(projection, palette) }
                --
                @{ overview_cards::master_switches_scene_with_snapshot(&projection.system_toggles, palette) }
                --
                @{ overview_cards::active_exit_node_scene_with_snapshot(&projection.active_exit, palette) }
                --
                @{ overview_public_ip::public_ip_probe_card_scene_with_snapshot(&projection.public_ip, palette) }
                --
                @{ overview_topology::topology_chain_scene_with_snapshot(&projection.traffic_topology, palette) }
                --
                @{ overview_cards::subscription_quota_scene_with_snapshot(&projection.subscription_quota, palette) }
                --
                @{ reload_mask_scene(palette) }
            ]
    }
}

/// The status banner: the accent-container card with the running dot and
/// state word, the mode chip, the data-origin note (demo fixture vs live
/// core version) and failure line, then the stop button — the danger pill
/// under a demo source, or an honest lifecycle caption under a live core
/// (nothing may masquerade as core lifecycle control until 0.30 wires it).
/// The origin is fixed for a mounted source, so baking the branch at mount
/// keeps every restamp in place.
fn banner_scene(projection: &OverviewProjection, palette: &UiPalette) -> impl Scene + use<> {
    let state = lifecycle_copy(&projection.lifecycle, "en-US");
    let state_node = status_semantic_node(&state);
    let chip = mode_status_copy(&projection.proxy_mode, "en-US");
    let failure = failure_copy(
        &projection.lifecycle,
        projection.failure.as_deref(),
        "en-US",
    );
    let note = banner_note(projection, "en-US");
    bsn! {
            Node {
                width: percent(100),
                padding: UiRect::all(Val::Px(space::S16)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
                row_gap: Val::Px(space::S8),
                flex_wrap: FlexWrap::Wrap,
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
            }
            BackgroundColor({ card_fill(&projection.lifecycle, palette) })
            OverviewStatusCard
            OverviewCardState({ Some(projection.lifecycle.clone()) })
            Children [
                Node {
                    flex_grow: 1.0,
                    min_width: px(180.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        Node {
                            width: px(10.0),
                            height: px(10.0),
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                        }
                        BackgroundColor({ status_dot_color(Some(&projection.lifecycle), palette) })
                        StatusDot
                        --
                        Text({ state }) OverviewLine(OverviewLineKind::State)
                        TextRole(Role::Display)
                        state_node
                    ]
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        Node {
                            padding: UiRect::all(Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.accent })
                        OverviewModeChip
                        Children [
                            Text({ chip }) OverviewLine(OverviewLineKind::ModeChip) TextRole(Role::Caption) OnAccentText
                        ]
                        --
                        Text({ note }) OverviewLine(OverviewLineKind::BannerNote) TextRole(Role::Caption)
                        --
                        Text({ failure }) OverviewLine(OverviewLineKind::Failure) TextRole(Role::Caption)
                    ]
                ]
                --
                @{ core_control_scene(palette) }
                --
                @{ speedtest_button_scene(palette) }
            ]
    }
}

/// The live traffic card: caption title, the up/down mono rate lines and
/// the dual-series trend chart (the widget layer's [`chart_scene`] —
/// upper series the accent ink, lower the success ink, fade fills under
/// both, exactly the reference card's language). The series come from
/// [`chart_series`]: the demo fixture's synthetic waves, or the live
/// pump's recorded ring. The widget's own `sync_charts` rasterizes the
/// plate and re-derives every ink from the live palette, so a theme
/// switch recolors the chart in place.
fn traffic_card_scene(
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (up, down, smooth, scale) = chart_inputs(projection, history);
    surface_scene(
        vec![
            Box::new(bsn! { LocalizedText::plain("overview_traffic") TextRole(Role::Caption) }),
            Box::new(rates_row_scene(
                rate_copy(&projection.readout.upload_bps, "en-US"),
                rate_copy(&projection.readout.download_bps, "en-US"),
            )),
            Box::new(scale_line_scene(&scale)),
            Box::new(bsn! { Text({rate_status(&projection.readout, "en-US")})
            OverviewLine(OverviewLineKind::TelemetryFailure) TextRole(Role::Caption) }),
            Box::new(overview_chart_scene(
                up,
                down,
                smooth,
                Some(scale.max_bps as f32),
            )),
            Box::new(scrubber_scene(palette)),
        ],
        palette,
    )
}

/// The trend chart plate, marked so the projection restamp and the replay
/// scrubber address only the Overview chart.
fn overview_chart_scene(
    up: Vec<f32>,
    down: Vec<f32>,
    smooth: bool,
    scale_max: Option<f32>,
) -> impl Scene + use<> {
    bsn! {
        @{ chart_scene_with_scale(
            up,
            down,
            CHART_WIDTH_PX,
            CHART_HEIGHT_PX,
            smooth,
            scale_max,
        ) }
        OverviewTrafficChart
    }
}

/// The time-travel scrubber row under the trend chart: coarse seeks, single
/// steps, a return-to-live button and the typed status readout. Purely
/// structural — the buttons' `Activate` observer and
/// [`sync_overview_replay`](crate::pages::overview_restamp::sync_overview_replay)
/// own the behavior.
fn scrubber_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
                row_gap: Val::Px(space::S4),
                flex_wrap: FlexWrap::Wrap,
            }
            Children [
                @{ scrubber_button_scene("⏮".to_owned(), ScrubberAction::SeekBackward, palette) }
                --
                @{ scrubber_button_scene("◀".to_owned(), ScrubberAction::StepBackward, palette) }
                --
                @{ scrubber_button_scene("▶".to_owned(), ScrubberAction::StepForward, palette) }
                --
                @{ scrubber_button_scene("⏭".to_owned(), ScrubberAction::SeekForward, palette) }
                --
                @{ scrubber_live_button_scene(palette) }
                --
                Text({ String::new() }) OverviewScrubberStatus TextRole(Role::Caption)
            ]
    }
}

/// One transport button: the shared button skin with a typed scrub action.
fn scrubber_button_scene(
    label: String,
    action: ScrubberAction,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        @{ button_sized_scene(label, ButtonVariant::Ghost, ButtonSize::Sm, palette) }
        OverviewScrubberButton(action)
    }
}

/// The return-to-live button: a primary-labeled control carrying the live
/// copy key, so it follows the active locale.
fn scrubber_live_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        @{ localized_button_scene(
            LocalizedText::plain("conn_state_live"),
            ButtonVariant::Primary,
            palette,
        ) }
        OverviewScrubberButton(ScrubberAction::ReturnToLive)
    }
}

/// The up/down rates side by side on one row (the reference layout).
fn rates_row_scene(upload: String, download: String) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S16),
            }
            Children [
                @{ rate_line("↑ ", OverviewLineKind::Upload, upload) }
                --
                @{ rate_line("↓ ", OverviewLineKind::Download, download) }
            ]
    }
}

fn scale_line_scene(scale: &TrafficScaleSnapshot) -> impl Scene + use<> {
    let label = format_scale(scale);
    bsn! {
            Node {
                width: percent(100),
                min_height: px(16.0),
            }
            Children [
                Text({ label }) OverviewLine(OverviewLineKind::Scale) TextRole(Role::Mono)
            ]
    }
}

pub(crate) fn format_scale(scale: &TrafficScaleSnapshot) -> String {
    format!(
        "scale max={} · ticks={}",
        scale.format_max(),
        scale
            .ticks
            .iter()
            .map(|tick| scale.format_tick(*tick))
            .collect::<Vec<_>>()
            .join(" / ")
    )
}

/// One rate line: the arrow and the mono rate share one marked text so
/// the refresh observer restamps them together (the arrow keeps the
/// line's ink — success for uplink, ordinary for downlink).
fn rate_line(arrow: &str, kind: OverviewLineKind, value: String) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
            }
            Children [
                Text({ format!("{arrow}{value}") }) OverviewLine(kind) TextRole(Role::Mono)
            ]
    }
}

/// A plain caption line inside a card.
/// The metrics band: four stat chips sharing the width evenly. Each chip
/// root carries a labeled `Group` semantic node ("name value") that the
/// refresh observer restamps alongside the visible value.
fn chips_row_scene(projection: &OverviewProjection, palette: &UiPalette) -> impl Scene + use<> {
    let connections = projection.active_connections.to_string();
    let memory = format_memory(projection.memory_bytes);
    let cpu = format_cpu(projection.cpu_percent);
    let upload = rate_copy(&projection.readout.upload_bps, "en-US");
    let download = rate_copy(&projection.readout.download_bps, "en-US");
    let total = format_total_traffic(projection.total_traffic_bytes);

    let connections_node = stat_group_semantic_node(&format!(
        "{} {connections}",
        chip_label(OverviewChipKind::Connections, "en-US")
    ));
    let memory_node = stat_group_semantic_node(&format!(
        "{} {memory}",
        chip_label(OverviewChipKind::Memory, "en-US")
    ));
    let cpu_node = stat_group_semantic_node(&format!(
        "{} {cpu}",
        chip_label(OverviewChipKind::Cpu, "en-US")
    ));
    let upload_node = stat_group_semantic_node(&format!(
        "{} {upload}",
        chip_label(OverviewChipKind::Upload, "en-US")
    ));
    let download_node = stat_group_semantic_node(&format!(
        "{} {download}",
        chip_label(OverviewChipKind::Download, "en-US")
    ));
    let total_node = stat_group_semantic_node(&format!(
        "{} {total}",
        chip_label(OverviewChipKind::TotalTraffic, "en-US")
    ));

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S12),
                row_gap: Val::Px(space::S12),
            }
            OverviewMetricsBand
            Children [
                @{ stat_chip_scene(IconId::Activity, chip_label(OverviewChipKind::Connections, "en-US").to_owned(), connections, palette) }
                OverviewChip(OverviewChipKind::Connections)
                connections_node
                --
                @{ stat_chip_scene(IconId::Zap, chip_label(OverviewChipKind::Memory, "en-US").to_owned(), memory, palette) }
                OverviewChip(OverviewChipKind::Memory)
                memory_node
                --
                @{ stat_chip_scene(IconId::Settings, chip_label(OverviewChipKind::Cpu, "en-US").to_owned(), cpu, palette) }
                OverviewChip(OverviewChipKind::Cpu)
                cpu_node
                --
                @{ stat_chip_scene(IconId::ArrowUp, chip_label(OverviewChipKind::Upload, "en-US").to_owned(), upload, palette) }
                OverviewChip(OverviewChipKind::Upload)
                upload_node
                --
                @{ stat_chip_scene(IconId::ArrowDown, chip_label(OverviewChipKind::Download, "en-US").to_owned(), download, palette) }
                OverviewChip(OverviewChipKind::Download)
                download_node
                --
                @{ stat_chip_scene(IconId::Globe, chip_label(OverviewChipKind::TotalTraffic, "en-US").to_owned(), total, palette) }
                OverviewChip(OverviewChipKind::TotalTraffic)
                total_node
            ]
    }
}

/// The traffic topology chain card: 4 linked stage chips with connecting arrows (">").
/// 4-stage network traffic topology chain scene (BEVY-GAP-018).
pub fn topology_chain_scene(palette: &UiPalette) -> impl Scene + use<> {
    overview_topology::topology_chain_scene(palette)
}

/// Subscription quota and billing cycle visualization card (BEVY-GAP-020).
pub fn subscription_quota_scene(palette: &UiPalette) -> impl Scene + use<> {
    overview_cards::subscription_quota_scene(palette)
}

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct OverviewPagePlugin;

impl Plugin for OverviewPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrafficReplay>();
        app.add_systems(
            Update,
            (
                overview_rates::refresh,
                overview_quota_copy::replay,
                sync_overview_replay,
            ),
        );
        app.add_observer(apply_overview_projection);
        app.add_observer(on_overview_scrubber_activated);
        app.add_observer(on_topology_stage_activated);
        app.add_observer(on_overview_master_switch_activated);
        app.add_observer(on_overview_speedtest_activated);
        app.add_observer(on_overview_speedtest_concurrency_stepped);
        app.add_observer(on_overview_speedtest_detail_activated);
        app.add_observer(on_overview_public_ip_refresh_activated);
        app.add_observer(on_overview_card_move_up_activated);
        app.add_observer(on_overview_card_move_down_activated);
    }
}

/// The scrubber's one behavior seam: turn an `Activate` on a marked button
/// into a typed replay action. The replay resource freezes the retained
/// store on the first non-live action, so stepping is stable.
pub(crate) fn on_overview_scrubber_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewScrubberButton>,
    history: Res<TrafficHistory>,
    mut replay: ResMut<TrafficReplay>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    replay.apply(&history, button.0);
}

/// Convert an Overview card move up action into a UiCommand::MoveOverviewCardUp.
pub(crate) fn on_overview_card_move_up_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewCardMoveUpButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(btn) = buttons.get(activate.entity) else {
        return;
    };
    handle.submit(UiCommand::MoveOverviewCardUp(btn.0));
}

pub(crate) fn on_overview_card_move_down_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewCardMoveDownButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(btn) = buttons.get(activate.entity) else {
        return;
    };
    handle.submit(UiCommand::MoveOverviewCardDown(btn.0));
}

/// Pin the Overview metrics chip band to the shared tier column count
/// (2 / 3 / 6 / 6) so the six tiles reflow on narrow windows.
/// When container dimensions are measured via [`ComputedNode`], the exact chip
/// width is computed via [`compute_ideal_column_layout`] to fill 100% of the
/// container width symmetrically. Otherwise, it falls back to the authoritative
/// percentage basis.
pub fn sync_overview_metrics_columns(
    ctx: Option<Res<ResponsiveContext>>,
    band_container: Query<Option<&ComputedNode>, With<OverviewMetricsBand>>,
    mut chips: Query<&mut Node, With<OverviewChip>>,
) {
    let Some(ctx) = ctx else {
        return;
    };
    let columns = match ctx.breakpoint {
        Breakpoint::Compact => 2,
        Breakpoint::Medium => 3,
        Breakpoint::Expanded | Breakpoint::Ultra => 6,
    };
    let fallback_basis = Val::Percent(FluidCardGrid::wrapped_item_percent(columns));

    let basis = if let Some(Some(computed)) = band_container.iter().next() {
        let measured_w = computed.size().x * computed.inverse_scale_factor();
        if measured_w > 100.0 {
            let layout = compute_ideal_column_layout(measured_w, 140.0, space::S12, columns);
            Val::Px(layout.item_width_px)
        } else {
            fallback_basis
        }
    } else {
        fallback_basis
    };

    for mut node in &mut chips {
        if node.flex_basis != basis {
            node.flex_basis = basis;
        }
    }
}
