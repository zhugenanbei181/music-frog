//! The Overview page: run state banner, live traffic card and the four
//! stat chips, rendered from an [`OverviewProjection`] in the iced
//! reference product language.
//!
//! **Update seam**: mutable nodes carry typed markers ([`OverviewLine`],
//! [`OverviewChip`] + the widget layer's `StatChipValue`,
//! [`OverviewStatusCard`] / [`OverviewCardState`], [`OverviewModePill`]).
//! The page self-registers [`apply_overview_projection`] once per world
//! (the [`OverviewPageRoot`] `on_insert` bind hook — the taskmanager
//! idiom), and the router fires the first paint with the mounted
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
//! **Copy note**: this page's user-facing strings are zh-CN literals by
//! design of this demo slice — the 0.30 i18n milestone unifies them into
//! locale keys. (本片文案为 zh-CN 字面量，0.30 i18n 里程碑统一 locale key。)

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent,
    Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::chart::chart_scene_with_scale;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::stat_chip_scene;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::ProxyMode;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::history::{TrafficHistory, chart_inputs};
use crate::pages::overview_public_ip::on_overview_public_ip_refresh_activated;
use crate::pages::overview_restamp::{
    apply_overview_projection, on_overview_master_switch_activated,
    on_overview_mode_segment_activated,
};
use crate::pages::overview_speedtest::{
    on_overview_speedtest_activated, on_overview_speedtest_concurrency_stepped,
    on_overview_speedtest_detail_activated, speedtest_button_scene,
};
use crate::pages::overview_topology::on_topology_stage_activated;
use crate::projection::{OverviewOrigin, OverviewProjection, OverviewState};
use crate::route::{PageRoot, Route};

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
/// next to it; mounting this marker binds the page's refresh observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_overview_page)]
pub struct OverviewPageRoot;

/// Once-per-world guard for the bind hook: a remounted page must not
/// stack duplicate refresh observers.
#[derive(Resource)]
struct OverviewPageBound;

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
pub struct OverviewCardSlot(pub infiltrator_contract::overview_layout::OverviewCardKind);

/// Marker on the reload / reconnect graceful degradation overlay mask (DUAL-03-13).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReloadMask;

/// Marker on the text rendered within the reload overlay mask.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReloadMaskText;

/// Reorder button action on an Overview card slot (Move Up).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardMoveUpButton(pub infiltrator_contract::overview_layout::OverviewCardKind);

/// Reorder button action on an Overview card slot (Move Down).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardMoveDownButton(pub infiltrator_contract::overview_layout::OverviewCardKind);

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
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct OverviewCardState(pub OverviewState);

/// Marker on the banner's status dot (success token fill).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusDot;

/// Marker on the banner's mode chip (accent fill, `on_accent` label).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewModeChip;

/// Marker on the banner's stop button (danger fill — demo semantics: the
/// official `Button` emits `Activate` but the shell wires no handler, so
/// the copy is honest about doing nothing yet).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StopButton;

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

/// Format one byte count for display, byte-exact with the iced reference
/// product (evidence: `crates/infiltrator-iced/src/utils.rs:3-17` —
/// 1024-based divisors carrying the decimal unit labels B / KB / MB / GB,
/// two decimals from KB up, an integer byte count below; the Overview page
/// renders rates as this formatter + `"/s"`, `src/view/overview.rs:205-208,
/// 241, 247`, and memory as this formatter directly, `:235`). Pure function.
pub fn format_byte_count(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

/// Format a byte-per-second rate for display: the shared [`format_byte_count`]
/// ladder (the iced reference spells rates the same way, plus `/s`) with an
/// honest zero for absent traffic and non-finite input. Pure function.
pub fn format_rate(bytes_per_second: f64) -> String {
    let rate = if bytes_per_second.is_finite() && bytes_per_second > 0.0 {
        bytes_per_second
    } else {
        0.0
    };
    // f64→u64 casts saturate (NaN was clamped above), so the reference
    // formatter's u64 input contract holds for any finite rate.
    format!("{}/s", format_byte_count(rate as u64))
}

/// Format a byte count for the memory chip: the shared [`format_byte_count`]
/// ladder; `None` renders an honest em-dash placeholder (the value is not
/// known — never a fabricated zero). Pure function.
pub fn format_memory(bytes: Option<u64>) -> String {
    bytes
        .map(format_byte_count)
        .unwrap_or_else(|| "—".to_owned())
}

/// Format CPU utilization percentage for the CPU chip.
pub fn format_cpu(percent: Option<f32>) -> String {
    percent
        .map(|p| format!("{:.1}%", p))
        .unwrap_or_else(|| "—".to_owned())
}

/// Format cumulative session total traffic for the TotalTraffic chip.
pub fn format_total_traffic(bytes: Option<u64>) -> String {
    bytes
        .map(format_byte_count)
        .unwrap_or_else(|| "—".to_owned())
}

/// The word the state line shows. The typed tri-state, spelled out
/// (zh-CN literals — see the module copy note).
pub(crate) fn state_label(state: OverviewState) -> &'static str {
    match state {
        OverviewState::Running => "运行中",
        OverviewState::Stopped => "已停止",
        OverviewState::Unavailable => "不可用",
    }
}

