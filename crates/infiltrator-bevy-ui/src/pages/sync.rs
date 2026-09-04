//! The Sync page (数据同步): WebDAV roaming, remote backups, 3-way merge,
//! conflict detection and resolution, snapshot history, and multi-device sync.
//!
//! **Update seam**: mutable nodes carry typed markers ([`SyncLine`],
//! [`SnapshotDeviceText`], [`SnapshotSizeText`], [`ConflictSummaryText`]).
//! The page self-registers [`apply_sync_projection`] and action observers
//! once per world via [`SyncPageRoot`]. When [`SyncProjectionUpdated`] fires,
//! texts, conflict panels, and snapshots restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
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

/// Marker for conflict summary text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConflictSummaryText;

/// Marker for conflict card container to toggle display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConflictCardContainer;

/// Marker for snapshot device/time text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDeviceText(pub usize);

/// Marker for snapshot size text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotSizeText(pub usize);

/// Marker for "Sync Now" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncNowButton;

/// Marker for "Create Backup" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreateBackupButton;

/// Marker for "Keep Local" conflict resolution button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeepLocalConflictButton;

/// Marker for "Take Remote" conflict resolution button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TakeRemoteConflictButton;

/// Marker for restoring a specific snapshot.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreSnapshotButton {
    pub snapshot_id: String,
    pub snapshot_idx: usize,
}

/// Status of the WebDAV sync connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncStatus {
    #[default]
    Connected,
    Disconnected,
    Syncing,
    Conflict,
    Error,
}

impl SyncStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "已连接 · 同步就绪",
            Self::Disconnected => "未连接",
            Self::Syncing => "正在同步数据中...",
            Self::Conflict => "同步冲突 · 需要手动解决",
            Self::Error => "同步异常",
        }
    }
}

/// Information about a single conflicting configuration key.
#[derive(Clone, Debug, PartialEq)]
pub struct ConflictingKey {
    pub key: String,
    pub local_value: String,
    pub remote_value: String,
}

