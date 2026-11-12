//! The Overview page: run state banner, live traffic card and the four
//! stat chips, rendered from the shared [`OverviewProjection`].
//!
//! **Update seam**: mutable nodes carry typed markers ([`OverviewLine`],
//! [`OverviewChip`] + the widget layer's `StatChipValue`,
//! [`OverviewStatusCard`] / [`OverviewCardState`]), and the dashboard's own
//! cards carry theirs (`overview_cards::OverviewRoutingIntentSegmentPill`,
//! `overview_restamp::OverviewMasterSwitchButton`): one control, one marker,
//! one mount.
//! [`OverviewPagePlugin`] registers [`apply_overview_projection`] once at product assembly, and the router fires the first paint with the mounted
//! projection right after `spawn_scene`. From then on, an
//! [`OverviewProjectionUpdated`] trigger restamps texts, inks, pill
//! selection bits and the banner's stored state *in place*: entity ids
//! never change, nothing is re-mounted, nothing polls (charter law:
//! observers change components, never rebuild trees).
//!
//! **Theme seam**: every filled node of the page (banner, status dot,
//! routing chip, stop button) carries a marker and is repainted every frame
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

use crate::history::{ScrubberAction, TrafficHistory, TrafficReplay, chart_inputs};
use crate::pages::overview_card_order;
use crate::pages::overview_hero::{banner_scene, globe_slot_scene};
use crate::pages::overview_policy_groups::{
    OverviewPolicyGroupCard, PolicyGroupPickerState, close_policy_group_picker_on_route,
    on_policy_group_activated, on_policy_group_request_close, policy_group_card_scene,
    policy_group_picker_scene, sync_policy_group_grid, sync_policy_group_picker,
};
use crate::pages::overview_public_ip::on_overview_public_ip_refresh_activated;
use crate::pages::overview_quota_copy;
use crate::pages::overview_rates;
use crate::pages::overview_resource_gauges::{resource_gauges_scene, sync_resource_gauges};
use crate::pages::overview_restamp::{
    apply_overview_projection, on_overview_master_switch_activated, sync_overview_replay,
};
use crate::pages::overview_service_health::{
    animate_service_health_pulses, service_health_card_scene, sync_service_health_tiles,
};
use crate::pages::overview_speedtest::{
    on_overview_speedtest_activated, on_overview_speedtest_concurrency_stepped,
    on_overview_speedtest_detail_activated,
};
use crate::pages::overview_topology::on_topology_stage_activated;
use crate::pages::overview_topology_drilldown;
use crate::pages::overview_traffic_scenes::{traffic_detail_card_scene, traffic_mini_card_scene};
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
use bevy::ui::PositionType;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, ComputedNode, Display, FlexDirection, FlexWrap, JustifyContent,
    Node, Overflow, Val, percent, px,
};
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_bevy_widgets::fluid_grid::{FluidCardGrid, compute_ideal_column_layout};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::{Breakpoint, space};
use infiltrator_bevy_widgets::waveform::WaveformSeries;
use infiltrator_contract::design_resources;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_contract::system_toggle::SystemToggleState;
use infiltrator_presentation::dashboard_hero_projection::ResourceGaugeProjection;
use infiltrator_presentation::dashboard_hero_projection::TrafficWaveformProjection;
use infiltrator_presentation::shell_readout::{observed_rate, rate_copy};
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_ui_presentation::core_status_projection::{
    OverviewBannerVerdict, banner_verdict_copy, project_banner_verdict, source_copy,
};

#[path = "overview_reactive.rs"]
pub mod overview_reactive;

#[path = "overview_pulse.rs"]
pub mod overview_pulse;

use bevy::feathers::theme::ThemeTextColor;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::rem;

/// One upload/download mini card's preferred width in baseline pixels. Below
/// two of them plus the gutter the pair stacks, so a rate never breaks in the
/// middle of its unit.
const TRAFFIC_MINI_BASIS_PX: f32 = 176.0;

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