/// The proxy mode's segmented-control label (zh-CN literals — see the
/// module copy note).
pub(crate) fn mode_label(mode: ProxyMode) -> &'static str {
    match mode {
        ProxyMode::Rule => "规则模式",
        ProxyMode::Global => "全局模式",
        ProxyMode::Direct => "直连模式",
        ProxyMode::Script => "脚本模式",
    }
}

/// A metrics chip's stat name (zh-CN literals — see the module copy
/// note). The scene and the a11y group label both spell it through this
/// one function, so they can never drift apart.
pub(crate) fn chip_label(kind: OverviewChipKind) -> &'static str {
    match kind {
        OverviewChipKind::Connections => "连接数",
        OverviewChipKind::Memory => "内存",
        OverviewChipKind::Cpu => "CPU",
        OverviewChipKind::Upload => "上传",
        OverviewChipKind::Download => "下载",
        OverviewChipKind::TotalTraffic => "总流量",
    }
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
pub(crate) fn state_ink(state: OverviewState, palette: &UiPalette) -> Color {
    match state {
        OverviewState::Running => palette.ink,
        OverviewState::Stopped => palette.ink_dim,
        OverviewState::Unavailable => palette.on_accent,
    }
}

/// The banner's data-origin note. The demo fixture owns 演示数据; a live
/// core names the version it actually reported (an empty or unread
/// version stays honest as 版本读取中 — the failure line, not this note,
/// carries the failure verdict). Pure function.
pub(crate) fn banner_note(projection: &OverviewProjection) -> String {
    match projection.origin {
        OverviewOrigin::LiveCore => match projection.core_version.as_deref() {
            Some(version) if !version.trim().is_empty() => {
                format!("实时内核 · {version}")
            }
            _ => "实时内核 · 版本读取中".to_owned(),
        },
        OverviewOrigin::Demo => "演示数据 · 未接实时内核".to_owned(),
    }
}

