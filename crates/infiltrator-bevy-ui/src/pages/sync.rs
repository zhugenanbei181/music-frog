//! The Sync page (数据同步): WebDAV roaming, remote backups, 3-way merge,
//! snapshot history, and multi-device profile roaming.
//!
//! **Update seam**: mutable nodes carry typed markers ([`SyncLine`],
//! [`SnapshotRowMarker`]). The page self-registers
//! [`apply_sync_projection`] once per world via [`SyncPageRoot`].

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::pages::overview::format_byte_count;
use crate::route::{PageRoot, Route};

/// Root marker on the Sync page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_sync_page)]
pub struct SyncPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct SyncPageBound;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncLine(pub SyncLineKind);

/// Different text lines on the sync page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncLineKind {
    /// Overview summary: connection status.
    #[default]
    Summary,
    /// Last sync timestamp.
    LastSync,
    /// Server URL info.
    ServerUrl,
}

/// Status of the WebDAV sync connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncStatus {
    #[default]
    Connected,
    Disconnected,
    Syncing,
    Error,
}

impl SyncStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "已连接 · 同步就绪",
            Self::Disconnected => "未连接",
            Self::Syncing => "正在同步数据中...",
            Self::Error => "同步异常",
        }
    }
}

/// A remote backup snapshot item.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotItem {
    pub id: String,
    pub timestamp: String,
    pub device: String,
    pub size_bytes: u64,
}

/// Snapshot of the Sync domain.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncProjection {
    pub status: SyncStatus,
    pub server_url: String,
    pub username: String,
    pub last_sync: Option<String>,
    pub auto_sync: bool,
    pub snapshots: Vec<SnapshotItem>,
}

impl SyncProjection {
    /// Believable demo fixture for the Sync page.
    pub fn demo() -> Self {
        Self {
            status: SyncStatus::Connected,
            server_url: "https://dav.jianguoyun.com/dav/MusicFrog/".to_owned(),
            username: "user@example.com".to_owned(),
            last_sync: Some("2026-09-02 10:15".to_owned()),
            auto_sync: true,
            snapshots: vec![
                SnapshotItem {
                    id: "snap-1".to_owned(),
                    timestamp: "2026-09-02 10:15".to_owned(),
                    device: "Linux Desktop (CachyOS)".to_owned(),
                    size_bytes: 142_800,
                },
                SnapshotItem {
                    id: "snap-2".to_owned(),
                    timestamp: "2026-09-01 22:30".to_owned(),
                    device: "Android (Pixel 9 Pro)".to_owned(),
                    size_bytes: 138_400,
                },
                SnapshotItem {
                    id: "snap-3".to_owned(),
                    timestamp: "2026-08-30 09:12".to_owned(),
                    device: "macOS (MacBook Air)".to_owned(),
                    size_bytes: 125_600,
                },
            ],
        }
    }
}

/// The typed event dispatched when sync data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct SyncProjectionUpdated(pub SyncProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastSyncProjection(pub Option<SyncProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn sync_page(projection: &SyncProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!("数据同步 · {}", projection.status.label());
    let last_sync_str = projection
        .last_sync
        .as_deref()
        .map(|t| format!("最近同步: {t}"))
        .unwrap_or_else(|| "最近同步: 从未".to_owned());

    let snapshot_scenes: Vec<Box<dyn Scene>> = projection
        .snapshots
        .iter()
        .map(|s| Box::new(snapshot_row_scene(s, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Sync)
        SyncPageRoot
        Children [
            ( { header_card_scene(summary, last_sync_str, palette) } ),
            ( { webdav_config_card(projection, palette) } ),
            ( { snapshots_card_scene(snapshot_scenes, palette) } ),
        ]
    }
}

fn header_card_scene(
    summary: String,
    last_sync: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("数据同步概览");

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
                        ( { icon_tile_scene(IconId::Zap, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) SyncLine(SyncLineKind::Summary) TextRole(Role::Heading) ),
                                ( Text(last_sync) SyncLine(SyncLineKind::LastSync) TextRole(Role::Caption) ),
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
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            Button
                            Children [
                                ( Text({ "立即同步".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
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
                                ( Text({ "创建备份".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn webdav_config_card(projection: &SyncProjection, palette: &UiPalette) -> impl Scene + use<> {
    let server_str = format!("服务端地址: {}", projection.server_url);
    let user_str = format!("账号: {}", projection.username);

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "WebDAV 云端漫游配置".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text(server_str) SyncLine(SyncLineKind::ServerUrl) TextRole(Role::Body) ),
                            ( Text(user_str) TextRole(Role::Caption) ),
                        ]
                    ),
                    ( { checkbox_scene("配置变更时自动同步 (Auto Sync on Change)".to_owned(), projection.auto_sync, palette) } ),
                ]
            }),
        ],
        palette,
    )
}

fn snapshots_card_scene(
    snapshot_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "云端快照历史 (Cloud Snapshots)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "支持 3-Way 差异比对与回滚".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { snapshot_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn snapshot_row_scene(snapshot: &SnapshotItem, palette: &UiPalette) -> impl Scene + use<> {
    let device_time = format!("{} · {}", snapshot.device, snapshot.timestamp);
    let size_str = format_byte_count(snapshot.size_bytes);

    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text(device_time) TextRole(Role::Body) ),
                    ( Text(size_str) TextRole(Role::Mono) ),
                ]
            ),
            (
                Node {
                    min_height: px(palette.control_height_px * 0.8),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.surface })
                Button
                Children [
                    ( Text({ "还原此版本".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    }
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_sync_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<SyncPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(SyncPageBound);
    commands.add_observer(apply_sync_projection);
}

pub(crate) fn apply_sync_projection(
    update: On<SyncProjectionUpdated>,
    mut last: Option<ResMut<LastSyncProjection>>,
    mut lines: Query<(&mut Text, &SyncLine)>,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            SyncLineKind::Summary => {
                text.0 = format!("数据同步 · {}", projection.status.label());
            }
            SyncLineKind::LastSync => {
                text.0 = projection
                    .last_sync
                    .as_deref()
                    .map(|t| format!("最近同步: {t}"))
                    .unwrap_or_else(|| "最近同步: 从未".to_owned());
            }
            SyncLineKind::ServerUrl => {
                text.0 = format!("服务端地址: {}", projection.server_url);
            }
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
    fn demo_sync_fixture() {
        let proj = SyncProjection::demo();
        assert_eq!(proj.status, SyncStatus::Connected);
        assert_eq!(proj.snapshots.len(), 3);
        assert!(proj.auto_sync);
    }
}