/// Marker component on the historical trend detail chart and scrubber controls, hidden in compact mobile mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewTrafficDetailSection;

/// Marker on the resource gauge card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewResourceGaugeCard;

/// Marks the Overview service-health card for sectioned screenshot capture.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewServiceHealthCard;

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
    /// The banner's data-origin note: 演示数据 for the fixture, the live
    /// core's real version for the pump (BEVY-005).
    BannerNote,
    /// Uplink peak rate readout (峰值 ...).
    UploadPeak,
    /// Downlink peak rate readout (峰值 ...).
    DownloadPeak,
    /// Cumulative uplink volume (累计 ...).
    UploadTotal,
    /// Cumulative downlink volume (累计 ...).
    DownloadTotal,
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

impl Default for OverviewCardSlot {
    fn default() -> Self {
        Self(default_overview_card_kind())
    }
}

/// Marker on the container that holds the reorderable card slots.
///
/// The cards live in one container so the page can put them back in the order
/// the owner published: a move publishes the shared reorder intent, the owner
/// answers with the next layout snapshot, and the page renders that order.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewCardSlots;

/// Marker on the reload / reconnect graceful degradation overlay mask (DUAL-03-13).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReloadMask;

/// Marker on the text rendered within the reload overlay mask.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReloadMaskText;

/// Reorder button action on an Overview card slot (Move Up).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardMoveUpButton(pub OverviewCardKind);

/// Marker on one card slot's shared reorder controls row. Compact mobile
/// hides the whole row: touch reorder belongs to a future gesture slice,
/// and the floating arrows read as desktop HUD traces.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewCardMoveControls;

impl Default for OverviewCardMoveUpButton {
    fn default() -> Self {
        Self(default_overview_card_kind())
    }
}

/// Reorder button action on an Overview card slot (Move Down).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewCardMoveDownButton(pub OverviewCardKind);

impl Default for OverviewCardMoveDownButton {
    fn default() -> Self {
        Self(default_overview_card_kind())
    }
}

/// The card kind a scene marker carries when the scene does not name one.
///
/// The canonical order's first card is the default: a marker without a card
/// is the page's first card, not an invented one.
fn default_overview_card_kind() -> OverviewCardKind {
    OverviewCardKind::DEFAULT_ORDER
        .first()
        .copied()
        .unwrap_or(OverviewCardKind::RoutingIntentSegment)
}

/// Marker for the Overview metrics chip band container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewMetricsBand;

/// Marker on the derived reactive load-score text (BEVY-028).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewReactiveSummary;

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
    rate_copy(&observed_rate(Some(bytes_per_second)), &Lang("en-US"))
}

/// Format a byte count for the memory chip: the shared [`format_bytes`]
/// ladder; `None` renders an honest em-dash placeholder (the value is not
/// known — never a fabricated zero). Pure function.
pub fn format_memory(bytes: &ResourceAvailability<u64>) -> String {
    match bytes {
        ResourceAvailability::Available(bytes) => format_bytes(*bytes),
        ResourceAvailability::NotObserved
        | ResourceAvailability::Unsupported
        | ResourceAvailability::Failed(_) => "—".to_owned(),
    }
}

