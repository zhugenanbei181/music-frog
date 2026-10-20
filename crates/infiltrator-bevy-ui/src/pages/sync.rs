//! The Sync page (数据同步): WebDAV roaming, remote backups, 3-way merge,
//! conflict detection and resolution, snapshot history, and multi-device sync.
//!
//! **Update seam**: mutable nodes carry typed markers ([`SyncLine`],
//! [`SnapshotDeviceText`], [`SnapshotSizeText`], [`ConflictSummaryText`]).
//! [`SyncPagePlugin`] registers [`apply_sync_projection`] and action observers
//! once at product assembly. When [`SyncProjectionUpdated`] fires,
//! texts, conflict panels, and snapshots restamp in place without tree rebuilds.

#[path = "sync_query_access.rs"]
pub mod query_access;
use self::query_access::SyncProjectionTargets;
use infiltrator_shared::locales::Lang;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use crate::pages::page_models::SyncPageModelState;
use crate::pages::snapshot_restore::OpenSnapshotRestore;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{
    SnapshotItemSnapshot, SyncConflictSnapshot, SyncPageSnapshot,
};
use infiltrator_presentation::sync_page_model::{ConflictResolution, SyncPageModel};
use infiltrator_presentation::sync_projection;
use infiltrator_shared::byte_units::format_bytes;
use lurker_core_contract::page::PageStatus;

use crate::pages::sync_merge::{
    SyncMergeRowsCache, replay_availability, replay_fields, sync_three_way_merge_scene,
};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, Node, Overflow, UiRect,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Root marker on the Sync page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SyncPageRoot;

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
    Username,
    HistoryStatus,
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
    pub profile: String,
    pub snapshot_id: String,
    pub snapshot_idx: usize,
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
    pub profile: String,
    pub id: String,
    pub timestamp: String,
    pub device: String,
    pub size_bytes: Option<u64>,
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
    pub history_status: PageStatus,
}

impl SyncProjection {
    /// The owner's page snapshot this projection renders.
    ///
    /// The page resolves the conflict it shows, so the projection carries the
    /// owner's own snapshot back to the shared model: the rendered conflict
    /// and the submitted intent can never disagree.
    pub fn owner_page(&self) -> SyncPageSnapshot {
        SyncPageSnapshot {
            status: self.status,
            history_status: self.history_status.clone(),
            server_url: self.server_url.clone(),
            username: self.username.clone(),
            last_sync: self.last_sync.clone(),
            auto_sync: self.auto_sync,
            conflict: self.conflict.as_ref().map(|conflict| SyncConflictSnapshot {
                remote_device: conflict.remote_device.clone(),
                conflict_time: conflict.conflict_time.clone(),
                conflicting_keys: conflict
                    .conflicting_keys
                    .iter()
                    .map(|key| {
                        (
                            key.key.clone(),
                            key.local_value.clone(),
                            key.remote_value.clone(),
                        )
                    })
                    .collect(),
            }),
            snapshots: self
                .snapshots
                .iter()
                .map(|item| SnapshotItemSnapshot {
                    profile: item.profile.clone(),
                    id: item.id.clone(),
                    timestamp: item.timestamp.clone(),
                    device: item.device.clone(),
                    size_bytes: item.size_bytes,
                })
                .collect(),
        }
    }

