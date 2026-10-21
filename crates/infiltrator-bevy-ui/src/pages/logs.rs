//! The Logs page (运行日志): live ring buffer logs, log level filtering,
//! tag classifications, and keyword searching.
//!
//! **Update seam**: the mounted rows carry typed markers ([`LogMessageText`]
//! and [`LogRowMeta`]). The shared search replay restamps their runs in
//! place — no tree rebuilds — while [`apply_logs_projection`] republishes
//! [`LastLogsProjection`] for the recycler, the level filter and the follow
//! policy. The row itself is a flat band of two text blocks (the meta rail
//! and the message paragraph), so the stream reads as one tiled console
//! instead of a pile of capsule cards.
//!
//! The console's own controls — the soft-wrap switch and the append
//! indicator — live with the stream card, so the card frame, the controls
//! and the rows are one surface.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::logs_export::{LogsExportPlugin, LogsExportState};
use crate::pages::logs_follow::{LogFollowLabel, LogsFollowPlugin};
use crate::pages::logs_ring::LogsRing;
use crate::pages::logs_rows::{LogRowsContainer, LogsRowsPlugin, log_row_scene};
use crate::pages::logs_search::{LogsSearchPlugin, level_filter_strip_scene, logs_search_scene};
use crate::pages::logs_virtual::{LOGS_VIRTUAL_THRESHOLD, LogsVirtualNodes};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Local, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::{LineBreak, TextLayout};
use bevy::time::Time;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::button::{
    ButtonLabel, ButtonSize, ButtonVariant, ControlVisual, button_with_label_scene,
};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::switch::{
    SwitchKnob, SwitchPosition, switch_knob_scene, switch_track_scene,
};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_runs::TextRuns;
use infiltrator_bevy_widgets::theme::space;
#[cfg(any(feature = "capture-tools", test))]
use infiltrator_contract::fixture_identities::{HK_PRIMARY, PROXIES, US};
use infiltrator_contract::logs::LogLevel;
use lurker_core_contract::page::PageStatus;
use lurker_core_contract::session::SessionToken;

/// Root marker on the Logs page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct LogsPageRoot;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogsLine(pub LogsLineKind);

/// Different text lines on the logs page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogsLineKind {
    /// Overview summary: total log lines count.
    #[default]
    Summary,
}

/// Marker for a log entry's message paragraph.
///
/// The paragraph is ONE text block: the entry's message is its own runs, so
/// the shared search replay can restamp the highlighted runs in place.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(TextRuns)]
pub struct LogMessageText(pub usize);

/// Marker for the "Clear Logs" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearLogsButton;

/// Marker for the "Pause Logs / Scroll Lock" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ControlVisual)]
pub struct PauseLogsButton;

/// Marker for the "Export Logs" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportLogsButton;

/// Color for log level tag.
pub fn log_level_color(level: LogLevel, palette: &UiPalette) -> Color {
    match level {
        LogLevel::Debug | LogLevel::Unknown => palette.ink_dim,
        LogLevel::Info => palette.accent,
        LogLevel::Warn => palette.warning,
        LogLevel::Error => palette.danger,
    }
}

/// A single log entry item.
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub level: LogLevel,
    pub tag: String,
    pub message: String,
}

/// Snapshot of the Logs domain.
#[derive(Clone, Debug, PartialEq)]
pub struct LogsProjection {
    pub status: PageStatus,
    pub generation: u64,
    pub session_token: Option<SessionToken>,
    pub total_entries: usize,
    pub active_level: Option<LogLevel>,
    pub entries: Vec<LogEntry>,
}

