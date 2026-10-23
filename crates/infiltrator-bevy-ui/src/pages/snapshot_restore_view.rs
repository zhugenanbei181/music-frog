//! Restricted component replay and native interaction gates preserve background editing state.
use crate::pages::snapshot_restore::RestoreState;
use crate::pages::snapshot_restore_scene::{
    RestoreControl, RestoreDiffRoot, RestoreDiffRow, RestoreDiffSummary, RestoreLine,
    RestoreOverlay, ReviewControl,
};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::EditableText;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{Display, FlexDirection, Node, UiRect, percent, px};
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::interaction_block::InteractionBlocked;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::yaml_ast_diff::{DiffKind, YamlAstDiffSnapshot};
use infiltrator_presentation::snapshot_restore_projection::project_restore;
use infiltrator_shared::locales::Lang;

#[derive(QueryFilter)]
pub struct BackgroundFilter {
    controls: Or<(With<Button>, With<EditableText>, With<TextField>)>,
}
#[derive(QueryData)]
pub struct BackgroundControl {
    entity: Entity,
    review: Has<ReviewControl>,
    blocked: Has<InteractionBlocked>,
}
#[derive(SystemParam)]
pub struct RestoreView<'w, 's> {
    state: ResMut<'w, RestoreState>,
    locale: Res<'w, UiLocale>,
    focus: ResMut<'w, InputFocus>,
    overlays: Query<'w, 's, &'static mut Node, With<RestoreOverlay>>,
    labels: Query<'w, 's, (&'static RestoreLine, &'static mut Text)>,
    controls: Query<'w, 's, (&'static RestoreControl, &'static mut ButtonDisabled)>,
    backgrounds: Query<'w, 's, BackgroundControl, BackgroundFilter>,
    live: Query<'w, 's, ()>,
}
pub fn replay(mut view: RestoreView, mut commands: Commands) {
    let open = view.state.model.is_visible();
    let presentation = project_restore(&view.state.model, &Lang(view.locale.code()));
    for mut node in &mut view.overlays {
        node.display = if open { Display::Flex } else { Display::None };
    }
    for (role, mut text) in &mut view.labels {
        let copy = match role {
            RestoreLine::Status => &presentation.status,
            RestoreLine::Details => &presentation.details,
            RestoreLine::Content => &presentation.content,
        };
        if text.0 != *copy {
            text.0.clone_from(copy);
        }
    }
    for (control, mut disabled) in &mut view.controls {
        disabled.0 = match control {
            RestoreControl::Confirm => !presentation.confirm,
            RestoreControl::Cancel => !presentation.cancel,
            RestoreControl::Retry => !presentation.retry,
        };
    }
    if open {
        if view.state.previous_focus.is_none() {
            view.state.previous_focus = view.focus.get();
        }
        for target in &view.backgrounds {
            if target.review || target.blocked {
                continue;
            }
            commands.entity(target.entity).insert(InteractionBlocked);
            view.state.blocked.push(target.entity);
        }
        if view.focus.get().is_some_and(|entity| {
            view.backgrounds
                .get(entity)
                .is_ok_and(|target| !target.review)
        }) {
            view.focus.clear();
        }
    } else {
        for entity in view.state.blocked.drain(..) {
            if view.live.contains(entity) {
                commands.entity(entity).remove::<InteractionBlocked>();
            }
        }
        if let Some(entity) = view
            .state
            .previous_focus
            .take()
            .filter(|entity| view.live.contains(*entity))
        {
            view.focus.set(entity, FocusCause::Navigated);
        }
    }
}

/// The diff the comparator last rendered.
///
/// The rows are rebuilt only when the owner's diff really changes, so the
/// comparator is a projection of one fact, not a second copy of it.
#[derive(Resource, Default)]
pub struct RenderedRestoreDiff(pub Option<YamlAstDiffSnapshot>);

/// The ink one side of a comparator row draws with.
fn side_color(kind: DiffKind, palette: &UiPalette) -> Color {
    match kind {
        DiffKind::Equal => palette.ink_dim,
        DiffKind::Insert => palette.success,
        DiffKind::Delete => palette.danger,
        DiffKind::Modify => palette.warning,
    }
}

/// One comparator row: the current document's line beside the restored one.
fn diff_row_scene(
    left: String,
    right: String,
    left_color: Color,
    right_color: Color,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            column_gap: rem(space::S12),
            padding: UiRect::axes(rem(space::S4), rem(space::S2)),
        }
        RestoreDiffRow
        Children [
            Node { flex_grow: 1.0, min_width: px(0.0) }
            Children [ Text(left) TextRole(Role::Mono) TextColor(left_color) ]
            --
            Node { flex_grow: 1.0, min_width: px(0.0) }
            Children [ Text(right) TextRole(Role::Mono) TextColor(right_color) ]
        ]
    }
}

type RestoreDiffNodeQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Node, Option<&'static RestoreLine>),
    Or<(With<RestoreDiffRoot>, With<RestoreLine>)>,
>;

/// The review's diff comparator.
///
/// The owner's own Myers diff is the fact: the review shows the two
/// documents side by side, and the plain text blob is only the fallback for
/// a review that carries no diff at all.
pub fn sync_restore_diff(
    state: Res<RestoreState>,
    palette: Res<UiPalette>,
    mut rendered: ResMut<RenderedRestoreDiff>,
    mut nodes: RestoreDiffNodeQuery,
    mut summaries: Query<&mut Text, With<RestoreDiffSummary>>,
    mut commands: Commands,
) {
    let diff = state.model.review().map(|review| review.difference.clone());
    let rows = diff
        .as_ref()
        .map(|diff| diff.split_rows.clone())
        .unwrap_or_default();
    let has_diff = !rows.is_empty();
    let mut roots: Vec<Entity> = Vec::new();

    for (entity, mut node, line) in &mut nodes {
        let display = match line {
            Some(RestoreLine::Content) => {
                if has_diff {
                    Display::None
                } else {
                    Display::Flex
                }
            }
            Some(_) => continue,
            None => {
                if has_diff {
                    Display::Flex
                } else {
                    Display::None
                }
            }
        };
        if node.display != display {
            node.display = display;
        }
        if line.is_none() {
            roots.push(entity);
        }
    }
    if let Some(diff) = diff.as_ref() {
        let summary = format!(
            "+{} -{} ~{}",
            diff.stats.additions, diff.stats.deletions, diff.stats.modifications
        );
        for mut text in &mut summaries {
            if text.0 != summary {
                text.0 = summary.clone();
            }
        }
    }
    if rendered.0 == diff {
        return;
    }
    rendered.0 = diff;
    // Batch teardown by marker (capability norm ⑦).
    commands.despawn_all::<With<RestoreDiffRow>>();
    for root in &roots {
        for row in &rows {
            let left = row.left.as_ref();
            let right = row.right.as_ref();
            let left_text = left
                .map(|line| format!("{} {}", line.kind.symbol(), line.content))
                .unwrap_or_default();
            let right_text = right
                .map(|line| format!("{} {}", line.kind.symbol(), line.content))
                .unwrap_or_default();
            let left_color = side_color(left.map(|line| line.kind).unwrap_or(row.kind), &palette);
            let right_color = side_color(right.map(|line| line.kind).unwrap_or(row.kind), &palette);
            commands
                .spawn_scene(diff_row_scene(
                    left_text,
                    right_text,
                    left_color,
                    right_color,
                ))
                .insert(ChildOf(*root));
        }
    }
}