    /// Believable demo fixture for the Sync page.
    #[cfg(any(feature = "capture-tools", test))]
    pub fn demo() -> Self {
        Self {
            status: SyncStatus::Connected,
            history_status: PageStatus::Ready,
            server_url: "https://dav.jianguoyun.com/dav/MusicFrog/".to_owned(),
            username: "user@example.com".to_owned(),
            last_sync: Some("2026-09-02 10:15".to_owned()),
            auto_sync: true,
            conflict: None,
            snapshots: vec![
                SnapshotItem {
                    profile: "main".into(),
                    id: "snap-1".to_owned(),
                    timestamp: "2026-09-02 10:15".to_owned(),
                    device: "Linux Desktop (CachyOS)".to_owned(),
                    size_bytes: Some(142_800),
                },
                SnapshotItem {
                    profile: "main".into(),
                    id: "snap-2".to_owned(),
                    timestamp: "2026-09-01 22:30".to_owned(),
                    device: "Android (Pixel 9 Pro)".to_owned(),
                    size_bytes: Some(138_400),
                },
                SnapshotItem {
                    profile: "main".into(),
                    id: "snap-3".to_owned(),
                    timestamp: "2026-08-30 09:12".to_owned(),
                    device: "macOS (MacBook Air)".to_owned(),
                    size_bytes: Some(125_600),
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
    let locale = UiLocale::default();
    let summary = sync_projection::summary(projection.status, &Lang(locale.code()));
    let last_sync_str =
        sync_projection::last_sync(projection.last_sync.as_deref(), &Lang(locale.code()));

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
                row_gap: rem(space::S16),
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::Sync)
            SyncPageRoot
            ScrollArea
            Children [
                @{ header_card_scene(summary, last_sync_str, palette) }
                --
                @{ conflict_scene } ConflictCardContainer
                --
                @{ sync_three_way_merge_scene(projection.conflict.as_ref(), palette) }
                --
                @{ webdav_config_card(projection, palette) }
                --
                @{ snapshots_card_scene(snapshot_scenes, &projection.history_status, palette) }
            ]
    }
}

fn header_card_scene(
    summary: String,
    last_sync: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("sync_observation_header");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S16),
                    }
                    AccessibilityNode(header_a11y) label
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Zap, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S4),
                            }
                            Children [
                                Text(summary) SyncLine(SyncLineKind::Summary) TextRole(Role::Heading)
                                --
                                Text(last_sync) SyncLine(SyncLineKind::LastSync) TextRole(Role::Caption)
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::Accent })
                            SyncNowButton
                            Button
                            Children [
                                LocalizedText::plain("sync_now") TextRole(Role::BodyStrong)
                            ]
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            CreateBackupButton
                            Button
                            Children [
                                LocalizedText::plain("common_create_backup") TextRole(Role::Body)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

fn conflict_panel_scene(
    conflict: Option<&SyncConflictInfo>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let locale = UiLocale::default();
    let conflict_text = conflict
        .map(|info| {
            sync_projection::conflict(
                &info.remote_device,
                &info.conflict_time,
                info.conflicting_keys.len(),
                &Lang(locale.code()),
            )
        })
        .unwrap_or_else(|| locale.text("sync_observation_conflict_none"));

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S8),
                    }
                    Children [
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            LocalizedText::plain("sync_conflict_pending") TextRole(Role::BodyStrong)
                        ]
                        --
                        Text(conflict_text) ConflictSummaryText TextRole(Role::Body)
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            Node {
                                min_height: px(palette.control_height_px * 0.85),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::Accent })
                            KeepLocalConflictButton
                            Button
                            Children [
                                LocalizedText::plain("sync_keep_local_action") TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px * 0.85),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            TakeRemoteConflictButton
                            Button
                            Children [
                                LocalizedText::plain("sync_take_remote_action") TextRole(Role::Body)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

fn webdav_config_card(projection: &SyncProjection, palette: &UiPalette) -> impl Scene + use<> {
    let server_str =
        sync_projection::server(&projection.server_url, &Lang(UiLocale::default().code()));
    let user_str =
        sync_projection::username(&projection.username, &Lang(UiLocale::default().code()));

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("sync_webdav_settings_title") TextRole(Role::BodyStrong)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S8),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(rem(space::S8)),
                                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                }
                                ThemeBackgroundColor({ Token::SurfaceElevated })
                                Children [
                                    Text(server_str) SyncLine(SyncLineKind::ServerUrl) TextRole(Role::Body)
                                    --
                                    Text(user_str) SyncLine(SyncLineKind::Username) TextRole(Role::Caption)
                                ]
                                --
                                @{ localized_checkbox_scene(LocalizedText::plain("sync_autostart_sync"), projection.auto_sync, palette) }
                            ]
            }),
        ],
        palette,
    )
}

fn snapshots_card_scene(
    snapshot_scenes: Vec<Box<dyn Scene>>,
    history_status: &PageStatus,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let history_copy =
        sync_projection::history_notice(history_status, &Lang(UiLocale::default().code()));
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("sync_observation_history_title") TextRole(Role::BodyStrong)
                                --
                                Text(history_copy) SyncLine(SyncLineKind::HistoryStatus) TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S8),
                            }
                            Children [
                                { snapshot_scenes }
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
    let size_str = snapshot
        .size_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".into());

    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(rem(space::S8)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                }
                Children [
                    Text(device_time) SnapshotDeviceText(idx) TextRole(Role::Body)
                    --
                    Text(size_str) SnapshotSizeText(idx) TextRole(Role::Mono)
                ]
                --
                Node {
                    min_height: px(palette.control_height_px * 0.8),
                    padding: UiRect::horizontal(rem(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                }
                ThemeBackgroundColor({ Token::Surface })
                RestoreSnapshotButton {
                    profile: { snapshot.profile.clone() },                    snapshot_id: { snapshot.id.clone() },
                    snapshot_idx: idx,
                }
                Button
                Children [
                    LocalizedText::plain("sync_restore_version_action") TextRole(Role::Caption)
                ]
            ]
    }
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct SyncPagePlugin;

impl Plugin for SyncPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastSyncProjection>();
        app.add_observer(apply_sync_projection);
        app.init_resource::<SyncMergeRowsCache>();
        app.add_systems(Update, (replay_sync, replay_fields, replay_availability));
        app.add_observer(on_sync_action_activated);
        app.add_observer(on_restore_snapshot);
    }
}

