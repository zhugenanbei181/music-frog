//! Configuration Snapshot Visual Diff & Rollback scene component (快照比对与秒级回滚).
//!
//! DUAL-09-08/09/12: the card renders the shared `YamlAstDiffSnapshot` the
//! snapshot application computed from a real snapshot file and the current
//! profile content. It never fabricates `+/-/~` rows; when no diff has been
//! computed yet it says so and offers a refresh action. The rollback button
//! routes through the shared apply transaction behind a two-step confirmation.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::snapshot_history::{SNAPSHOT_DEFAULT_KEEP, SnapshotHistorySnapshot};

use super::profiles_diff_history::{SnapshotHistoryBody, SnapshotHistorySummaryText};
use infiltrator_contract::yaml_ast_diff::{DiffKind, DiffLine, YamlAstDiffSnapshot};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{LastProfilesProjection, ProfileProtectionText, ProfilesProjection};

/// Marker for snapshot diff root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDiffRoot;

/// Marker for snapshot rollback button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RollbackSnapshotButton;

/// DUAL-09-08: ask the shared application to recompute the diff.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshSnapshotDiffButton;

/// DUAL-09-08: switch the shared diff between inline and split layout.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDiffModeButton {
    pub split: bool,
}

/// The card's real `+a -d ~m` summary line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDiffSummaryText;

/// The rollback button's label; restamped when the confirmation arms.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RollbackSnapshotLabel;

/// Container whose children are rebuilt from the shared diff rows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDiffBody;

/// DUAL-09-08: which layout the card renders. Both come from the same shared
/// snapshot; this is a presentation-only choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnapshotDiffViewMode {
    #[default]
    Inline,
    Split,
}

/// Surface-local view state plus the two-step rollback confirmation.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SnapshotDiffViewState {
    pub mode: SnapshotDiffViewMode,
    pub rollback_armed: bool,
    /// DUAL-09-06: history entry the user selected to diff (and roll back to).
    pub selected_snapshot: Option<String>,
    /// DUAL-09-07: retention requested by the manual prune control.
    pub prune_keep: usize,
}

impl Default for SnapshotDiffViewState {
    fn default() -> Self {
        Self {
            mode: SnapshotDiffViewMode::default(),
            rollback_armed: false,
            selected_snapshot: None,
            prune_keep: SNAPSHOT_DEFAULT_KEEP,
        }
    }
}

/// Snapshot Diff & Rollback scene. Both the mode chips and the actions are
/// mounted even before a diff exists; the row body is filled by
/// [`sync_snapshot_diff`] from the shared projection.
pub fn snapshot_diff_scene(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let summary = diff_summary(projection.yaml_ast_diff.as_ref());
    let diff_available = projection
        .yaml_ast_diff
        .as_ref()
        .is_some_and(|diff| diff.has_differences());
    let initial_rows = diff_rows_scene(
        projection.yaml_ast_diff.as_ref(),
        SnapshotDiffViewMode::Inline,
        palette,
    );

    let history = projection.snapshot_history.clone();
    let history_summary = snapshot_history_summary(history.as_ref(), SNAPSHOT_DEFAULT_KEEP);
    let initial_history_rows =
        super::profiles_diff_history::history_rows_scene(history.as_ref(), None, palette);

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                SnapshotDiffRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S4),
                                }
                                Children [
                                    ( Text({ "配置历史快照比对 (Snapshot Visual Diff)".to_owned() }) TextRole(Role::BodyStrong) ),
                                    ( Text(summary) SnapshotDiffSummaryText TextRole(Role::Caption) ),
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
                            ( { mode_button("行内", false, palette) } ),
                            ( { mode_button("并排", true, palette) } ),
                            ( { refresh_button(palette) } ),
                            ( { rollback_button(diff_available, palette) } ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::all(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.window_clear })
                Children [
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text({ history_summary }) SnapshotHistorySummaryText TextRole(Role::Caption) ),
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    ( { super::profiles_diff_history::backup_button(palette) } ),
                                    ( { super::profiles_diff_history::history_refresh_button(palette) } ),
                                    ( { super::profiles_diff_history::prune_keep_row(palette) } ),
                                    ( { super::profiles_diff_history::prune_button(palette) } ),
                                ]
                            ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S2),
                        }
                        SnapshotHistoryBody
                        Children [
                            ( { initial_history_rows } ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S4),
                            padding: UiRect::top(Val::Px(space::S4)),
                        }
                        Children [
                            (
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S2),
                                }
                                SnapshotDiffBody
                                Children [
                                    ( { initial_rows } ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// DUAL-09-06/07: the shared history summary. Never claims retention facts
/// that have not been loaded from the shared application.
pub fn snapshot_history_summary(history: Option<&SnapshotHistorySnapshot>, keep: usize) -> String {
    match history {
        Some(history) => {
            let mut summary = history.summary_zh();
            if history.duplicate_entries > 0 {
                summary.push_str(&format!(" · 重复内容 {} 份", history.duplicate_entries));
            }
            if let Some(report) = history.last_prune {
                summary.push_str(&format!(
                    " · 上次{}：删除 {} 份",
                    report.source.label_zh(),
                    report.removed
                ));
            }
            summary
        }
        None => format!("尚未读取快照历史（上限 {keep} 份）：点击「刷新列表」从共享快照应用读取"),
    }
}

fn mode_button(label: &str, split: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let background = if split {
        palette.surface_elevated
    } else {
        palette.accent
    };
    let label = label.to_owned();
    Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ background })
        Button
        template_value(SnapshotDiffModeButton { split })
        Children [
            ( Text({ label }) TextRole(Role::Caption) ),
        ]
    })
}

