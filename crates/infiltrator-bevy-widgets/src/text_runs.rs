//! Native text-span replay. Search decisions arrive as immutable neutral runs.
use crate::bidi::LayoutDirection;
use crate::fonts::FontSources;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole, role_typography};
use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{Changed, QueryData, With, Without};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{CommandsSceneExt, bsn};
use bevy::text::{Justify, TextColor, TextFont, TextLayout, TextSpan};
use bevy::ui::prelude::{Display, FlexDirection, Node};
use bevy::ui::widget::Text;
use infiltrator_contract::search_text::SearchTextRun;

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(Text)]
pub struct TextRuns(pub Vec<SearchTextRun>);
#[derive(Component, Clone, Copy, Default)]
pub struct TextRunSpan;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TextRunsInk(pub Color);

/// Authoring input that propagates a writing direction onto a run block's
/// layout: the root flex row and inline text alignment mirror together.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextRunsDirection(pub LayoutDirection);

/// Cached base (LTR) layout so re-applying a direction stays idempotent.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct AppliedRunLayout {
    base_flex: FlexDirection,
    base_justify: Justify,
}
#[derive(QueryData)]
#[query_data(mutable)]
struct RunRoot {
    entity: Entity,
    runs: &'static TextRuns,
    text: &'static mut Text,
    font: &'static TextFont,
    color: &'static mut TextColor,
    role: Option<&'static TextRole>,
    ink: Option<&'static TextRunsInk>,
    children: Option<&'static Children>,
    node: Option<&'static mut Node>,
}
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRunsSet;
#[derive(QueryData)]
#[query_data(mutable)]
struct RunSpanData {
    text: &'static TextSpan,
    font: &'static mut TextFont,
    color: &'static mut TextColor,
}
pub struct TextRunsPlugin;
impl Plugin for TextRunsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (replay_runs.in_set(TextRunsSet), sync_text_runs_direction).chain(),
        );
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
struct DirectionRoot {
    entity: Entity,
    direction: &'static TextRunsDirection,
    node: &'static mut Node,
    layout: &'static mut TextLayout,
    applied: Option<&'static AppliedRunLayout>,
}
fn sync_text_runs_direction(
    mut commands: Commands,
    mut roots: Query<DirectionRoot, (With<TextRuns>, Changed<TextRunsDirection>)>,
) {
    for mut root in &mut roots {
        let base_flex = root
            .applied
            .map_or(root.node.flex_direction, |state| state.base_flex);
        let base_justify = root
            .applied
            .map_or(root.layout.justify, |state| state.base_justify);
        let (flex, justify) = root.direction.0.mirror_layout(base_flex, base_justify);
        root.node.flex_direction = flex;
        root.layout.justify = justify;
        commands.entity(root.entity).insert(AppliedRunLayout {
            base_flex,
            base_justify,
        });
    }
}
fn replay_runs(
    mut roots: Query<RunRoot, With<TextRuns>>,
    mut spans: Query<RunSpanData, (With<TextRunSpan>, Without<TextRuns>)>,
    palette: Res<UiPalette>,
    fonts: Option<Res<FontSources>>,
    mut commands: Commands,
) {
    for mut root in &mut roots {
        let display = if root.runs.0.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if let Some(node) = root.node.as_deref_mut()
            && node.display != display
        {
            node.display = display;
        }
        let plain = root.ink.map(|ink| ink.0).unwrap_or_else(|| {
            role_typography(
                root.role.map_or(Role::Body, |role| role.0),
                &palette,
                fonts.as_deref(),
            )
            .ink
        });
        let runs = &root.runs.0;
        let base = runs
            .first()
            .map(|run| run.text.as_str())
            .unwrap_or_default();
        if root.text.0 != base {
            root.text.0 = base.into();
        }
        root.color.0 = if runs.first().is_some_and(|run| run.highlighted) {
            palette.accent
        } else {
            plain
        };
        let remaining = runs.get(1..).unwrap_or_default();
        let existing: Vec<_> = root
            .children
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter(|child| spans.get(*child).is_ok())
            .collect();
        let unchanged = existing.len() == remaining.len()
            && existing
                .iter()
                .zip(remaining)
                .all(|(entity, run)| spans.get(*entity).is_ok_and(|span| span.text.0 == run.text));
        if unchanged {
            for (entity, run) in existing.into_iter().zip(remaining) {
                if let Ok(mut part) = spans.get_mut(entity) {
                    if *part.font != *root.font {
                        *part.font = root.font.clone();
                    }
                    part.color.0 = if run.highlighted {
                        palette.accent
                    } else {
                        plain
                    };
                }
            }
        } else {
            for entity in existing {
                commands.entity(entity).despawn();
            }
            for run in remaining {
                let color = if run.highlighted {
                    palette.accent
                } else {
                    plain
                };
                let parent = root.entity;
                let font = root.font.clone();
                let scene = bsn! { TextSpan({run.text.clone()}) TextColor(color) TextRunSpan };
                commands.spawn_scene(scene).insert((ChildOf(parent), font));
            }
        }
    }
}
