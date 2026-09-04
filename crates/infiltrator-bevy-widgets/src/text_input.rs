//! Text field: a controlled single-line input, pure core + scene adapter.
//!
//! **Why not the official `EditableText` primitive** (`bevy_text::
//! EditableText` + `bevy_ui_widgets::EditableTextInputPlugin`): the official
//! editing core itself is headless-composable, but every input path that
//! drives it — `FocusedInput<KeyboardInput>` from the focus dispatcher,
//! `Pointer<Press/Drag>` click-to-place from the picking runtime, and `Ime`
//! window messages — originates in window event queues that only a windowed
//! composition registers. A `MinimalPlugins` headless composition can spawn
//! the component but can never exercise it, the same finding taskmanager
//! recorded for the menu primitives. This module therefore owns a zero-bevy
//! state machine ([`TextFieldState`]: controlled text, caret, minimal
//! selection, IME preedit) that hosts drive from whatever input seam they
//! own, and a token-skinned [`text_field_scene`] adapter. Revisit when the
//! official primitive grows a drivable edit seam.
//!
//! **State → visual projection** (BEVY-010): [`field_visual`] decomposes the
//! controlled state into the visible runs — before / selected / after —
//! plus the caret slot and placeholder. The scene mounts that decomposition
//! as a fixed run structure; sync systems restamp it in place.
//!
//! **CJK IME & Text Interaction Engine**:
//! - [`ImeCursorArea`] and [`compute_ime_cursor_area`]: absolute screen coordinate
//!   calculation for candidate window popup placement and soft keyboard avoidance;
//! - [`PreeditStateMachine`]: Pinyin / CJK syllable and clause segmentation
//!   state machine with navigation and conversion states;
//! - [`TextFieldState::safe_backspace`] / [`TextFieldState::safe_delete`]:
//!   Unicode extended grapheme cluster safe deletion (never breaks emojis, flags,
//!   skin tones, or combining marks);
//! - Word boundary navigation (`WordLeft`, `WordRight`, `BackspaceWord`, `DeleteWord`);
//! - Password / masked input mode;
//! - Placeholder and validation status (Normal, Valid, Warning, Error);
//! - [`ImeTransaction`]: transaction snapshots with rollback on cancellation and
//!   atomic commit.

pub mod ime;
pub mod state;
use ime::*;
use state::*;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Changed, Or, With, Without};
use bevy::ecs::system::{Commands, Local, Query, Res};
use bevy::math::Vec2;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::time::{Time, Virtual};
use bevy::transform::components::GlobalTransform;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, ComputedNode, FlexDirection, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;

/// The controlled state mounted on a field's root node.
#[derive(Component, Clone, Debug, Default)]
pub struct TextField(pub TextFieldState);

/// Marker on the run left of the caret.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldBefore;

/// Marker on the run right of the selection.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldAfter;

/// Marker on the placeholder label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPlaceholder;

/// Marker on the selection wash node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldSelection;

/// Marker on the text inside the selection wash.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldSelectionText;

/// Marker on the preedit column.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPreedit;

/// Marker on the preedit composition text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPreeditText;

/// Marker on the preedit underline.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPreeditUnderline;

/// One blinking caret bar.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldCaret(pub usize);

/// Focused marker component.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldFocused(pub bool);

/// A single-line field scene.
pub fn text_field_scene(initial: String, palette: &UiPalette) -> impl Scene + use<> {
    text_field_with_placeholder_scene(initial, String::new(), palette)
}

/// A single-line field scene with placeholder support.
pub fn text_field_with_placeholder_scene(
    initial: String,
    placeholder: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let state = TextFieldState::new(initial.clone()).with_placeholder(placeholder.clone());
    let visual = field_visual(&state);
    let before = visual.before;
    let after = visual.after;
    let caret_w = palette.caret_width_px;
    let caret_h = palette.control_square_px;
    let placeholder_text = if visual.is_placeholder_visible {
        placeholder
    } else {
        String::new()
    };

    bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor {
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        }
        TextField(state)
        TextFieldFocused(false)
        Children [
            ( Text(placeholder_text) TextRole(Role::Body) TextFieldPlaceholder ),
            ( Text(before) TextRole(Role::Body) TextFieldBefore ),
            (
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(0)
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                TextFieldPreedit
                Children [
                    ( Text({ String::new() }) TextRole(Role::Body) TextFieldPreeditText ),
                    (
                        Node {
                            width: percent(100),
                            height: px(palette.hairline_px),
                            flex_shrink: 0.0,
                        }
                        BackgroundColor({ palette.accent })
                        TextFieldPreeditUnderline
                    ),
                ]
            ),
            (
                Node { padding: UiRect::horizontal(Val::Px(space::S4)) }
                BackgroundColor({ palette.selection_fill() })
                TextFieldSelection
                Children [
                    ( Text({ String::new() }) TextRole(Role::Body) TextFieldSelectionText ),
                ]
            ),
            (
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(1)
            ),
            ( Text(after) TextRole(Role::Body) TextFieldAfter ),
        ]
    }
}