fn refresh_button(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Button
        RefreshSnapshotDiffButton
        Children [
            ( Text({ "刷新差异".to_owned() }) TextRole(Role::Body) ),
        ]
    })
}

fn rollback_button(available: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let background = if available {
        palette.danger
    } else {
        palette.surface_elevated
    };
    Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ background })
        Button
        RollbackSnapshotButton
        Children [
            ( Text({ "一键安全还原此快照".to_owned() }) RollbackSnapshotLabel TextRole(Role::BodyStrong) ),
        ]
    })
}

/// Honest summary: never claims changes exist before a diff was computed.
pub fn diff_summary(diff: Option<&YamlAstDiffSnapshot>) -> String {
    match diff {
        Some(diff) if diff.has_differences() => format!(
            "对比 {} → {} · {} · 保真 {}",
            diff.source_id,
            diff.target_id,
            diff.change_summary(),
            diff.fidelity_grade.as_str(),
        ),
        Some(_) => "快照与当前配置内容一致 (无差异)".to_owned(),
        None => "尚未计算快照差异：点击「刷新差异」从共享快照应用读取".to_owned(),
    }
}

/// DUAL-09-09: the snapshot path the rollback button would restore, if any.
pub fn rollback_target(projection: &ProfilesProjection) -> Option<String> {
    projection
        .yaml_ast_diff
        .as_ref()
        .and_then(|diff| diff.source_path.clone())
}

fn diff_color(kind: DiffKind, palette: &UiPalette) -> Color {
    match kind {
        DiffKind::Insert => palette.success,
        DiffKind::Delete => palette.danger,
        DiffKind::Modify => palette.warning,
        DiffKind::Equal => palette.ink_dim,
    }
}

fn line_label(line: &DiffLine) -> String {
    match (line.old_line, line.new_line) {
        (Some(old), Some(new)) => format!("{old:>4}|{new:<4}"),
        (Some(old), None) => format!("{old:>4}|    "),
        (None, Some(new)) => format!("    |{new:<4}"),
        (None, None) => "        ".to_owned(),
    }
}

fn diff_line_scene(line: &DiffLine, palette: &UiPalette) -> Box<dyn Scene> {
    let label = format!(
        "{} {} {}",
        line_label(line),
        line.kind.symbol(),
        line.content
    );
    let color = diff_color(line.kind, palette);
    Box::new(bsn! {
        Node {
            width: percent(100),
        }
        Children [
            ( Text({ label }) TextRole(Role::Mono) TextColor({ color }) ),
        ]
    })
}

pub(super) fn diff_notice_scene(text: &str) -> Box<dyn Scene> {
    let label = text.to_owned();
    Box::new(bsn! {
        Node {
            width: percent(100),
        }
        Children [
            ( Text({ label }) TextRole(Role::Caption) ),
        ]
    })
}

