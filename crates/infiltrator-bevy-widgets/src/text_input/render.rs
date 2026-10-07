//! In-place single-line field rendering with explicit, disjoint ECS queries.

use crate::palette::UiPalette;
use crate::text_input::ime::{
    ImeCursorArea, ImeCursorAreaParams, compute_ime_cursor_area, estimate_text_width,
};
use crate::text_input::state::{field_visual, validation_border_color};
use crate::text_input::{
    TextField, TextFieldAfter, TextFieldBefore, TextFieldCaret, TextFieldFocused,
    TextFieldPlaceholder, TextFieldPreedit, TextFieldPreeditText, TextFieldPreeditUnderline,
    TextFieldSelection, TextFieldSelectionText,
};
use crate::theme::space;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Changed, Or, QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Local, Query, Res, SystemParam};
use bevy::math::Vec2;
use bevy::text::TextColor;
use bevy::time::{Time, Virtual};
use bevy::transform::components::GlobalTransform;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, BorderColor, ComputedNode, Display, Node};

#[derive(QueryFilter)]
pub struct PlaceholderFilter {
    kind: With<TextFieldPlaceholder>,
    exclude_text_field_before: Without<TextFieldBefore>,
    exclude_text_field_after: Without<TextFieldAfter>,
    exclude_text_field_selection_text: Without<TextFieldSelectionText>,
    exclude_text_field_preedit_text: Without<TextFieldPreeditText>,
}

#[derive(QueryFilter)]
pub struct BeforeFilter {
    kind: With<TextFieldBefore>,
    exclude_text_field_placeholder: Without<TextFieldPlaceholder>,
    exclude_text_field_after: Without<TextFieldAfter>,
    exclude_text_field_selection_text: Without<TextFieldSelectionText>,
    exclude_text_field_preedit_text: Without<TextFieldPreeditText>,
}

#[derive(QueryFilter)]
pub struct AfterFilter {
    kind: With<TextFieldAfter>,
    exclude_text_field_placeholder: Without<TextFieldPlaceholder>,
    exclude_text_field_before: Without<TextFieldBefore>,
    exclude_text_field_selection_text: Without<TextFieldSelectionText>,
    exclude_text_field_preedit_text: Without<TextFieldPreeditText>,
}

#[derive(QueryFilter)]
pub struct SelectionTextFilter {
    kind: With<TextFieldSelectionText>,
    exclude_text_field_placeholder: Without<TextFieldPlaceholder>,
    exclude_text_field_before: Without<TextFieldBefore>,
    exclude_text_field_after: Without<TextFieldAfter>,
    exclude_text_field_preedit_text: Without<TextFieldPreeditText>,
}

#[derive(QueryFilter)]
pub struct PreeditTextFilter {
    kind: With<TextFieldPreeditText>,
    exclude_text_field_placeholder: Without<TextFieldPlaceholder>,
    exclude_text_field_before: Without<TextFieldBefore>,
    exclude_text_field_after: Without<TextFieldAfter>,
    exclude_text_field_selection_text: Without<TextFieldSelectionText>,
}

#[derive(QueryFilter)]
pub struct PreeditRootFilter {
    kind: With<TextFieldPreedit>,
    exclude_text_field_selection: Without<TextFieldSelection>,
}

#[derive(QueryFilter)]
pub struct SelectionWashFilter {
    kind: With<TextFieldSelection>,
    exclude_text_field_preedit: Without<TextFieldPreedit>,
    exclude_text_field_preedit_underline: Without<TextFieldPreeditUnderline>,
    exclude_text_field_caret: Without<TextFieldCaret>,
}

#[derive(QueryFilter)]
pub struct UnderlineFilter {
    kind: With<TextFieldPreeditUnderline>,
    exclude_text_field_selection: Without<TextFieldSelection>,
    exclude_text_field_caret: Without<TextFieldCaret>,
}

#[derive(QueryFilter)]
pub struct CaretFilter {
    kind: With<TextFieldCaret>,
    exclude_text_field_selection: Without<TextFieldSelection>,
    exclude_text_field_preedit_underline: Without<TextFieldPreeditUnderline>,
}

#[derive(SystemParam)]
pub struct FieldRuns<'w, 's> {
    fields: Query<'w, 's, (&'static TextField, &'static Children)>,
    wrappers: Query<'w, 's, &'static Children>,
    placeholders: Query<'w, 's, (&'static mut Text, &'static mut TextColor), PlaceholderFilter>,
    befores: Query<'w, 's, &'static mut Text, BeforeFilter>,
    afters: Query<'w, 's, &'static mut Text, AfterFilter>,
    selecteds: Query<'w, 's, &'static mut Text, SelectionTextFilter>,
    preedit_texts: Query<'w, 's, &'static mut Text, PreeditTextFilter>,
    preedit_roots: Query<'w, 's, &'static mut Node, PreeditRootFilter>,
    washes: Query<'w, 's, (&'static mut BackgroundColor, &'static mut Node), SelectionWashFilter>,
    underlines: Query<'w, 's, &'static mut BackgroundColor, UnderlineFilter>,
    carets: Query<'w, 's, &'static mut BackgroundColor, CaretFilter>,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct FieldBorder {
    field: &'static TextField,
    focused: Option<&'static TextFieldFocused>,
    border: &'static mut BorderColor,
}