/// The banner's fill: the accent container token, or the danger token
/// while unavailable — the whole-banner failure projection.
pub(crate) fn card_fill(state: OverviewState, palette: &UiPalette) -> Color {
    match state {
        OverviewState::Unavailable => palette.danger,
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
            position_type: bevy::ui::PositionType::Absolute,
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
            ( { icon_scene(IconId::Activity, 24.0, palette.accent) } ),
            ( Text({ "内核重载中 · 保持上一帧快照 (Reloading Core)".to_owned() }) OverviewReloadMaskText TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
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
        Children [
            ( { banner_scene(projection, palette) } ),
            ( { crate::pages::overview_cards::mode_segmented_controller_scene_with_snapshot(&projection.proxy_mode, palette) } ),
            ( { traffic_card_scene(projection, history, palette) } ),
            ( { chips_row_scene(projection, palette) } ),
            ( { crate::pages::overview_cards::master_switches_scene_with_snapshot(&projection.system_toggles, palette) } ),
            ( { crate::pages::overview_cards::active_exit_node_scene_with_snapshot(&projection.active_exit, palette) } ),
            ( { crate::pages::overview_public_ip::public_ip_probe_card_scene_with_snapshot(&projection.public_ip, palette) } ),
            ( { crate::pages::overview_topology::topology_chain_scene_with_snapshot(&projection.traffic_topology, palette) } ),
            ( { crate::pages::overview_cards::subscription_quota_scene_with_snapshot(&projection.subscription_quota, palette) } ),
            ( { reload_mask_scene(palette) } ),
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
    let state = state_label(projection.state).to_owned();
    let state_node = status_semantic_node(state_label(projection.state));
    let chip = mode_label(projection.mode).to_owned();
    let failure = projection.failure_text().to_owned();
    let note = banner_note(projection);
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
        BackgroundColor({ card_fill(projection.state, palette) })
        OverviewStatusCard
        OverviewCardState({ projection.state })
        Children [
            (
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            (
                                Node {
                                    width: px(10.0),
                                    height: px(10.0),
                                    flex_shrink: 0.0,
                                    border_radius: BorderRadius::all(Val::Px(5.0)),
                                }
                                BackgroundColor({ palette.success })
                                StatusDot
                            ),
                            (
                                Text({ state }) OverviewLine(OverviewLineKind::State)
                                TextRole(Role::Display)
                                template_value(state_node)
                            ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                            flex_wrap: FlexWrap::Wrap,
                            row_gap: Val::Px(space::S4),
                        }
                        Children [
                            (
                                Node {
                                    padding: UiRect::all(Val::Px(space::S4)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                OverviewModeChip
                                Children [
                                    ( Text({ chip }) OverviewLine(OverviewLineKind::ModeChip) TextRole(Role::Caption) OnAccentText ),
                                ]
                            ),
                            ( Text({ note }) OverviewLine(OverviewLineKind::BannerNote) TextRole(Role::Caption) ),
                            ( Text({ failure }) OverviewLine(OverviewLineKind::Failure) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( { speedtest_button_scene(palette) } ),
                    ( { stop_area_scene(projection, palette) } ),
                ]
            ),
        ]
    }
}

/// The banner's trailing action: demo sources keep the danger stop pill
/// (demo semantics, as before); live sources get the honest lifecycle
/// caption instead — the pump exposes no core-lifecycle command yet
/// (0.30), and a stop button that stopped nothing would be a lie. Boxed:
/// the two arms are different scene types.
fn stop_area_scene(projection: &OverviewProjection, palette: &UiPalette) -> Box<dyn Scene> {
    match projection.origin {
        OverviewOrigin::Demo => Box::new(stop_button_scene(palette)),
        OverviewOrigin::LiveCore => Box::new(lifecycle_caption_scene()),
    }
}

/// The banner's stop button: the official unstyled `Button` in a danger
/// pill skin. `Activate` carries no business action — demo semantics.
fn stop_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.danger })
        StopButton
        Button
        Children [
            ( Text({ "停止代理".to_owned() }) TextRole(Role::BodyStrong) OnAccentText ),
        ]
    }
}

/// The live-core banner's trailing slot: a plain, typed caption stating
/// that core lifecycle control ships after 0.30. A plain node (never a
/// `Button`) — it must not even look pressable, matching the sidebar's
/// 未迁移 nav entries.
fn lifecycle_caption_scene() -> impl Scene + use<> {
    bsn! {
        Node {
            padding: UiRect::horizontal(Val::Px(space::S12)),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
        }
        Children [
            ( Text({ "核心生命周期控制 · 0.30 后续接入".to_owned() }) TextRole(Role::Caption) ),
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
            Box::new(plain_caption("实时流量".to_owned())),
            Box::new(rates_row_scene(
                format_rate(projection.upload_bps),
                format_rate(projection.download_bps),
            )),
            Box::new(scale_line_scene(&scale)),
            Box::new(chart_scene_with_scale(
                up,
                down,
                CHART_WIDTH_PX,
                CHART_HEIGHT_PX,
                smooth,
                Some(scale.max_bps as f32),
            )),
        ],
        palette,
    )
}

/// The up/down rates side by side on one row (the reference layout).
fn rates_row_scene(upload: String, download: String) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S16),
        }
        Children [
            ( { rate_line("↑ ", OverviewLineKind::Upload, upload) } ),
            ( { rate_line("↓ ", OverviewLineKind::Download, download) } ),
        ]
    }
}

fn scale_line_scene(
    scale: &infiltrator_contract::traffic_scale::TrafficScaleSnapshot,
) -> impl Scene + use<> {
    let label = format_scale(scale);
    bsn! {
        Node {
            width: percent(100),
            min_height: px(16.0),
        }
        Children [
            ( Text({ label }) OverviewLine(OverviewLineKind::Scale) TextRole(Role::Mono) ),
        ]
    }
}

pub(crate) fn format_scale(
    scale: &infiltrator_contract::traffic_scale::TrafficScaleSnapshot,
) -> String {
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
            ( Text({ format!("{arrow}{value}") }) OverviewLine(kind) TextRole(Role::Mono) ),
        ]
    }
}

/// A plain caption line inside a card.
fn plain_caption(label: String) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
        }
        Children [
            ( Text({ label }) TextRole(Role::Caption) ),
        ]
    }
}