/// Format CPU utilization percentage for the CPU chip.
pub fn format_cpu(percent: &ResourceAvailability<f32>) -> String {
    match percent {
        ResourceAvailability::Available(percent) => format!("{percent:.1}%"),
        ResourceAvailability::NotObserved
        | ResourceAvailability::Unsupported
        | ResourceAvailability::Failed(_) => "—".to_owned(),
    }
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

pub(crate) fn is_vpn_unauthorized(projection: &OverviewProjection) -> bool {
    matches!(
        projection.system_toggles.tun,
        SystemToggleState::Failed { .. } | SystemToggleState::Unsupported { .. }
    ) || matches!(
        projection.system_toggles.system_proxy,
        SystemToggleState::Failed { .. }
    )
}

pub(crate) fn is_proxy_or_vpn_active(projection: &OverviewProjection) -> bool {
    projection.system_toggles.system_proxy.is_enabled()
        || projection.system_toggles.tun.is_enabled()
}

pub fn banner_verdict(projection: &OverviewProjection) -> OverviewBannerVerdict {
    let origin = match projection.origin {
        #[cfg(any(feature = "capture-tools", test))]
        OverviewOrigin::Demo => SurfaceOrigin::Demo,
        OverviewOrigin::LiveCore => SurfaceOrigin::Live,
    };
    project_banner_verdict(
        origin,
        &projection.lifecycle,
        is_proxy_or_vpn_active(projection),
        is_vpn_unauthorized(projection),
    )
}

pub fn banner_state_text(projection: &OverviewProjection, language: &str) -> String {
    let verdict = banner_verdict(projection);
    banner_verdict_copy(
        verdict,
        &projection.lifecycle,
        &Lang(language),
        is_vpn_unauthorized(projection),
    )
}

/// The state line's ink: readable on the accent container while
/// running/stopped; semantic state ink otherwise. Tokens only.
pub fn state_ink(
    projection: Option<&OverviewProjection>,
    state: &CoreLifecycle,
    palette: &UiPalette,
) -> Color {
    if let Some(projection) = projection {
        match banner_verdict(projection) {
            OverviewBannerVerdict::Demo => palette.ink,
            OverviewBannerVerdict::RealRunning => palette.ink,
            OverviewBannerVerdict::CoreStopped => palette.ink_dim,
            OverviewBannerVerdict::VpnUnauthorized | OverviewBannerVerdict::Transitioning => {
                palette.warning
            }
            OverviewBannerVerdict::MissingHost => palette.danger,
        }
    } else {
        match state {
            CoreLifecycle::Running | CoreLifecycle::Ready => palette.ink,
            CoreLifecycle::Stopped => palette.ink_dim,
            CoreLifecycle::Starting | CoreLifecycle::Stopping => palette.warning,
            CoreLifecycle::Failed => palette.danger,
        }
    }
}

pub fn status_dot_color(projection: Option<&OverviewProjection>, palette: &UiPalette) -> Color {
    let Some(projection) = projection else {
        return palette.ink_dim;
    };
    match banner_verdict(projection) {
        OverviewBannerVerdict::Demo => palette.accent,
        OverviewBannerVerdict::RealRunning => palette.success,
        OverviewBannerVerdict::VpnUnauthorized | OverviewBannerVerdict::Transitioning => {
            palette.warning
        }
        OverviewBannerVerdict::MissingHost => palette.danger,
        OverviewBannerVerdict::CoreStopped => palette.ink_dim,
    }
}

/// The banner's data-origin note. The demo fixture owns 演示数据; a live
/// core names the version it actually reported (an empty or unread
/// version stays honest as 版本读取中 — the failure line, not this note,
/// carries the failure verdict). Pure function.
pub fn banner_note(projection: &OverviewProjection, language: &str) -> String {
    source_copy(
        match projection.origin {
            #[cfg(any(feature = "capture-tools", test))]
            OverviewOrigin::Demo => SurfaceOrigin::Demo,
            OverviewOrigin::LiveCore => SurfaceOrigin::Live,
        },
        projection.core_version.as_deref(),
        &Lang(language),
    )
}

/// The banner's fill: the standard opaque card surface. Availability is
/// carried by the status dot and state ink, not by a full-bleed semantic fill.
pub fn card_fill(
    _projection: Option<&OverviewProjection>,
    _state: &CoreLifecycle,
    palette: &UiPalette,
) -> Color {
    palette.surface
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
                row_gap: rem(space::S8),
            }
            BackgroundColor({ scrim })
            OverviewReloadMask
            Children [
                @{ icon_scene(IconId::Activity, 24.0, palette.accent) }
                --
                LocalizedText::plain("overview_content_effecting") OverviewReloadMaskText TextRole(design_resources::TextRole::BodyStrong) ThemeTextColor({ Token::OnAccent })
            ]
    }
}