impl LogsProjection {
    /// Believable demo fixture for the Logs page.
    ///
    /// The records follow the runtime log format the core parses — the level,
    /// the timestamp and the module tag are separate fields, so the message
    /// carries only the payload. A message that repeated the tag would show
    /// the module twice in one line.
    #[cfg(any(feature = "capture-tools", test))]
    pub fn demo() -> Self {
        Self {
            status: PageStatus::Ready,
            generation: 0,
            session_token: None,
            total_entries: 5,
            active_level: None,
            entries: vec![
                LogEntry {
                    id: 1,
                    timestamp: "10:14:02.124".to_owned(),
                    level: LogLevel::Info,
                    tag: "TCP".to_owned(),
                    message: format!("127.0.0.1:54120 --> api.github.com:443 match DomainSuffix(github.com) using {PROXIES}[{HK_PRIMARY}]"),
                },
                LogEntry {
                    id: 2,
                    timestamp: "10:14:03.018".to_owned(),
                    level: LogLevel::Info,
                    tag: "DNS".to_owned(),
                    message: "resolve manifest.googlevideo.com via https://1.1.1.1/dns-query -> 172.217.160.78 (32ms)".to_owned(),
                },
                LogEntry {
                    id: 3,
                    timestamp: "10:14:04.550".to_owned(),
                    level: LogLevel::Warn,
                    tag: "TUN".to_owned(),
                    message: "high socket buffer pressure (85% capacity reached on utun9)".to_owned(),
                },
                LogEntry {
                    id: 4,
                    timestamp: "10:14:05.102".to_owned(),
                    level: LogLevel::Debug,
                    tag: "ROUTING".to_owned(),
                    message: "process matched: Discord (pid: 11024) -> rule: DOMAIN-SUFFIX discord.gg".to_owned(),
                },
                LogEntry {
                    id: 5,
                    timestamp: "10:14:08.882".to_owned(),
                    level: LogLevel::Error,
                    tag: "PROXY".to_owned(),
                    message: format!("dial timeout on backup node {US} (after 5000ms)"),
                },
            ],
        }
    }
}

/// The typed event dispatched when logs data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct LogsProjectionUpdated(pub LogsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastLogsProjection(pub Option<LogsProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn logs_page(projection: &LogsProjection, palette: &UiPalette) -> impl Scene + use<> {
    // BANDROID-010: large ring buffers mount through the recycler (bounded
    // window + spacers). Small buffers keep the full-mount vocabulary.
    let virtualized = projection.entries.len() > LOGS_VIRTUAL_THRESHOLD;
    let log_scenes: Vec<Box<dyn Scene>> = if virtualized {
        Vec::new()
    } else {
        projection
            .entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| Box::new(log_row_scene(idx, entry, palette)) as Box<dyn Scene>)
            .collect()
    };

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
            PageRoot(Route::Logs)
            LogsPageRoot
            ScrollArea
            Children [
                @{ header_card_scene(projection.total_entries, palette) }
                --
                @{ logs_search_scene(palette) }
                --
                @{ logs_stream_scene(log_scenes, virtualized, projection.active_level, palette) }
            ]
    }
}

/// The page toolbar: the stream summary and the three console actions.
///
/// The actions carry a real hierarchy — the export is the affirmative
/// (secondary) control, the clear is a hairline outline, and the scroll lock
/// is a ghost toggle that only paints when the stream is actually held.
pub fn header_card_scene(total_entries: usize, palette: &UiPalette) -> impl Scene + use<> {
    let header_a11y = accesskit::Node::new(accesskit::Role::Header);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S16),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: rem(space::S8),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("logs_toolbar_label")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::FileText, 36.0, palette) }
                            --
                            LocalizedText::new("logs_buffer_summary", vec![("count", total_entries.to_string())]) LogsLine(LogsLineKind::Summary) TextRole(Role::Heading)
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            @{ scroll_lock_button_scene(palette) }
                            --
                            @{ export_button_scene(palette) }
                            --
                            @{ clear_button_scene(palette) }
                        ]
                    ]
        })],
        palette,
    )
}

/// The scroll-lock toggle: a ghost control whose label follows the follow
/// state, so the held stream is readable from the label itself.
fn scroll_lock_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain("logs_scroll_lock") TextRole(Role::Body) ButtonLabel LogFollowLabel },
                ButtonVariant::Ghost,
                ButtonSize::Md,
                palette,
            ) }
            PauseLogsButton
    })
}

/// The export action: the page's affirmative control.
fn export_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain("logs_export_action") TextRole(Role::Body) ButtonLabel },
                ButtonVariant::Secondary,
                ButtonSize::Md,
                palette,
            ) }
            ExportLogsButton
    })
}

/// The clear action: a quiet hairline control, never a filled one.
fn clear_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain("common_clear") TextRole(Role::Body) ButtonLabel },
                ButtonVariant::Outline,
                ButtonSize::Md,
                palette,
            ) }
            ClearLogsButton
    })
}

// ---- The console stream ----------------------------------------------------

/// The soft-wrap switch: it decides whether a long record stays on one line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogSoftWrapSwitch;