pub(crate) fn on_sync_action_activated(
    activate: On<Activate>,
    sync_now_buttons: Query<(), With<SyncNowButton>>,
    create_backup_buttons: Query<(), With<CreateBackupButton>>,
    keep_local_buttons: Query<(), With<KeepLocalConflictButton>>,
    take_remote_buttons: Query<(), With<TakeRemoteConflictButton>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut model: Option<ResMut<SyncPageModelState>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Some(model) = model.as_deref_mut() else {
        return;
    };
    if sync_now_buttons.contains(activate.entity) {
        model.0.sync_now_phase_mut().begin();
        handle.submit(UiCommand::SyncNow);
    } else if create_backup_buttons.contains(activate.entity) {
        handle.submit(UiCommand::CreateBackupSnapshot);
    } else if keep_local_buttons.contains(activate.entity) {
        model.0.select_resolution(ConflictResolution::KeepLocal);
        submit_conflict_resolution(&handle, &mut model.0);
    } else if take_remote_buttons.contains(activate.entity) {
        model.0.select_resolution(ConflictResolution::TakeRemote);
        submit_conflict_resolution(&handle, &mut model.0);
    }
}

/// Submit the typed conflict-resolution intent the shared model owns.
///
/// The resolution is a typed value in the shared model; the surface only
/// carries it to the command bus.
fn submit_conflict_resolution(handle: &CommandSinkHandle, model: &mut SyncPageModel) {
    match model.resolve_conflict_intent() {
        Some(CommandIntent::ResolveConflictKeepLocal) => {
            model.resolve_phase_mut().begin();
            handle.submit(UiCommand::ResolveConflictKeepLocal);
        }
        Some(CommandIntent::ResolveConflictTakeRemote) => {
            model.resolve_phase_mut().begin();
            handle.submit(UiCommand::ResolveConflictTakeRemote);
        }
        Some(_) | None => {}
    }
}

pub(crate) fn apply_sync_projection(
    update: On<SyncProjectionUpdated>,
    mut last: ResMut<LastSyncProjection>,
    mut model: Option<ResMut<SyncPageModelState>>,
) {
    last.0 = Some(update.0.clone());
    let Some(model) = model.as_deref_mut() else {
        return;
    };
    // The page acts on the conflict it renders: the shared model observes the
    // same projection the page shows, so the rendered conflict and the
    // submitted intent can never disagree.
    model.0.observe_page(update.0.owner_page());
    settle_submitted_phases(&mut model.0);
}

/// End a submitted phase the owner's next publication answered.
///
/// The Bevy command bus is fire-and-forget: the owner's next publication is
/// the only answer this surface can observe, so it ends the submitted phase
/// instead of leaving it running forever.
fn settle_submitted_phases(model: &mut SyncPageModel) {
    if model.sync_now_phase().is_running()
        && let Some(token) = model.sync_now_phase().token()
    {
        model.sync_now_phase_mut().succeed(token);
    }
    if model.resolve_phase().is_running()
        && let Some(token) = model.resolve_phase().token()
    {
        model.resolve_phase_mut().succeed(token);
    }
}