/// The Overview page: hero band, the reorderable card grid and the page's
/// own detail blocks, filling the shell's content slot. The detail blocks
/// follow the card band — the trend band first, then service health, the
/// resource gauges, the policy grid and the view-relative floating layers —
/// so the desktop page opens with dense dashboard modules, not a full-width
/// visualization. The trend band's series seed from [`chart_series`]: the
/// demo fixture's synthetic waves at mount, the live pump's recorded ring for
/// a live source.
pub fn overview_page(
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
    language: &str,
) -> impl Scene + use<> {
    let order = overview_card_order::published_order(&projection.layout);
    let cards =
        overview_card_order::overview_card_scenes(projection, history, palette, language, &order);
    let banner: Box<dyn Scene> = Box::new(bsn! { @{ banner_scene(projection, palette) } });
    let card_slots: Box<dyn Scene> = Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::FlexStart,
                column_gap: rem(space::S12),
                row_gap: rem(space::S16),
            }
            OverviewCardSlots
            Children [
                { cards }
            ]
    });
    let service_health: Box<dyn Scene> = Box::new(bsn! {
            Node {
                width: percent(100),
            }
            OverviewServiceHealthCard
            Children [
                @{ service_health_card_scene(&projection.speedtest, palette) }
            ]
    });
    let traffic_detail: Box<dyn Scene> =
        Box::new(traffic_detail_band_scene(projection, history, palette));
    let resource_gauges: Box<dyn Scene> = Box::new(bsn! {
            Node {
                width: percent(100),
            }
            OverviewResourceGaugeCard
            Children [
                @{ resource_gauges_scene(&resource_gauge_projection(projection), palette) }
            ]
    });
    let policy_groups_scene = || -> Box<dyn Scene> {
        Box::new(bsn! {
            Node {
                width: percent(100),
            }
            OverviewPolicyGroupCard
            Children [
                @{ policy_group_card_scene(&projection.policy_groups, palette) }
            ]
        })
    };
    let mut children: Vec<Box<dyn Scene>> = vec![banner];
    // The reference dashboard opens with the owner's card band, and the page's
    // own detail blocks follow it: the trend chart, then service health, the
    // resource gauges and the policy grid. The owner's card order stays
    // authoritative inside its band; the tier only decides that the band
    // precedes the page's detail blocks instead of burying it under them. On
    // the compact surface the trend band hides itself (the phone carries the
    // two mini readings), so one order serves both viewports.
    children.push(card_slots);
    children.push(traffic_detail);
    children.push(service_health);
    children.push(resource_gauges);
    children.push(policy_groups_scene());
    // The picker rides the page but not its scroll clip: the root is a
    // `FixedNode`, so it positions against the viewport like every other
    // overlay in this product.
    children.push(Box::new(bsn! { @{ policy_group_picker_scene(palette) } }));
    children.push(Box::new(bsn! { @{ globe_slot_scene() } }));
    children.push(Box::new(
        bsn! { @{ overview_reactive::reactive_summary_scene(palette) } },
    ));
    children.push(Box::new(bsn! { @{ reload_mask_scene(palette) } }));

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S16),
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::Overview)
            OverviewPageRoot
            ScrollArea
            Children [
                { children }
            ]
    }
}