/// Conflict details when WebDAV detects out-of-sync diverging vector clocks.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncConflictInfo {
    pub remote_device: String,
    pub conflict_time: String,
    pub conflicting_keys: Vec<ConflictingKey>,
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
    pub conflict: Option<SyncConflictInfo>,
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
            conflict: None,
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
        .enumerate()
        .map(|(idx, s)| Box::new(snapshot_row_scene(idx, s, palette)) as Box<dyn Scene>)
        .collect();

    let conflict_scene = conflict_panel_scene(projection.conflict.as_ref(), palette);

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
        PageRoot(Route::Sync)
        SyncPageRoot
        Children [
            ( { header_card_scene(summary, last_sync_str, palette) } ),
            ( { conflict_scene } ),
            ( { crate::pages::sync_merge::sync_three_way_merge_scene(palette) } ),
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
                            SyncNowButton
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
                            CreateBackupButton
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

fn conflict_panel_scene(
    conflict: Option<&SyncConflictInfo>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let has_conflict = conflict.is_some();
    let display_mode = if has_conflict {
        Display::Flex
    } else {
        Display::None
    };
    let conflict_text = match conflict {
        Some(info) => format!(
            "检测到冲突：远端设备 {} 于 {} 产生变更，共 {} 处不一致",
            info.remote_device,
            info.conflict_time,
            info.conflicting_keys.len()
        ),
        None => "无冲突".to_owned(),
    };

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
                display: { display_mode },
            }
            ConflictCardContainer
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        ( Text({ "⚠️ WebDAV 3-Way 同步冲突待解决".to_owned() }) TextRole(Role::BodyStrong) ),
                    ]
                ),
                ( Text(conflict_text) ConflictSummaryText TextRole(Role::Body) ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        (
                            Node {
                                min_height: px(palette.control_height_px * 0.85),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            KeepLocalConflictButton
                            Button
                            Children [
                                ( Text({ "保留本地配置 (Keep Local)".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px * 0.85),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            TakeRemoteConflictButton
                            Button
                            Children [
                                ( Text({ "采用远端配置 (Take Remote)".to_owned() }) TextRole(Role::Body) ),
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

fn snapshot_row_scene(
    idx: usize,
    snapshot: &SnapshotItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
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
                    ( Text(device_time) SnapshotDeviceText(idx) TextRole(Role::Body) ),
                    ( Text(size_str) SnapshotSizeText(idx) TextRole(Role::Mono) ),
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
                template_value(RestoreSnapshotButton {
                    snapshot_id: snapshot.id.clone(),
                    snapshot_idx: idx,
                } )
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
    commands.add_observer(on_sync_action_activated);
}

pub(crate) fn on_sync_action_activated(
    activate: On<Activate>,
    sync_now_buttons: Query<(), With<SyncNowButton>>,
    create_backup_buttons: Query<(), With<CreateBackupButton>>,
    keep_local_buttons: Query<(), With<KeepLocalConflictButton>>,
    take_remote_buttons: Query<(), With<TakeRemoteConflictButton>>,
    restore_buttons: Query<&RestoreSnapshotButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if sync_now_buttons.contains(activate.entity) {
        handle.submit(UiCommand::SyncNow);
    } else if create_backup_buttons.contains(activate.entity) {
        handle.submit(UiCommand::CreateBackupSnapshot);
    } else if keep_local_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ResolveConflictKeepLocal);
    } else if take_remote_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ResolveConflictTakeRemote);
    } else if let Ok(btn) = restore_buttons.get(activate.entity) {
        handle.submit(UiCommand::RestoreSnapshot {
            id: btn.snapshot_id.clone(),
        });
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_sync_projection(
    update: On<SyncProjectionUpdated>,
    mut last: Option<ResMut<LastSyncProjection>>,
    mut lines: Query<
        (&mut Text, &SyncLine),
        (
            With<SyncLine>,
            Without<ConflictSummaryText>,
            Without<SnapshotDeviceText>,
            Without<SnapshotSizeText>,
        ),
    >,
    mut conflict_texts: Query<
        (&mut Text, &ConflictSummaryText),
        (
            With<ConflictSummaryText>,
            Without<SyncLine>,
            Without<SnapshotDeviceText>,
            Without<SnapshotSizeText>,
        ),
    >,
    mut conflict_containers: Query<&mut Node, With<ConflictCardContainer>>,
    mut snapshot_devices: Query<
        (&mut Text, &SnapshotDeviceText),
        (
            With<SnapshotDeviceText>,
            Without<SyncLine>,
            Without<ConflictSummaryText>,
            Without<SnapshotSizeText>,
        ),
    >,
    mut snapshot_sizes: Query<
        (&mut Text, &SnapshotSizeText),
        (
            With<SnapshotSizeText>,
            Without<SyncLine>,
            Without<ConflictSummaryText>,
            Without<SnapshotDeviceText>,
        ),
    >,
    mut restore_buttons: Query<&mut RestoreSnapshotButton>,
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

    let has_conflict = projection.conflict.is_some();
    for mut container in &mut conflict_containers {
        container.display = if has_conflict {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Some(conflict) = &projection.conflict {
        for (mut text, _) in &mut conflict_texts {
            text.0 = format!(
                "检测到冲突：远端设备 {} 于 {} 产生变更，共 {} 处不一致",
                conflict.remote_device,
                conflict.conflict_time,
                conflict.conflicting_keys.len()
            );
        }
    }

    for (mut text, marker) in &mut snapshot_devices {
        if let Some(snap) = projection.snapshots.get(marker.0) {
            text.0 = format!("{} · {}", snap.device, snap.timestamp);
        }
    }

    for (mut text, marker) in &mut snapshot_sizes {
        if let Some(snap) = projection.snapshots.get(marker.0) {
            text.0 = format_byte_count(snap.size_bytes);
        }
    }

    for mut btn in &mut restore_buttons {
        if let Some(snap) = projection.snapshots.get(btn.snapshot_idx) {
            btn.snapshot_id = snap.id.clone();
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
        assert_eq!(proj.server_url, "https://dav.jianguoyun.com/dav/MusicFrog/");
        assert_eq!(proj.username, "user@example.com");
        assert_eq!(proj.snapshots.len(), 3);
        assert_eq!(proj.snapshots[0].device, "Linux Desktop (CachyOS)");
        assert_eq!(proj.snapshots[0].size_bytes, 142_800);
        assert!(proj.auto_sync);
        assert_eq!(proj.conflict, None);
    }
}