#[derive(QueryFilter)]
pub struct FieldBorderChanged {
    changed: Or<(Changed<TextField>, Changed<TextFieldFocused>)>,
}

#[derive(QueryData)]
pub struct FieldGeometry {
    entity: Entity,
    field: &'static TextField,
    transform: Option<&'static GlobalTransform>,
    node: Option<&'static ComputedNode>,
    existing_area: Option<&'static ImeCursorArea>,
}

/// Mirror controlled state onto each field's run structure, compare-and-set.
pub fn sync_text_fields(palette: Res<UiPalette>, runs: FieldRuns) {
    let FieldRuns {
        fields,
        wrappers,
        mut placeholders,
        mut befores,
        mut afters,
        mut selecteds,
        mut preedit_texts,
        mut preedit_roots,
        mut washes,
        mut underlines,
        mut carets,
    } = runs;
    let accent = palette.accent;
    let wash = palette.selection_fill();
    let dim_ink = palette.ink_dim;

    for (field, children) in &fields {
        let visual = field_visual(&field.0);
        let preedit = field.0.preedit().to_string();
        let target_placeholder = if visual.is_placeholder_visible {
            visual.placeholder.clone()
        } else {
            String::new()
        };

        for child in children.iter() {
            if let Ok(mut node) = preedit_roots.get_mut(*child) {
                node.display = if preedit.is_empty() {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            if let Ok((mut ptext, mut pink)) = placeholders.get_mut(*child) {
                if ptext.0 != target_placeholder {
                    ptext.0 = target_placeholder.clone();
                }
                if pink.0 != dim_ink {
                    pink.0 = dim_ink;
                }
            }
            if let Ok(mut text) = befores.get_mut(*child)
                && text.0 != visual.before
            {
                text.0 = visual.before.clone();
            }
            if let Ok(mut text) = afters.get_mut(*child)
                && text.0 != visual.after
            {
                text.0 = visual.after.clone();
            }
            if let Ok((mut fill, mut node)) = washes.get_mut(*child) {
                if fill.0 != wash {
                    fill.0 = wash;
                }
                node.display = if visual.selected.is_empty() {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            if let Ok(mut fill) = carets.get_mut(*child)
                && fill.0 != accent
            {
                fill.0 = accent;
            }
            if let Ok(grandchildren) = wrappers.get(*child) {
                for inner in grandchildren.iter() {
                    if let Ok(mut text) = selecteds.get_mut(*inner)
                        && text.0 != visual.selected
                    {
                        text.0 = visual.selected.clone();
                    }
                    if let Ok(mut text) = preedit_texts.get_mut(*inner)
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
pub fn sync_field_borders(
    palette: Res<UiPalette>,
    mut fields: Query<FieldBorder, FieldBorderChanged>,
) {
    for FieldBorderItem {
        field,
        focused: focused_comp,
        mut border,
    } in &mut fields
    {
        let is_focused = focused_comp.map(|f| f.0).unwrap_or(false);
        let target = validation_border_color(field.0.validation(), is_focused, &palette);
        if border.top != target {
            border.set_all(target);
        }
    }
}

/// System to compute and update [`ImeCursorArea`] for all text fields with computed layout geometry.
pub fn sync_ime_cursor_areas(
    mut commands: Commands,
    palette: Res<UiPalette>,
    fields: Query<FieldGeometry>,
) {
    for FieldGeometryItem {
        entity,
        field,
        transform,
        node,
        existing_area,
    } in &fields
    {
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
    fields: Query<(&TextField, Option<&TextFieldFocused>, &Children)>,
    mut carets: Query<(&TextFieldCaret, &mut Visibility)>,
) {
    clock.elapsed += time.delta().as_secs_f32();
    if clock.elapsed >= UiPalette::CARET_BLINK_SECS {
        clock.elapsed %= UiPalette::CARET_BLINK_SECS;
        clock.shown = !clock.shown;
    }
    for (field, focused, children) in &fields {
        let visual = field_visual(&field.0);
        let is_focused = focused.is_some_and(|focused| focused.0);
        for child in children.iter() {
            if let Ok((caret, mut visibility)) = carets.get_mut(*child) {
                let target = if is_focused && caret.0 == visual.caret_slot && clock.shown {
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
