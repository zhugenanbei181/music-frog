//! The Logs page (运行日志): live ring buffer logs, log level filtering,
//! tag classifications, and keyword searching.
//!
//! **Update seam**: mutable nodes carry typed markers ([`LogsLine`],
//! [`LogMessageText`], [`LogTimestampText`], [`LogLevelText`], [`LogTagText`]).
//! [`LogsPagePlugin`] registers [`apply_logs_projection`] and action observers once
//! at product assembly. When [`LogsProjectionUpdated`] fires, texts,
//! level colors, and filter buttons restamp in place without tree rebuilds.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::logs_export::{LogsExportPlugin, LogsExportState};
use crate::pages::logs_follow::{LogFollowLabel, LogsFollowPlugin};
use crate::pages::logs_ring::LogsRing;
use crate::pages::logs_rows::{LogRowIdentity, LogRowsContainer, LogsRowsPlugin};
use crate::pages::logs_search::{LogsSearchPlugin, logs_search_scene};
use crate::pages::logs_virtual::{LOGS_VIRTUAL_THRESHOLD, LogsVirtualNodes};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsInk};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_composition::demo_identities::{HK_PRIMARY, PROXIES, US};
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::surface_snapshot::PageStatus;

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

/// Marker for log entry message text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(TextRuns)]
pub struct LogMessageText(pub usize);

/// Marker for log entry timestamp text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(TextRuns)]
pub struct LogTimestampText(pub usize);

/// Marker for log entry level tag text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(TextRuns)]
pub struct LogLevelText(pub usize);

/// Marker for log entry category tag text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(TextRuns)]
pub struct LogTagText(pub usize);

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

/// Marker and target information for log level filter buttons.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogLevelFilterButton {
    pub level: Option<LogLevel>,
}

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
                    message: format!("[TCP] 127.0.0.1:54120 --> api.github.com:443 match DomainSuffix(github.com) using {PROXIES}[{HK_PRIMARY}]"),
                },
                LogEntry {
                    id: 2,
                    timestamp: "10:14:03.018".to_owned(),
                    level: LogLevel::Info,
                    tag: "DNS".to_owned(),
                    message: "[DNS] resolve manifest.googlevideo.com via https://1.1.1.1/dns-query -> 172.217.160.78 (32ms)".to_owned(),
                },
                LogEntry {
                    id: 3,
                    timestamp: "10:14:04.550".to_owned(),
                    level: LogLevel::Warn,
                    tag: "TUN".to_owned(),
                    message: "[TUN] high socket buffer pressure (85% capacity reached on utun9)".to_owned(),
                },
                LogEntry {
                    id: 4,
                    timestamp: "10:14:05.102".to_owned(),
                    level: LogLevel::Debug,
                    tag: "ROUTING".to_owned(),
                    message: "[ROUTING] process matched: Discord (pid: 11024) -> rule: DOMAIN-SUFFIX discord.gg".to_owned(),
                },
                LogEntry {
                    id: 5,
                    timestamp: "10:14:08.882".to_owned(),
                    level: LogLevel::Error,
                    tag: "PROXY".to_owned(),
                    message: format!("[PROXY] dial timeout on backup node {US} (after 5000ms)"),
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
                row_gap: Val::Px(space::S16),
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
                @{ logs_container_scene(log_scenes, virtualized, projection.active_level, palette) }
            ]
    }
}

pub fn header_card_scene(total_entries: usize, palette: &UiPalette) -> impl Scene + use<> {
    let header_a11y = accesskit::Node::new(accesskit::Role::Header);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(space::S16),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(space::S8),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("logs_toolbar_label")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::FileText, 36.0, palette) }
                            --
                            LocalizedText::new("logs_buffer_summary", vec![("count", total_entries.to_string())]) LogsLine(LogsLineKind::Summary) TextRole(Role::Heading)
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            PauseLogsButton
                            Button
                            Children [
                                LocalizedText::plain("logs_scroll_lock") LogFollowLabel TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            ExportLogsButton
                            Button
                            Children [
                                LocalizedText::plain("logs_export_action") TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            ClearLogsButton
                            Button
                            Children [
                                LocalizedText::plain("common_clear") TextRole(Role::Body)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

fn level_filter_pill(
    label: &str,
    level: Option<LogLevel>,
    active: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let bg = if active {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let label_str = label.to_owned();
    let label_scene: Box<dyn Scene> = if level.is_none() {
        Box::new(bsn! { LocalizedText::plain("logs_level_all") TextRole(Role::Caption) })
    } else {
        Box::new(bsn! { Text(label_str) TextRole(Role::Caption) })
    };

    let label_scenes = vec![label_scene];
    bsn! {
            Node {
                height: px(palette.control_height_px * 0.8),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ bg })
            ControlVisual({ active })
            LogLevelFilterButton { level: { level } }
            Button
            Children [
                { label_scenes }
            ]
    }
}

fn logs_container_scene(
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
                row_gap: Val::Px(space::S4),
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
                                row_gap: Val::Px(space::S8),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("logs_stream_title") TextRole(Role::BodyStrong)
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                    flex_wrap: FlexWrap::Wrap,
                                    row_gap: Val::Px(space::S4),
                                }
                                Children [
                                    @{ level_filter_pill("", None, active_level.is_none(), palette) }
                                    --
                                    @{ level_filter_pill("DEBUG", Some(LogLevel::Debug), active_level == Some(LogLevel::Debug), palette) }
                                    --
                                    @{ level_filter_pill("INFO", Some(LogLevel::Info), active_level == Some(LogLevel::Info), palette) }
                                    --
                                    @{ level_filter_pill("WARN", Some(LogLevel::Warn), active_level == Some(LogLevel::Warn), palette) }
                                    --
                                    @{ level_filter_pill("ERROR", Some(LogLevel::Error), active_level == Some(LogLevel::Error), palette) }
                                    --
                                    @{ level_filter_pill("UNKNOWN", Some(LogLevel::Unknown), active_level == Some(LogLevel::Unknown), palette) }
                                ]
                            ]
            }),
            rows_scene,
        ],
        palette,
    )
}