/// Password input scene.
pub fn password_field_scene(
    initial: String,
    placeholder: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let state = TextFieldState::new(initial.clone())
        .with_placeholder(placeholder.clone())
        .with_masked(true);
    let visual = field_visual(&state);
    let before = visual.before;
    let after = visual.after;
    let caret_w = palette.caret_width_px;
    let caret_h = palette.control_square_px;
    let placeholder_text = if visual.is_placeholder_visible {
        placeholder
    } else {
        String::new()
    };

    bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor {
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        }
        TextField(state)
        TextFieldFocused(false)
        Children [
            ( Text(placeholder_text) TextRole(Role::Body) TextFieldPlaceholder ),
            ( Text(before) TextRole(Role::Body) TextFieldBefore ),
            (
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(0)
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                TextFieldPreedit
                Children [
                    ( Text({ String::new() }) TextRole(Role::Body) TextFieldPreeditText ),
                    (
                        Node {
                            width: percent(100),
                            height: px(palette.hairline_px),
                            flex_shrink: 0.0,
                        }
                        BackgroundColor({ palette.accent })
                        TextFieldPreeditUnderline
                    ),
                ]
            ),
            (
                Node { padding: UiRect::horizontal(Val::Px(space::S4)) }
                BackgroundColor({ palette.selection_fill() })
                TextFieldSelection
                Children [
                    ( Text({ String::new() }) TextRole(Role::Body) TextFieldSelectionText ),
                ]
            ),
            (
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(1)
            ),
            ( Text(after) TextRole(Role::Body) TextFieldAfter ),
        ]
    }
}