/// The append indicator: the console's live rule above the newest records.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamAppendIndicator;

/// How a long log line is laid out inside the stream column.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLineWrap {
    /// The line wraps at the column edge (the console default).
    #[default]
    Soft,
    /// The line stays on one line and is clipped at the column edge.
    Single,
}

/// The stream's append indicator phase.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub enum StreamAppendPulse {
    /// No new records since the last fade.
    #[default]
    Idle,
    /// New records arrived; the marker fades back over the remaining time.
    Fading {
        /// Seconds left before the marker is back at its resting tint.
        remaining: f32,
    },
}

/// How long the append indicator takes to fade back after new records.
const STREAM_APPEND_FADE_SECS: f32 = 2.5;

/// The append indicator's resting tint: the console's live rule.
const STREAM_APPEND_REST_ALPHA: f32 = 0.16;

/// The append indicator's flash: the moment new records land.
const STREAM_APPEND_FLASH_ALPHA: f32 = 0.85;

/// The console card: the stream header, the append rule and the rows.
pub fn logs_stream_scene(
    log_scenes: Vec<Box<dyn Scene>>,
    virtualized: bool,
    active_level: Option<LogLevel>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let rows_scene: Box<dyn Scene> = if virtualized {
        // The recycler owns the row children; the container keeps its
        // `LogRowsContainer` marker so the shared search display reduction
        // still addresses it.
        Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
            }
            LogRowsContainer
            LogsVirtualNodes
        })
    } else {
        Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
            }
            LogRowsContainer
            Children [
                { log_scenes }
            ]
        })
    };
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S8),
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    column_gap: rem(space::S12),
                                    flex_wrap: FlexWrap::Wrap,
                                    row_gap: rem(space::S4),
                                }
                                Children [
                                    LocalizedText::plain("logs_stream_title") TextRole(Role::BodyStrong)
                                    --
                                    @{ soft_wrap_switch_scene(palette) }
                                ]
                                --
                                @{ level_filter_strip_scene(active_level, palette) }
                                --
                                Node {
                                    width: percent(100),
                                    height: rem(space::S2),
                                    border_radius: BorderRadius::MAX,
                                }
                                BackgroundColor({ palette.accent.with_alpha(STREAM_APPEND_REST_ALPHA) })
                                StreamAppendIndicator
                            ]
            }),
            rows_scene,
        ],
        palette,
    )
}

/// The console's soft-wrap switch: the shared track with the shared knob.
fn soft_wrap_switch_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let mut semantic = accesskit::Node::new(accesskit::Role::Switch);
    semantic.set_label("Soft wrap");
    semantic.set_toggled(accesskit::Toggled::True);
    Box::new(bsn! {
            @{ switch_track_scene(palette, palette.surface_elevated, palette.border) }
            LogSoftWrapSwitch
            Button
            AccessibilityNode(semantic)
            Children [
                @{ switch_knob_scene(palette.accent, SwitchPosition::On) }
                SwitchKnob({ SwitchPosition::On })
            ]
    })
}

/// The soft-wrap switch was pressed: the console's line layout flips.
fn toggle_soft_wrap(
    activate: On<Activate>,
    mut switches: Query<&mut AccessibilityNode, With<LogSoftWrapSwitch>>,
    mut state: ResMut<LogLineWrap>,
    mut knobs: Query<&mut SwitchKnob>,
) {
    let Ok(mut semantic) = switches.get_mut(activate.entity) else {
        return;
    };
    let next = match *state {
        LogLineWrap::Soft => LogLineWrap::Single,
        LogLineWrap::Single => LogLineWrap::Soft,
    };
    *state = next;
    let soft = matches!(next, LogLineWrap::Soft);
    let position = SwitchPosition::from_enabled(soft);
    for mut knob in &mut knobs {
        knob.0 = position;
    }
    semantic.0.set_toggled(if soft {
        accesskit::Toggled::True
    } else {
        accesskit::Toggled::False
    });
}

/// Apply the console's line layout to every mounted message paragraph.
fn apply_line_wrap(
    state: Res<LogLineWrap>,
    mut messages: Query<&mut TextLayout, With<LogMessageText>>,
) {
    let linebreak = match *state {
        LogLineWrap::Soft => LineBreak::WordOrCharacter,
        LogLineWrap::Single => LineBreak::NoWrap,
    };
    for mut layout in &mut messages {
        if layout.linebreak != linebreak {
            layout.linebreak = linebreak;
        }
    }
}