pub(super) fn log_row_scene(
    idx: usize,
    entry: &LogEntry,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let id = entry.id;
    let time = entry.timestamp.clone();
    let level_str = format!("[{}]", entry.level.label());
    let level_color = log_level_color(entry.level, palette);
    let tag_str = format!("[{}]", entry.tag);
    let msg = entry.message.clone();

    bsn! {
            Node {
                width: percent(100),
                min_height: px(palette.control_height_px * 0.8),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            LogRowIdentity(id)
            Children [
                Text(time) LogTimestampText(idx) TextRole(Role::Caption)
                --
                Text(level_str) LogLevelText(idx) TextRole(Role::BodyStrong) TextColor(level_color) TextRunsInk(level_color)
                --
                Text(tag_str) LogTagText(idx) TextRole(Role::Caption)
                --
                Text(msg) LogMessageText(idx) TextRole(Role::Mono)
            ]
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
            .add_observer(apply_logs_projection);
        app.add_plugins((
            LogsRowsPlugin,
            LogsSearchPlugin,
            LogsFollowPlugin,
            LogsExportPlugin,
        ));
        app.add_observer(on_logs_action_activated);
    }
}

fn on_logs_action_activated(
    activate: On<Activate>,
    clear_buttons: Query<(), With<ClearLogsButton>>,
    filter_buttons: Query<&LogLevelFilterButton>,
    handle: Option<Res<CommandSinkHandle>>,
    export: Res<LogsExportState>,
) {
    if export.model.open {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    if clear_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ClearLogs);
    } else if let Ok(btn) = filter_buttons.get(activate.entity) {
        handle.submit(UiCommand::SetLogLevelFilter {
            level: btn.level.map(|l| l.label().to_string()),
        });
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct LogCopy {
    text: &'static mut Text,
    color: Option<&'static mut TextColor>,
    summary: Option<&'static LogsLine>,
    localized: Option<&'static mut LocalizedText>,
    message: Option<&'static LogMessageText>,
    timestamp: Option<&'static LogTimestampText>,
    level: Option<&'static LogLevelText>,
    tag: Option<&'static LogTagText>,
}
fn apply_logs_projection(
    update: On<LogsProjectionUpdated>,
    palette: Res<UiPalette>,
    mut ring: ResMut<LogsRing>,
    mut last: ResMut<LastLogsProjection>,
    mut copies: Query<LogCopy>,
    mut filter_buttons: Query<(
        &mut BackgroundColor,
        &mut ControlVisual,
        &LogLevelFilterButton,
    )>,
) {
    // BEVY-020: ingest into the bounded window and restamp from its snapshot.
    // The raw event may carry a whole stream; only the ring's tail is stored.
    ring.ingest(&update.0);
    let projection = ring.snapshot();
    for mut copy in &mut copies {
        if copy.summary.is_some() {
            if let Some(ref mut localized) = copy.localized {
                **localized = LocalizedText::new(
                    "logs_buffer_summary",
                    vec![("count", projection.total_entries.to_string())],
                );
            }
        } else if let Some(marker) = copy.message {
            if let Some(entry) = projection.entries.get(marker.0) {
                copy.text.0 = entry.message.clone();
            }
        } else if let Some(marker) = copy.timestamp {
            if let Some(entry) = projection.entries.get(marker.0) {
                copy.text.0 = entry.timestamp.clone();
            }
        } else if let Some(marker) = copy.level {
            if let Some(entry) = projection.entries.get(marker.0) {
                copy.text.0 = format!("[{}]", entry.level.label());
                if let Some(ref mut color) = copy.color {
                    color.0 = log_level_color(entry.level, &palette);
                }
            }
        } else if let Some(marker) = copy.tag
            && let Some(entry) = projection.entries.get(marker.0)
        {
            copy.text.0 = format!("[{}]", entry.tag);
        }
    }
    for (mut bg, mut visual, btn) in &mut filter_buttons {
        let active = btn.level == projection.active_level;
        visual.0 = active;
        bg.0 = if active {
            palette.accent
        } else {
            palette.surface_elevated
        };
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
        assert_eq!(
            proj.entries[0].message,
            "[TCP] 127.0.0.1:54120 --> api.github.com:443 match DomainSuffix(github.com) using PROXIES[HK-01]"
        );
        assert_eq!(proj.entries[2].level, LogLevel::Warn);
        assert_eq!(proj.entries[2].tag, "TUN");
    }
}