fn diff_rows_scene(
    diff: Option<&YamlAstDiffSnapshot>,
    mode: SnapshotDiffViewMode,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let rows: Vec<Box<dyn Scene>> = match diff {
        Some(diff) if diff.has_differences() => match mode {
            SnapshotDiffViewMode::Inline => diff
                .unified_lines
                .iter()
                .map(|line| diff_line_scene(line, palette))
                .collect(),
            SnapshotDiffViewMode::Split => diff
                .split_rows
                .iter()
                .flat_map(|row| {
                    row.left
                        .iter()
                        .chain(row.right.iter())
                        .map(|line| diff_line_scene(line, palette))
                        .collect::<Vec<_>>()
                })
                .collect(),
        },
        Some(_) => vec![diff_notice_scene("快照与当前配置一致，没有可显示的差异")],
        None => vec![diff_notice_scene(
            "尚未计算差异；点击「刷新差异」按需读取共享快照应用",
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

/// Rebuild the diff body in place from the shared projection and view state.
fn rebuild_body(
    commands: &mut Commands,
    entity: Entity,
    diff: Option<&YamlAstDiffSnapshot>,
    mode: SnapshotDiffViewMode,
    palette: &UiPalette,
) {
    commands.entity(entity).despawn_children();
    commands
        .spawn_scene(diff_rows_scene(diff, mode, palette))
        .insert(ChildOf(entity));
}

/// Restamp the card's summary, protection chips, mode chips and diff rows from
/// the shared projection. The card keeps no second copy of the diff.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn sync_snapshot_diff(
    update: On<crate::pages::profiles::ProfilesProjectionUpdated>,
    palette: Res<UiPalette>,
    view: Res<SnapshotDiffViewState>,
    mut commands: Commands,
    mut summary: Query<
        &mut Text,
        (
            With<SnapshotDiffSummaryText>,
            Without<ProfileProtectionText>,
            Without<SnapshotHistorySummaryText>,
        ),
    >,
    mut protection_texts: Query<
        (&mut Text, &ProfileProtectionText),
        (
            Without<SnapshotDiffSummaryText>,
            Without<SnapshotHistorySummaryText>,
        ),
    >,
    mut mode_buttons: Query<(&SnapshotDiffModeButton, &mut BackgroundColor)>,
    bodies: Query<Entity, With<SnapshotDiffBody>>,
    mut history_summaries: Query<
        &mut Text,
        (
            With<SnapshotHistorySummaryText>,
            Without<SnapshotDiffSummaryText>,
            Without<ProfileProtectionText>,
        ),
    >,
    history_bodies: Query<Entity, With<SnapshotHistoryBody>>,
) {
    let projection = &update.0;
    let summary_text = diff_summary(projection.yaml_ast_diff.as_ref());
    for mut text in &mut summary {
        text.0 = summary_text.clone();
    }

    for (mut text, marker) in &mut protection_texts {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile.write_protection.label_zh().to_owned();
        }
    }

    for (button, mut background) in &mut mode_buttons {
        background.0 = if button.split == (view.mode == SnapshotDiffViewMode::Split) {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }

    for entity in &bodies {
        rebuild_body(
            &mut commands,
            entity,
            projection.yaml_ast_diff.as_ref(),
            view.mode,
            &palette,
        );
    }

    let history_summary_text =
        snapshot_history_summary(projection.snapshot_history.as_ref(), view.prune_keep);
    for mut text in &mut history_summaries {
        if text.0 != history_summary_text {
            text.0 = history_summary_text.clone();
        }
    }
    for entity in &history_bodies {
        commands.entity(entity).despawn_children();
        let scene = super::profiles_diff_history::history_rows_scene(
            projection.snapshot_history.as_ref(),
            view.selected_snapshot.as_deref(),
            &palette,
        );
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
}

/// Toggle the shared diff layout and rebuild the rows immediately.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_snapshot_diff_mode_activated(
    activate: On<Activate>,
    buttons: Query<&SnapshotDiffModeButton>,
    mut view: ResMut<SnapshotDiffViewState>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    bodies: Query<Entity, With<SnapshotDiffBody>>,
    mut mode_buttons: Query<(&SnapshotDiffModeButton, &mut BackgroundColor)>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    view.mode = if button.split {
        SnapshotDiffViewMode::Split
    } else {
        SnapshotDiffViewMode::Inline
    };
    let diff = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.yaml_ast_diff.as_ref());
    for (button, mut background) in &mut mode_buttons {
        background.0 = if button.split == (view.mode == SnapshotDiffViewMode::Split) {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }
    for entity in &bodies {
        rebuild_body(&mut commands, entity, diff, view.mode, &palette);
    }
}

/// Ask the shared snapshot application to recompute "newest snapshot vs now".
pub(super) fn on_refresh_snapshot_diff(
    activate: On<Activate>,
    buttons: Query<(), With<RefreshSnapshotDiffButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::LoadSnapshotDiff { snapshot_id: None });
}

/// DUAL-09-09: two-step rollback. The first click arms the confirmation; only
/// a second click submits the shared restore through the apply transaction.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_rollback_snapshot_activated(
    activate: On<Activate>,
    buttons: Query<(), With<RollbackSnapshotButton>>,
    mut view: ResMut<SnapshotDiffViewState>,
    last: Option<Res<LastProfilesProjection>>,
    mut labels: Query<&mut Text, With<RollbackSnapshotLabel>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(path) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(rollback_target)
    else {
        return;
    };
    if !view.rollback_armed {
        view.rollback_armed = true;
        for mut label in &mut labels {
            label.0 = "再次点击确认回滚".to_owned();
        }
        return;
    }
    view.rollback_armed = false;
    for mut label in &mut labels {
        label.0 = "一键安全还原此快照".to_owned();
    }
    handle.submit(UiCommand::RestoreSnapshot { id: path });
}