/// The metrics band: four stat chips sharing the width evenly. Each chip
/// root carries a labeled `Group` semantic node ("name value") that the
/// refresh observer restamps alongside the visible value.
fn chips_row_scene(projection: &OverviewProjection, palette: &UiPalette) -> impl Scene + use<> {
    let connections = projection.active_connections.to_string();
    let memory = format_memory(projection.memory_bytes);
    let cpu = format_cpu(projection.cpu_percent);
    let upload = format_rate(projection.upload_bps);
    let download = format_rate(projection.download_bps);
    let total = format_total_traffic(projection.total_traffic_bytes);

    let connections_node = stat_group_semantic_node(&format!(
        "{} {connections}",
        chip_label(OverviewChipKind::Connections)
    ));
    let memory_node = stat_group_semantic_node(&format!(
        "{} {memory}",
        chip_label(OverviewChipKind::Memory)
    ));
    let cpu_node =
        stat_group_semantic_node(&format!("{} {cpu}", chip_label(OverviewChipKind::Cpu)));
    let upload_node = stat_group_semantic_node(&format!(
        "{} {upload}",
        chip_label(OverviewChipKind::Upload)
    ));
    let download_node = stat_group_semantic_node(&format!(
        "{} {download}",
        chip_label(OverviewChipKind::Download)
    ));
    let total_node = stat_group_semantic_node(&format!(
        "{} {total}",
        chip_label(OverviewChipKind::TotalTraffic)
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
        Children [
            (
                { stat_chip_scene(IconId::Activity, chip_label(OverviewChipKind::Connections).to_owned(), connections, palette) }
                OverviewChip(OverviewChipKind::Connections)
                template_value(connections_node)
            ),
            (
                { stat_chip_scene(IconId::Zap, chip_label(OverviewChipKind::Memory).to_owned(), memory, palette) }
                OverviewChip(OverviewChipKind::Memory)
                template_value(memory_node)
            ),
            (
                { stat_chip_scene(IconId::Settings, chip_label(OverviewChipKind::Cpu).to_owned(), cpu, palette) }
                OverviewChip(OverviewChipKind::Cpu)
                template_value(cpu_node)
            ),
            (
                { stat_chip_scene(IconId::ArrowUp, chip_label(OverviewChipKind::Upload).to_owned(), upload, palette) }
                OverviewChip(OverviewChipKind::Upload)
                template_value(upload_node)
            ),
            (
                { stat_chip_scene(IconId::ArrowDown, chip_label(OverviewChipKind::Download).to_owned(), download, palette) }
                OverviewChip(OverviewChipKind::Download)
                template_value(download_node)
            ),
            (
                { stat_chip_scene(IconId::Globe, chip_label(OverviewChipKind::TotalTraffic).to_owned(), total, palette) }
                OverviewChip(OverviewChipKind::TotalTraffic)
                template_value(total_node)
            ),
        ]
    }
}

/// The traffic topology chain card: 4 linked stage chips with connecting arrows (">").
/// 4-stage network traffic topology chain scene (BEVY-GAP-018).
pub fn topology_chain_scene(palette: &UiPalette) -> impl Scene + use<> {
    crate::pages::overview_topology::topology_chain_scene(palette)
}

/// Subscription quota and billing cycle visualization card (BEVY-GAP-020).
pub fn subscription_quota_scene(palette: &UiPalette) -> impl Scene + use<> {
    crate::pages::overview_cards::subscription_quota_scene(palette)
}

fn bind_overview_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<OverviewPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(OverviewPageBound);
    commands.add_observer(apply_overview_projection);
    commands.add_observer(on_topology_stage_activated);
    commands.add_observer(on_overview_master_switch_activated);
    commands.add_observer(on_overview_mode_segment_activated);
    commands.add_observer(on_overview_speedtest_activated);
    commands.add_observer(on_overview_speedtest_concurrency_stepped);
    commands.add_observer(on_overview_speedtest_detail_activated);
    commands.add_observer(on_overview_public_ip_refresh_activated);
    commands.add_observer(on_overview_card_move_up_activated);
    commands.add_observer(on_overview_card_move_down_activated);
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
/// (2 / 3 / 6 / 6) so the six tiles reflow on narrow windows. Column count is
/// read from the same authoritative table the Iced surface uses; the responsive
/// parity guard keeps the two in step.
pub fn sync_overview_metrics_columns(
    ctx: Option<Res<infiltrator_bevy_widgets::responsive::ResponsiveContext>>,
    mut chips: Query<&mut Node, With<OverviewChip>>,
) {
    let Some(ctx) = ctx else {
        return;
    };
    let columns = match ctx.breakpoint {
        infiltrator_bevy_widgets::theme::Breakpoint::Compact => 2,
        infiltrator_bevy_widgets::theme::Breakpoint::Medium => 3,
        infiltrator_bevy_widgets::theme::Breakpoint::Expanded
        | infiltrator_bevy_widgets::theme::Breakpoint::Ultra => 6,
    };
    let basis = Val::Percent(
        infiltrator_bevy_widgets::fluid_grid::FluidCardGrid::wrapped_item_percent(columns),
    );
    for mut node in &mut chips {
        if node.flex_basis != basis {
            node.flex_basis = basis;
        }
    }
}
