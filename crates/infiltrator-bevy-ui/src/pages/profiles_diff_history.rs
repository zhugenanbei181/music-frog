//! DUAL-09-06/07: the snapshot-history card of the Profiles page.
//!
//! Split out of `profiles_diff.rs` to keep both files inside the business line
//! budget. The card renders the shared `SnapshotHistorySnapshot` (entries, the
//! shared prune view and the retention control) and submits the shared
//! commands: manual backup, list refresh, prune and per-entry diff selection.
//! It never derives a retention rule of its own.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::Res;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::LastProfilesProjection;
use crate::pages::profiles_diff::{
    RollbackSnapshotLabel, SnapshotDiffViewState, diff_notice_scene,
};

/// DUAL-09-06: store a manual snapshot of the active profile now.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackupSnapshotButton;

/// DUAL-09-06: reload the shared history list.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshSnapshotHistoryButton;

/// DUAL-09-07: retention preset for the manual prune.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotPruneKeepButton {
    pub keep: usize,
}

/// DUAL-09-07: run the shared prune with the selected retention.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PruneSnapshotsButton;

/// DUAL-09-06: diff one history entry against the current content.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotHistoryEntryButton {
    pub id: String,
}

/// DUAL-09-14: restore one history entry behind the same two-step
/// confirmation the Iced history panel and the card rollback use.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotHistoryRestoreButton {
    pub id: String,
}

/// Container whose children are the history rows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotHistoryBody;

/// The shared history summary line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotHistorySummaryText;
pub(super) fn backup_button(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    Box::new(bsn! {
        Node {
            min_height: px(24.0),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ background })
        Button
        BackupSnapshotButton
        Children [
            ( Text({ "立即备份".to_owned() }) TextRole(Role::Caption) ),
        ]
    })
}

pub(super) fn history_refresh_button(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    Box::new(bsn! {
        Node {
            min_height: px(24.0),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ background })
        Button
        RefreshSnapshotHistoryButton
        Children [
            ( Text({ "刷新列表".to_owned() }) TextRole(Role::Caption) ),
        ]
    })
}

pub(super) fn prune_keep_row(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    let mut chips: Vec<Box<dyn Scene>> = Vec::new();
    for keep in [5usize, 10, 20, 50] {
        chips.push(Box::new(bsn! {
            Node {
                min_height: px(24.0),
                padding: UiRect::horizontal(Val::Px(space::S4)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
            }
            BackgroundColor({ background })
            Button
            template_value(SnapshotPruneKeepButton { keep })
            Children [
                ( Text({ format!("{keep}") }) TextRole(Role::Caption) ),
            ]
        }));
    }
    Box::new(bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S2),
        }
        Children [
            ( Text({ "保留".to_owned() }) TextRole(Role::Caption) ),
            { chips },
        ]
    })
}

pub(super) fn prune_button(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    Box::new(bsn! {
        Node {
            min_height: px(24.0),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ background })
        Button
        PruneSnapshotsButton
        Children [
            ( Text({ "立即修剪".to_owned() }) TextRole(Role::Caption) ),
        ]
    })
}