/// The live traffic card: caption title, the up/down mono rate lines and
/// the WESL-glow waveform strip under each rate. The two readings share one
/// slot so the pair is read together; the page's large trend chart is **not**
/// inside this card — it owns the band under the card grid
/// ([`traffic_detail_band_scene`]), because a chart three times the height of
/// its neighbour leaves a hole in the row beside it.
pub(crate) fn traffic_card_scene(
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
    language: &str,
) -> impl Scene + use<> {
    let (up, down, _, _) = chart_inputs(projection, history);
    let upload = up.clone();
    let download = down.clone();
    let total_bytes = projection.total_traffic_bytes.unwrap_or(0);
    let waveform_proj = TrafficWaveformProjection::from_waveform(
        &projection.traffic_waveform,
        total_bytes / 2,
        total_bytes / 2,
    );
    let total_up = waveform_proj.total_up_formatted.clone();
    let total_down = waveform_proj.total_down_formatted.clone();
    let peak_up = Lang(language)
        .tr("峰值 {value}")
        .replace("{value}", &waveform_proj.peak_up_formatted);
    let peak_down = Lang(language)
        .tr("峰值 {value}")
        .replace("{value}", &waveform_proj.peak_down_formatted);
    let total_up = Lang(language)
        .tr("累计 {value}")
        .replace("{value}", &total_up);
    let total_down = Lang(language)
        .tr("累计 {value}")
        .replace("{value}", &total_down);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
            }
            OverviewTrafficCard
            Children [
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::FlexStart,
                    column_gap: rem(space::S12),
                    row_gap: rem(space::S12),
                }
                Children [
                    Node {
                        flex_grow: 1.0,
                        flex_basis: rem(TRAFFIC_MINI_BASIS_PX),
                        min_width: px(0.0),
                    }
                    Children [
                        @{ traffic_mini_card_scene(
                        TrafficMiniReading {
                            series: WaveformSeries::Upload,
                            icon: IconId::ArrowUp,
                            label_key: "overview_upload",
                            rate: rate_copy(&projection.readout.upload_bps, &Lang(language)),
                            peak: peak_up,
                            total: total_up,
                            samples: upload,
                        },
                            palette,
                        ) }
                    ]
                    --
                    Node {
                        flex_grow: 1.0,
                        flex_basis: rem(TRAFFIC_MINI_BASIS_PX),
                        min_width: px(0.0),
                    }
                    Children [
                        @{ traffic_mini_card_scene(
                        TrafficMiniReading {
                            series: WaveformSeries::Download,
                            icon: IconId::ArrowDown,
                            label_key: "overview_download",
                            rate: rate_copy(&projection.readout.download_bps, &Lang(language)),
                            peak: peak_down,
                            total: total_down,
                            samples: download,
                        },
                            palette,
                        ) }
                    ]
                ]
            ]
    }
}

/// The trend detail band: the page's own full-width block under the card
/// grid. The dual-series chart (the widget layer's [`chart_scene`] — upper
/// series the accent ink, lower the success ink, fade fills under both) is
/// the page's largest single reading, so it keeps a band of its own instead
/// of stretching one card slot. The series come from [`chart_series`]: the
/// demo fixture's synthetic waves at mount, the live pump's recorded ring for
/// a live source, and the widget's own `sync_charts` re-derives every ink
/// from the live palette, so a theme switch recolors the chart in place.
/// The compact surface hides the whole section: the phone dashboard carries
/// the two mini readings instead.
pub(crate) fn traffic_detail_band_scene(
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (up, down, smooth, scale) = chart_inputs(projection, history);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
            }
            OverviewTrafficDetailSection
            Children [
                @{ traffic_detail_card_scene(
                    projection,
                    up,
                    down,
                    smooth,
                    scale.max_bps as f32,
                    palette,
                ) }
            ]
    }
}

/// One channel's authoritative mini reading: the identity the card shows, the
/// values the page restamps, and the series the widget rasterizes. Bundled so
/// the two cards differ by data, never by a positional argument list.
pub(crate) struct TrafficMiniReading {
    pub(crate) series: WaveformSeries,
    pub(crate) icon: IconId,
    pub(crate) label_key: &'static str,
    pub(crate) rate: String,
    pub(crate) peak: String,
    pub(crate) total: String,
    pub(crate) samples: Vec<f32>,
}

/// The traffic topology chain card: 4 linked stage chips with connecting arrows (">").
/// 4-stage network traffic topology chain scene (BEVY-GAP-018).
#[cfg(any(feature = "capture-tools", test))]
pub fn topology_chain_scene(palette: &UiPalette) -> impl Scene + use<> {
    use crate::pages::overview_topology;

    overview_topology::topology_chain_scene(palette)
}

