//! Configuration Snapshot Visual Diff & Rollback scene component (快照比对与秒级回滚).
//!
//! DUAL-09-08/09/12: the card renders the shared `YamlAstDiffSnapshot` the
//! snapshot application computed from a real snapshot file and the current
//! profile content. It never fabricates `+/-/~` rows; when no diff has been
//! computed yet it says so and offers a refresh action. The rollback button
//! routes through the shared apply transaction behind a two-step confirmation.

#[path = "profiles_diff_query_access.rs"]
pub mod query_access;
use self::query_access::{SnapshotDiffModeControls, SnapshotDiffTargets};

use super::profiles_diff_history::{
    SnapshotHistoryBody, SnapshotHistorySummaryText, backup_button, history_refresh_button,
    history_rows_scene, prune_button, prune_keep_row,
};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{
    LastProfilesProjection, ProfilesProjection, ProfilesProjectionUpdated,
};
use crate::pages::snapshot_restore::OpenSnapshotRestore;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::snapshot_presentation;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_contract::yaml_ast_diff::{DiffKind, DiffLine, YamlAstDiffSnapshot};

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

/// Surface-local layout and selection; restoration is owned by its independent workbench.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SnapshotDiffViewState {
    pub mode: SnapshotDiffViewMode,
    /// DUAL-09-06: history entry the user selected to diff (and roll back to).
    pub selected_snapshot: Option<String>,
    /// DUAL-09-07: retention requested by the manual prune control.
    pub prune_keep: usize,
}

impl Default for SnapshotDiffViewState {
    fn default() -> Self {
        Self {
            mode: SnapshotDiffViewMode::default(),
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
    let summary = snapshot_presentation::diff_summary(
        projection.yaml_ast_diff.as_ref(),
        UiLocale::default().code(),
    );
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
    let history_summary = snapshot_presentation::history_summary(
        history.as_ref(),
        SNAPSHOT_DEFAULT_KEEP,
        UiLocale::default().code(),
    );
    let initial_history_rows = history_rows_scene(history.as_ref(), None, palette);

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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::FileText, 24.0, palette) }
                                    --
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(space::S4),
                                    }
                                    Children [
                                        LocalizedText::plain("profiles_snapshot_diff_title") TextRole(Role::BodyStrong)
                                        --
                                        Text(summary) SnapshotDiffSummaryText TextRole(Role::Caption)
                                    ]
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ mode_button("snapshot_diff_inline", false, palette) }
                                    --
                                    @{ mode_button("snapshot_diff_split", true, palette) }
                                    --
                                    @{ refresh_button(palette) }
                                    --
                                    @{ rollback_button(diff_available, palette) }
                                ]
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
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    Text({ history_summary }) SnapshotHistorySummaryText TextRole(Role::Caption)
                                    --
                                    Node {
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(space::S8),
                                    }
                                    Children [
                                        @{ backup_button(palette) }
                                        --
                                        @{ history_refresh_button(palette) }
                                        --
                                        @{ prune_keep_row(palette) }
                                        --
                                        @{ prune_button(palette) }
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S2),
                                }
                                SnapshotHistoryBody
                                Children [
                                    @{ initial_history_rows }
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S4),
                                    padding: UiRect::top(Val::Px(space::S4)),
                                }
                                Children [
                                    Node {
                                        width: percent(100),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(space::S2),
                                    }
                                    SnapshotDiffBody
                                    Children [
                                        @{ initial_rows }
                                    ]
                                ]
                            ]
            }),
        ],
        palette,
    )
}

fn mode_button(key: &'static str, split: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let background = if split {
        palette.surface_elevated
    } else {
        palette.accent
    };
    let label = LocalizedText::plain(key);
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
            SnapshotDiffModeButton { split }
            Children [
                label TextRole(Role::Caption)
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
                LocalizedText::plain("snapshot_diff_refresh") TextRole(Role::Body)
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
                LocalizedText::plain("profiles_snapshot_restore_action") RollbackSnapshotLabel TextRole(Role::BodyStrong)
            ]
    })
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
                Text({ label }) TextRole(Role::Mono) TextColor({ color })
            ]
    })
}

pub(super) fn diff_notice_scene(key: &'static str) -> Box<dyn Scene> {
    let label = LocalizedText::plain(key);
    Box::new(bsn! {
            Node {
                width: percent(100),
            }
            Children [
                label TextRole(Role::Caption)
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
        Some(_) => vec![diff_notice_scene("snapshot_diff_identical")],
        None => vec![diff_notice_scene("snapshot_diff_unobserved")],
    };
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S2),
            }
            Children [
                { rows }
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
pub(super) fn sync_snapshot_diff(
    update: On<ProfilesProjectionUpdated>,
    palette: Res<UiPalette>,
    view: Res<SnapshotDiffViewState>,
    mut commands: Commands,
    targets: SnapshotDiffTargets,
    locale: Res<UiLocale>,
) {
    let SnapshotDiffTargets {
        mut summary,
        mut mode_buttons,
        bodies,
        mut history_summaries,
        history_bodies,
    } = targets;

    let projection = &update.0;
    let summary_text =
        snapshot_presentation::diff_summary(projection.yaml_ast_diff.as_ref(), locale.code());
    for mut text in &mut summary {
        text.0 = summary_text.clone();
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

    let history_summary_text = snapshot_presentation::history_summary(
        projection.snapshot_history.as_ref(),
        view.prune_keep,
        locale.code(),
    );
    for mut text in &mut history_summaries {
        if text.0 != history_summary_text {
            text.0 = history_summary_text.clone();
        }
    }
    for entity in &history_bodies {
        commands.entity(entity).despawn_children();
        let scene = history_rows_scene(
            projection.snapshot_history.as_ref(),
            view.selected_snapshot.as_deref(),
            &palette,
        );
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
}

/// Toggle the shared diff layout and rebuild the rows immediately.
pub(super) fn on_snapshot_diff_mode_activated(
    activate: On<Activate>,
    mut view: ResMut<SnapshotDiffViewState>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    targets: SnapshotDiffModeControls,
) {
    let SnapshotDiffModeControls {
        buttons,
        bodies,
        mut mode_buttons,
    } = targets;

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

/// Open the independent restoration review; the launcher never commits a write.
pub(super) fn on_rollback_snapshot_activated(
    activate: On<Activate>,
    buttons: Query<(), With<RollbackSnapshotButton>>,
    last: Option<Res<LastProfilesProjection>>,
    mut commands: Commands,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let Some(id) = rollback_target(projection) else {
        return;
    };
    let Some(profile) = projection
        .yaml_ast_diff
        .as_ref()
        .map(|diff| diff.target_id.clone())
    else {
        return;
    };
    commands.trigger(OpenSnapshotRestore(SnapshotRestoreTarget {
        profile,
        snapshot_id: id,
    }));
}