/// Mirror controlled state onto each field's run structure, compare-and-set.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn sync_text_fields(
    palette: Res<UiPalette>,
    fields: Query<(&TextField, Option<&TextFieldFocused>, &Children)>,
    wrappers: Query<&Children>,
    mut placeholders: Query<
        (&TextFieldPlaceholder, &mut Text, &mut TextColor),
        (
            With<TextFieldPlaceholder>,
            Without<TextFieldBefore>,
            Without<TextFieldAfter>,
            Without<TextFieldSelectionText>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut befores: Query<
        (&TextFieldBefore, &mut Text),
        (
            With<TextFieldBefore>,
            Without<TextFieldPlaceholder>,
            Without<TextFieldAfter>,
            Without<TextFieldSelectionText>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut afters: Query<
        (&TextFieldAfter, &mut Text),
        (
            With<TextFieldAfter>,
            Without<TextFieldPlaceholder>,
            Without<TextFieldBefore>,
            Without<TextFieldSelectionText>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut selecteds: Query<
        (&TextFieldSelectionText, &mut Text),
        (
            With<TextFieldSelectionText>,
            Without<TextFieldPlaceholder>,
            Without<TextFieldBefore>,
            Without<TextFieldAfter>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut preedit_texts: Query<
        (&TextFieldPreeditText, &mut Text),
        (
            With<TextFieldPreeditText>,
            Without<TextFieldPlaceholder>,
            Without<TextFieldBefore>,
            Without<TextFieldAfter>,
            Without<TextFieldSelectionText>,
        ),
    >,
    mut washes: Query<
        &mut BackgroundColor,
        (
            With<TextFieldSelection>,
            Without<TextFieldPreeditUnderline>,
            Without<TextFieldCaret>,
        ),
    >,
    mut underlines: Query<
        &mut BackgroundColor,
        (
            With<TextFieldPreeditUnderline>,
            Without<TextFieldSelection>,
            Without<TextFieldCaret>,
        ),
    >,
    mut carets: Query<
        &mut BackgroundColor,
        (
            With<TextFieldCaret>,
            Without<TextFieldSelection>,
            Without<TextFieldPreeditUnderline>,
        ),
    >,
) {
    let accent = palette.accent;
    let wash = palette.selection_fill();
    let dim_ink = palette.ink_dim;

    for (field, _focused, children) in &fields {
        let visual = field_visual(&field.0);
        let preedit = field.0.preedit().to_string();
        let target_placeholder = if visual.is_placeholder_visible {
            visual.placeholder.clone()
        } else {
            String::new()
        };

        for child in children.iter() {
            if let Ok((_, mut ptext, mut pink)) = placeholders.get_mut(*child) {
                if ptext.0 != target_placeholder {
                    ptext.0 = target_placeholder.clone();
                }
                if pink.0 != dim_ink {
                    pink.0 = dim_ink;
                }
            }
            if let Ok((_, mut text)) = befores.get_mut(*child)
                && text.0 != visual.before
            {
                text.0 = visual.before.clone();
            }
            if let Ok((_, mut text)) = afters.get_mut(*child)
                && text.0 != visual.after
            {
                text.0 = visual.after.clone();
            }
            if let Ok(mut fill) = washes.get_mut(*child)
                && fill.0 != wash
            {
                fill.0 = wash;
            }
            if let Ok(mut fill) = carets.get_mut(*child)
                && fill.0 != accent
            {
                fill.0 = accent;
            }
            if let Ok(grandchildren) = wrappers.get(*child) {
                for inner in grandchildren.iter() {
                    if let Ok((_, mut text)) = selecteds.get_mut(*inner)
                        && text.0 != visual.selected
                    {
                        text.0 = visual.selected.clone();
                    }
                    if let Ok((_, mut text)) = preedit_texts.get_mut(*inner)
                        && text.0 != preedit
                    {
                        text.0 = preedit.clone();
                    }
                    if let Ok(mut fill) = underlines.get_mut(*inner)
                        && fill.0 != accent
                    {
                        fill.0 = accent;
                    }
                }
            }
        }
    }
}

/// Sync validation and focus borders on text fields.
#[allow(clippy::type_complexity)]
pub fn sync_field_borders(
    palette: Res<UiPalette>,
    mut fields: Query<
        (&TextField, Option<&TextFieldFocused>, &mut BorderColor),
        Or<(Changed<TextField>, Changed<TextFieldFocused>)>,
    >,
) {
    for (field, focused_comp, mut border) in &mut fields {
        let is_focused = focused_comp.map(|f| f.0).unwrap_or(false);
        let target = validation_border_color(field.0.validation(), is_focused, &palette);
        if border.top != target {
            border.set_all(target);
        }
    }
}

/// System to compute and update [`ImeCursorArea`] for all text fields with computed layout geometry.
#[allow(clippy::type_complexity)]
pub fn sync_ime_cursor_areas(
    mut commands: Commands,
    palette: Res<UiPalette>,
    fields: Query<(
        bevy::ecs::entity::Entity,
        &TextField,
        Option<&GlobalTransform>,
        Option<&ComputedNode>,
        Option<&ImeCursorArea>,
    )>,
) {
    for (entity, field, transform, node, existing_area) in &fields {
        let size = node
            .map(|n| n.size())
            .unwrap_or_else(|| Vec2::new(200.0, palette.control_height_px));
        let origin = transform
            .map(|t| {
                let trans = t.translation();
                Vec2::new(trans.x, trans.y) - size * 0.5
            })
            .unwrap_or(Vec2::ZERO);
        let visual = field_visual(&field.0);

        let font_size = palette.body_font_px;
        let caret_offset_x = estimate_text_width(&visual.before, font_size);
        let preedit_width = estimate_text_width(field.0.preedit(), font_size);

        let params = ImeCursorAreaParams {
            field_origin: origin,
            field_size: size,
            padding: Vec2::new(space::S12, 0.0),
            caret_offset_x,
            caret_width: palette.caret_width_px,
            caret_height: palette.control_square_px,
            preedit_width,
        };

        let calculated = compute_ime_cursor_area(params);
        if let Some(existing) = existing_area {
            if *existing != calculated {
                commands.entity(entity).insert(calculated);
            }
        } else {
            commands.entity(entity).insert(calculated);
        }
    }
}

/// The blink cadence state.
#[derive(Clone, Copy, Debug)]
pub struct CaretClock {
    elapsed: f32,
    shown: bool,
}

impl Default for CaretClock {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            shown: true,
        }
    }
}

/// Blink the active caret bar.
pub fn sync_field_carets(
    mut clock: Local<CaretClock>,
    time: Res<Time<Virtual>>,
    fields: Query<(&TextField, &Children)>,
    mut carets: Query<(&TextFieldCaret, &mut Visibility)>,
) {
    clock.elapsed += time.delta().as_secs_f32();
    if clock.elapsed >= UiPalette::CARET_BLINK_SECS {
        clock.elapsed %= UiPalette::CARET_BLINK_SECS;
        clock.shown = !clock.shown;
    }
    for (field, children) in &fields {
        let visual = field_visual(&field.0);
        for child in children.iter() {
            if let Ok((caret, mut visibility)) = carets.get_mut(*child) {
                let target = if caret.0 == visual.caret_slot && clock.shown {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *visibility != target {
                    *visibility = target;
                }
            }
        }
    }
}