/// The append indicator: new records brighten the rule, then it fades back.
fn advance_append_pulse(
    time: Res<Time>,
    projection: Res<LastLogsProjection>,
    mut pulse: ResMut<StreamAppendPulse>,
    mut seen_total: Local<usize>,
    mut indicators: Query<&mut BackgroundColor, With<StreamAppendIndicator>>,
) {
    let total = projection
        .0
        .as_ref()
        .map(|projection| projection.total_entries)
        .unwrap_or_default();
    if total > *seen_total {
        *seen_total = total;
        *pulse = StreamAppendPulse::Fading {
            remaining: STREAM_APPEND_FADE_SECS,
        };
    }
    let alpha = match *pulse {
        StreamAppendPulse::Idle => STREAM_APPEND_REST_ALPHA,
        StreamAppendPulse::Fading { remaining } => {
            let progress = (remaining / STREAM_APPEND_FADE_SECS).clamp(0.0, 1.0);
            let next = remaining - time.delta_secs();
            *pulse = if next <= 0.0 {
                StreamAppendPulse::Idle
            } else {
                StreamAppendPulse::Fading { remaining: next }
            };
            STREAM_APPEND_REST_ALPHA
                + (STREAM_APPEND_FLASH_ALPHA - STREAM_APPEND_REST_ALPHA) * progress
        }
    };
    for mut indicator in &mut indicators {
        let target = indicator.0.with_alpha(alpha);
        if indicator.0 != target {
            indicator.0 = target;
        }
    }
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct LogsPagePlugin;

impl Plugin for LogsPagePlugin {
    fn build(&self, app: &mut App) {
        // BEVY-020: the ring owns the bounded window. `apply_logs_projection`
        // ingests into it and republishes `LastLogsProjection`; the row
        // reconciler ingests idempotently too, so observer order does not
        // matter.
        app.init_resource::<LastLogsProjection>()
            .init_resource::<LogsRing>()
            .init_resource::<LogLineWrap>()
            .init_resource::<StreamAppendPulse>()
            .add_observer(apply_logs_projection);
        app.add_plugins((
            LogsRowsPlugin,
            LogsSearchPlugin,
            LogsFollowPlugin,
            LogsExportPlugin,
        ));
        app.add_observer(on_logs_action_activated)
            .add_observer(toggle_soft_wrap)
            .add_systems(Update, (apply_line_wrap, advance_append_pulse));
    }
}

fn on_logs_action_activated(
    activate: On<Activate>,
    clear_buttons: Query<(), With<ClearLogsButton>>,
    handle: Option<Res<CommandSinkHandle>>,
    export: Res<LogsExportState>,
) {
    if export.model.is_open() {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    if clear_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ClearLogs);
    }
}

fn apply_logs_projection(
    update: On<LogsProjectionUpdated>,
    mut ring: ResMut<LogsRing>,
    mut last: ResMut<LastLogsProjection>,
    mut summaries: Query<&mut LocalizedText, With<LogsLine>>,
) {
    // BEVY-020: ingest into the bounded window and restamp from its snapshot.
    // The raw event may carry a whole stream; only the ring's tail is stored.
    ring.ingest(&update.0);
    let projection = ring.snapshot();
    // The summary line is the page's own count of the ring's window; it is
    // restamped in place so the toolbar never rebuilds.
    for mut summary in &mut summaries {
        *summary = LocalizedText::new(
            "logs_buffer_summary",
            vec![("count", projection.total_entries.to_string())],
        );
    }
    last.0 = Some(projection);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_logs_fixture() {
        let proj = LogsProjection::demo();
        assert_eq!(proj.total_entries, 5);
        assert_eq!(proj.entries.len(), 5);
        assert_eq!(proj.entries[0].timestamp, "10:14:02.124");
        assert_eq!(proj.entries[0].level, LogLevel::Info);
        assert_eq!(proj.entries[0].tag, "TCP");
        assert!(
            !proj.entries[0].message.starts_with('['),
            "the module tag is its own field; the message carries only the payload"
        );
        assert_eq!(proj.entries[2].level, LogLevel::Warn);
        assert_eq!(proj.entries[2].tag, "TUN");
    }
}