fn history_row_scene(
    entry: &infiltrator_contract::snapshot_history::SnapshotEntry,
    selected: bool,
    armed_restore: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let stamp = entry.stamp_label();
    let label = format!(
        "{stamp} · {} · {}{}",
        entry.short_hash(),
        if entry.is_newest { "最新 · " } else { "" },
        if entry.is_duplicate {
            "重复内容"
        } else {
            "对比此快照"
        }
    );
    let color = if selected {
        palette.accent
    } else {
        palette.ink_dim
    };
    let restore_label = if armed_restore {
        "再次点击确认回滚"
    } else {
        "恢复此快照"
    };
    let restore_color = if armed_restore {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let id = entry.id.clone();
    let restore_id = entry.id.clone();
    Box::new(bsn! {
        Node {
            width: percent(100),
            min_height: px(20.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S4),
        }
        Children [
            (
                Node {
                    width: percent(70),
                    min_height: px(20.0),
                    padding: UiRect::horizontal(Val::Px(space::S4)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ if selected { palette.surface_elevated } else { palette.window_clear } })
                Button
                template_value(SnapshotHistoryEntryButton { id })
                Children [
                    ( Text({ label }) TextRole(Role::Mono) TextColor({ color }) ),
                ]
            ),
            (
                Node {
                    min_height: px(20.0),
                    padding: UiRect::horizontal(Val::Px(space::S4)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ restore_color })
                Button
                template_value(SnapshotHistoryRestoreButton { id: restore_id })
                Children [
                    ( Text({ restore_label.to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    })
}

pub(super) fn history_rows_scene(
    history: Option<&SnapshotHistorySnapshot>,
    selected: Option<&str>,
    armed_restore: Option<&str>,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let rows: Vec<Box<dyn Scene>> = match history {
        Some(history) if !history.entries.is_empty() => history
            .entries
            .iter()
            .take(12)
            .map(|entry| {
                history_row_scene(
                    entry,
                    selected == Some(entry.id.as_str()),
                    armed_restore == Some(entry.id.as_str()),
                    palette,
                )
            })
            .collect(),
        Some(_) => vec![diff_notice_scene(
            "暂无历史快照；每次成功应用会自动备份一份",
        )],
        None => vec![diff_notice_scene(
            "尚未读取快照历史；点击「刷新列表」按需读取共享快照应用",
        )],
    };
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S2),
        }
        Children [
            { rows },
        ]
    })
}

/// DUAL-09-06: store a manual snapshot through the shared snapshot application.
pub(super) fn on_backup_snapshot_activated(
    activate: On<Activate>,
    buttons: Query<(), With<BackupSnapshotButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::CreateBackupSnapshot);
}

/// DUAL-09-06: reload the shared history list.
pub(super) fn on_refresh_snapshot_history(
    activate: On<Activate>,
    buttons: Query<(), With<RefreshSnapshotHistoryButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::LoadSnapshotHistory);
}

/// DUAL-09-07: pick the retention the manual prune keeps.
pub(super) fn on_snapshot_prune_keep_activated(
    activate: On<Activate>,
    buttons: Query<&SnapshotPruneKeepButton>,
    mut view: ResMut<SnapshotDiffViewState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    view.prune_keep = button.keep;
}

/// DUAL-09-07: run the shared dedupe+LRU prune with the selected retention.
pub(super) fn on_prune_snapshots_activated(
    activate: On<Activate>,
    buttons: Query<(), With<PruneSnapshotsButton>>,
    view: Res<SnapshotDiffViewState>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::PruneSnapshots {
        keep: Some(view.prune_keep),
    });
}

/// DUAL-09-06: diff (and therefore roll back to) one specific history entry.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_snapshot_history_entry_activated(
    activate: On<Activate>,
    buttons: Query<&SnapshotHistoryEntryButton>,
    mut view: ResMut<SnapshotDiffViewState>,
    mut labels: Query<&mut Text, With<RollbackSnapshotLabel>>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    history_bodies: Query<Entity, With<SnapshotHistoryBody>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    view.selected_snapshot = Some(button.id.clone());
    view.rollback_armed = false;
    view.armed_restore = None;
    for mut label in &mut labels {
        label.0 = "一键安全还原此快照".to_owned();
    }
    let history = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.snapshot_history.as_ref());
    for entity in &history_bodies {
        commands.entity(entity).despawn_children();
        let scene = history_rows_scene(
            history,
            view.selected_snapshot.as_deref(),
            view.armed_restore.as_deref(),
            &palette,
        );
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
    if let Some(handle) = handle.as_ref() {
        handle.submit(UiCommand::LoadSnapshotDiff {
            snapshot_id: Some(button.id.clone()),
        });
    }
}

/// DUAL-09-14: per-entry two-step restore — the same confirmed action the
/// Iced history panel offers. The first click only arms; the second submits
/// the shared `RestoreSnapshot` through the apply transaction.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_snapshot_history_restore_activated(
    activate: On<Activate>,
    buttons: Query<&SnapshotHistoryRestoreButton>,
    mut view: ResMut<SnapshotDiffViewState>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    history_bodies: Query<Entity, With<SnapshotHistoryBody>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let history = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.snapshot_history.as_ref());
    if view.armed_restore.as_deref() != Some(button.id.as_str()) {
        view.armed_restore = Some(button.id.clone());
        for entity in &history_bodies {
            commands.entity(entity).despawn_children();
            let scene = history_rows_scene(
                history,
                view.selected_snapshot.as_deref(),
                view.armed_restore.as_deref(),
                &palette,
            );
            commands.spawn_scene(scene).insert(ChildOf(entity));
        }
        return;
    }
    view.armed_restore = None;
    for entity in &history_bodies {
        commands.entity(entity).despawn_children();
        let scene = history_rows_scene(history, view.selected_snapshot.as_deref(), None, &palette);
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
    if let Some(handle) = handle.as_ref() {
        handle.submit(UiCommand::RestoreSnapshot {
            id: button.id.clone(),
        });
    }
}
