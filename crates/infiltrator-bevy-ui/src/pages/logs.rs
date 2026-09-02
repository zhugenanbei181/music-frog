//! The Logs page (运行日志): live ring buffer logs, log level filtering,
//! tag classifications, and keyword searching.
//!
//! **Update seam**: mutable nodes carry typed markers ([`LogsLine`],
//! [`LogEntryMarker`]). The page self-registers
//! [`apply_logs_projection`] once per world via [`LogsPageRoot`].

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::route::{PageRoot, Route};

/// Root marker on the Logs page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_logs_page)]
pub struct LogsPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct LogsPageBound;

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

/// Marker for log entry text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogMessageText(pub usize);

/// Severity log levels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// Color for log level tag.
pub fn log_level_color(level: LogLevel, palette: &UiPalette) -> Color {
    match level {
        LogLevel::Debug => palette.ink_dim,
        LogLevel::Info => palette.accent,
        LogLevel::Warn => palette.warning,
        LogLevel::Error => palette.danger,
    }
}

/// A single log entry item.
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub tag: String,
    pub message: String,
}

/// Snapshot of the Logs domain.
#[derive(Clone, Debug, PartialEq)]
pub struct LogsProjection {
    pub total_entries: usize,
    pub active_level: Option<LogLevel>,
    pub entries: Vec<LogEntry>,
}

impl LogsProjection {
    /// Believable demo fixture for the Logs page.
    pub fn demo() -> Self {
        Self {
            total_entries: 5,
            active_level: None,
            entries: vec![
                LogEntry {
                    timestamp: "10:14:02.124".to_owned(),
                    level: LogLevel::Info,
                    tag: "TCP".to_owned(),
                    message: "[TCP] 127.0.0.1:54120 --> api.github.com:443 match DomainSuffix(github.com) using 节点选择[🇭🇰 香港 01]".to_owned(),
                },
                LogEntry {
                    timestamp: "10:14:03.018".to_owned(),
                    level: LogLevel::Info,
                    tag: "DNS".to_owned(),
                    message: "[DNS] resolve manifest.googlevideo.com via https://1.1.1.1/dns-query -> 172.217.160.78 (32ms)".to_owned(),
                },
                LogEntry {
                    timestamp: "10:14:04.550".to_owned(),
                    level: LogLevel::Warn,
                    tag: "TUN".to_owned(),
                    message: "[TUN] high socket buffer pressure (85% capacity reached on utun9)".to_owned(),
                },
                LogEntry {
                    timestamp: "10:14:05.102".to_owned(),
                    level: LogLevel::Debug,
                    tag: "ROUTING".to_owned(),
                    message: "[ROUTING] process matched: Discord (pid: 11024) -> rule: DOMAIN-SUFFIX discord.gg".to_owned(),
                },
                LogEntry {
                    timestamp: "10:14:08.882".to_owned(),
                    level: LogLevel::Error,
                    tag: "PROXY".to_owned(),
                    message: "[PROXY] dial timeout on backup node 🇺🇸 美国硅谷 01 (after 5000ms)".to_owned(),
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
    let summary = format!(
        "运行日志 · 环形缓冲区共 {} 行日志",
        projection.total_entries
    );

    let log_scenes: Vec<Box<dyn Scene>> = projection
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| Box::new(log_row_scene(idx, entry, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Logs)
        LogsPageRoot
        Children [
            ( { toolbar_card_scene(summary, projection.active_level, palette) } ),
            ( { logs_container_scene(log_scenes, palette) } ),
        ]
    }
}

fn toolbar_card_scene(
    summary: String,
    active_level: Option<LogLevel>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("运行日志控制栏");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
            }
            template_value(AccessibilityNode(header_a11y))
            Children [
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( { icon_tile_scene(IconId::FileText, 36.0, palette) } ),
                        ( Text(summary) LogsLine(LogsLineKind::Summary) TextRole(Role::Heading) ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        ( { level_filter_pill("全部", active_level.is_none(), palette) } ),
                        ( { level_filter_pill("INFO", active_level == Some(LogLevel::Info), palette) } ),
                        ( { level_filter_pill("WARN", active_level == Some(LogLevel::Warn), palette) } ),
                        ( { level_filter_pill("ERROR", active_level == Some(LogLevel::Error), palette) } ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            Children [
                                ( Text({ "清空".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn level_filter_pill(label: &str, active: bool, palette: &UiPalette) -> impl Scene + use<> {
    let bg = if active {
        palette.accent
    } else {
        palette.surface_elevated
    };
    bsn! {
        Node {
            height: px(palette.control_height_px * 0.8),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ bg })
        ControlVisual(active)
        Button
        Children [
            ( Text({ label.to_owned() }) TextRole(Role::Caption) ),
        ]
    }
}

fn logs_container_scene(
    log_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S4),
            }
            Children [
                { log_scenes },
            ]
        })],
        palette,
    )
}

fn log_row_scene(idx: usize, entry: &LogEntry, palette: &UiPalette) -> impl Scene + use<> {
    let time = entry.timestamp.clone();
    let level_str = format!("[{}]", entry.level.label());
    let level_color = log_level_color(entry.level, palette);
    let tag_str = format!("[{}]", entry.tag);
    let msg = entry.message.clone();

    bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px * 0.8),
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            ( Text(time) TextRole(Role::Caption) ),
            ( Text(level_str) TextRole(Role::BodyStrong) TextColor(level_color) ),
            ( Text(tag_str) TextRole(Role::Caption) ),
            ( Text(msg) LogMessageText(idx) TextRole(Role::Mono) ),
        ]
    }
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_logs_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<LogsPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(LogsPageBound);
    commands.add_observer(apply_logs_projection);
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_logs_projection(
    update: On<LogsProjectionUpdated>,
    mut last: Option<ResMut<LastLogsProjection>>,
    mut lines: Query<(&mut Text, &LogsLine), (With<LogsLine>, Without<LogMessageText>)>,
    mut messages: Query<(&mut Text, &LogMessageText), (With<LogMessageText>, Without<LogsLine>)>,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            LogsLineKind::Summary => {
                text.0 = format!(
                    "运行日志 · 环形缓冲区共 {} 行日志",
                    projection.total_entries
                );
            }
        }
    }

    for (mut text, marker) in &mut messages {
        if let Some(entry) = projection.entries.get(marker.0) {
            text.0 = entry.message.clone();
        }
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_logs_fixture() {
        let proj = LogsProjection::demo();
        assert_eq!(proj.total_entries, 5);
        assert_eq!(proj.entries.len(), 5);
        assert_eq!(proj.entries[0].level, LogLevel::Info);
    }
}