pub(crate) fn replay_sync(
    last: Res<LastSyncProjection>,
    locale: Res<UiLocale>,
    targets: SyncProjectionTargets,
) {
    let SyncProjectionTargets {
        mut lines,
        mut conflict_texts,
        mut conflict_containers,
        mut snapshot_devices,
        mut snapshot_sizes,
        mut restore_buttons,
    } = targets;

    let Some(projection) = last.0.as_ref() else {
        return;
    };
    for (mut text, line) in &mut lines {
        text.0 = match line.0 {
            SyncLineKind::Summary => {
                sync_projection::summary(projection.status, &Lang(locale.code()))
            }
            SyncLineKind::LastSync => {
                sync_projection::last_sync(projection.last_sync.as_deref(), &Lang(locale.code()))
            }
            SyncLineKind::ServerUrl => {
                sync_projection::server(&projection.server_url, &Lang(locale.code()))
            }
            SyncLineKind::Username => {
                sync_projection::username(&projection.username, &Lang(locale.code()))
            }
            SyncLineKind::HistoryStatus => {
                sync_projection::history_notice(&projection.history_status, &Lang(locale.code()))
            }
        };
    }
    let has_conflict = projection.conflict.is_some();
    for mut container in &mut conflict_containers {
        container.display = if has_conflict {
            Display::Flex
        } else {
            Display::None
        };
    }
    let message = projection
        .conflict
        .as_ref()
        .map(|conflict| {
            sync_projection::conflict(
                &conflict.remote_device,
                &conflict.conflict_time,
                conflict.conflicting_keys.len(),
                &Lang(locale.code()),
            )
        })
        .unwrap_or_else(|| locale.text("sync_observation_conflict_none"));
    for (mut text, _) in &mut conflict_texts {
        text.0 = message.clone();
    }
    for (mut text, marker) in &mut snapshot_devices {
        if let Some(snap) = projection.snapshots.get(marker.0) {
            text.0 = format!("{} · {}", snap.device, snap.timestamp);
        }
    }
    for (mut text, marker) in &mut snapshot_sizes {
        if let Some(snap) = projection.snapshots.get(marker.0) {
            text.0 = snap
                .size_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "—".into());
        }
    }
    for mut btn in &mut restore_buttons {
        if let Some(snap) = projection.snapshots.get(btn.snapshot_idx) {
            btn.snapshot_id = snap.id.clone();
            btn.profile = snap.profile.clone();
        }
    }
}

fn on_restore_snapshot(
    event: On<Activate>,
    buttons: Query<&RestoreSnapshotButton>,
    mut commands: Commands,
) {
    if let Ok(button) = buttons.get(event.entity) {
        commands.trigger(OpenSnapshotRestore(SnapshotRestoreTarget {
            profile: button.profile.clone(),
            snapshot_id: button.snapshot_id.clone(),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sync_page_resolves_the_conflict_it_renders() {
        let mut model = SyncPageModel::default();
        let page = SyncPageSnapshot {
            status: SyncStatus::Conflict,
            server_url: "https://dav.example/".to_owned(),
            username: "user@example.com".to_owned(),
            last_sync: None,
            auto_sync: true,
            conflict: Some(SyncConflictSnapshot {
                remote_device: "Android (Pixel 9 Pro)".to_owned(),
                conflict_time: "2026-09-02 10:14".to_owned(),
                conflicting_keys: vec![("mode".to_owned(), "Rule".to_owned(), "Global".to_owned())],
            }),
            snapshots: Vec::new(),
            history_status: PageStatus::Ready,
        };
        model.observe_page(page);

        // The typed resolution and the typed intent come from the shared
        // model, so the page can never submit a resolution it does not show.
        model.select_resolution(ConflictResolution::KeepLocal);
        assert_eq!(
            model.resolve_conflict_intent(),
            Some(CommandIntent::ResolveConflictKeepLocal)
        );
        model.select_resolution(ConflictResolution::TakeRemote);
        assert_eq!(
            model.resolve_conflict_intent(),
            Some(CommandIntent::ResolveConflictTakeRemote)
        );
    }

    #[test]
    fn the_owner_publication_ends_a_submitted_phase() {
        let mut model = SyncPageModel::default();
        model.sync_now_phase_mut().begin();
        assert!(model.sync_now_phase().is_running());
        settle_submitted_phases(&mut model);
        assert!(
            !model.sync_now_phase().is_running(),
            "the owner's next publication ends the submitted phase"
        );
    }

    #[test]
    fn demo_sync_fixture() {
        let proj = SyncProjection::demo();
        assert_eq!(proj.status, SyncStatus::Connected);
        assert_eq!(proj.server_url, "https://dav.jianguoyun.com/dav/MusicFrog/");
        assert_eq!(proj.username, "user@example.com");
        assert_eq!(proj.snapshots.len(), 3);
        assert_eq!(proj.snapshots[0].device, "Linux Desktop (CachyOS)");
        assert_eq!(proj.snapshots[0].size_bytes, Some(142_800));
        assert!(proj.auto_sync);
        assert_eq!(proj.conflict, None);
    }
}