/// Subscription quota and billing cycle visualization card (BEVY-GAP-020).
#[cfg(any(feature = "capture-tools", test))]
pub fn subscription_quota_scene(palette: &UiPalette) -> impl Scene + use<> {
    use crate::pages::overview_cards;

    overview_cards::subscription_quota_scene(palette)
}

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct OverviewPagePlugin;

impl Plugin for OverviewPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrafficReplay>();
        app.init_resource::<overview_pulse::OverviewPulseState>();
        app.init_resource::<PolicyGroupPickerState>();
        overview_reactive::register(app);
        // The hero's status dot breathes around its live dot; the drilldown
        // writes its own texts, and its queries are disjoint from the
        // overview's scrubber status, so the two systems do not fight over
        // the same component.
        app.add_systems(
            Update,
            (
                sync_overview_card_columns,
                overview_rates::refresh,
                overview_quota_copy::replay,
                sync_overview_replay,
                overview_topology_drilldown::sync_topology_drilldown,
                overview_pulse::animate_status_dot,
                sync_service_health_tiles,
                sync_resource_gauges,
                sync_policy_group_grid,
                sync_policy_group_picker,
                close_policy_group_picker_on_route,
                animate_service_health_pulses,
                overview_card_order::sync_overview_card_order,
            ),
        );
        app.add_observer(apply_overview_projection);
        app.add_observer(on_policy_group_activated);
        app.add_observer(on_policy_group_request_close);
        app.add_observer(on_overview_scrubber_activated);
        app.add_observer(on_topology_stage_activated);
        app.add_observer(on_overview_master_switch_activated);
        app.add_observer(on_overview_speedtest_activated);
        app.add_observer(on_overview_speedtest_concurrency_stepped);
        app.add_observer(on_overview_speedtest_detail_activated);
        app.add_observer(on_overview_public_ip_refresh_activated);
        app.add_observer(overview_card_order::on_card_move_up_activated);
        app.add_observer(overview_card_order::on_card_move_down_activated);
        app.add_observer(overview_topology_drilldown::on_drilldown_navigate_activated);
        app.add_observer(overview_topology_drilldown::on_drilldown_close_activated);
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

/// The shared overview card's column count at each responsive tier.
///
/// The contract owns the same operator in
/// [`infiltrator_contract::responsive_viewport`]; this local resolver exists
/// so the Bevy UI can apply it to a different layout marker than the metrics
/// band without inventing a second breakpoint ladder.
pub fn overview_card_columns(breakpoint: Breakpoint) -> usize {
    match breakpoint {
        Breakpoint::Compact => 1,
        Breakpoint::Medium => 2,
        Breakpoint::Expanded => 2,
        Breakpoint::Ultra => 3,
    }
}

/// The authoritative resource gauge projection for the page.
pub(crate) fn resource_gauge_projection(
    projection: &OverviewProjection,
) -> ResourceGaugeProjection {
    ResourceGaugeProjection::new(
        match &projection.memory_bytes {
            ResourceAvailability::Available(bytes) => Some(*bytes),
            _ => None,
        },
        projection.memory_limit_bytes,
        match &projection.cpu_percent {
            ResourceAvailability::Available(percent) => Some(*percent),
            _ => None,
        },
    )
}

/// Pin overview cards to the shared tier column count (1 / 2 / 2 / 3).
///
/// The wrapping container owns the gutters; each card slot only changes its
/// width basis, so card order and reorder behavior stay the same across all
/// four tiers.
pub fn sync_overview_card_columns(
    ctx: Option<Res<ResponsiveContext>>,
    mut slots: Query<&mut Node, With<OverviewCardSlot>>,
) {
    let Some(ctx) = ctx else {
        return;
    };
    let basis = Val::Percent(FluidCardGrid::wrapped_item_percent(overview_card_columns(
        ctx.breakpoint,
    )));
    for mut node in &mut slots {
        if node.width != basis {
            node.width = basis;
        }
        if node.flex_basis != basis {
            node.flex_basis = basis;
        }
    }
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
