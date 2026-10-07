//! Text field: a controlled single-line input, pure core + scene adapter.
//!
//! This controlled single-line adapter predates the SDK multiline editor.
//! Native SDK keyboard, pointer and IME events are headless-testable when their
//! input plugins and message resources are installed. Multiline documents use
//! `multiline_editor`; this adapter keeps its existing controlled-field contract
//! and must not also consume events while a native editor owns focus.
//!
//! **State → visual projection** (BEVY-010): [`field_visual`] decomposes the
//! controlled state into the visible runs — before / selected / after —
//! plus the caret slot and placeholder. The scene mounts that decomposition
//! as a fixed run structure; sync systems restamp it in place.
//!
//! **CJK IME & Text Interaction Engine**:
//! - [`ime::ImeCursorArea`] and [`ime::compute_ime_cursor_area`]: absolute screen coordinate
//!   calculation for candidate window popup placement and soft keyboard avoidance;
//! - [`ime::PreeditStateMachine`]: Pinyin / CJK syllable and clause segmentation
//!   state machine with navigation and conversion states;
//! - [`TextFieldState::safe_backspace`] / [`TextFieldState::safe_delete`]:
//!   Unicode extended grapheme cluster safe deletion (never breaks emojis, flags,
//!   skin tones, or combining marks);
//! - Word boundary navigation (`WordLeft`, `WordRight`, `BackspaceWord`, `DeleteWord`);
//! - Password / masked input mode;
//! - Placeholder and validation status (Normal, Valid, Warning, Error);
//! - [`ime::ImeTransaction`]: transaction snapshots with rollback on cancellation and
//!   atomic commit.

pub mod ime;
pub mod native;
pub mod render;
pub mod state;
use state::{TextFieldState, field_visual};

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;
use accesskit;
use bevy::a11y::AccessibilityNode;
use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, Node, UiRect, Val, percent,
    px,
};
use bevy::ui::widget::Text;

/// The controlled state mounted on a field's root node.
#[derive(Component, Clone, Debug, Default)]
#[require(AccessibilityNode(accesskit::Node::new(accesskit::Role::TextInput)))]
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
                Text(placeholder_text) TextRole(Role::Body) TextFieldPlaceholder
                --
                Text(before) TextRole(Role::Body) TextFieldBefore
                --
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(0) Visibility::Hidden
                --
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                TextFieldPreedit
                Children [
                    Text({ String::new() }) TextRole(Role::Body) TextFieldPreeditText
                    --
                    Node {
                        width: percent(100),
                        height: px(palette.hairline_px),
                        flex_shrink: 0.0,
                    }
                    BackgroundColor({ palette.accent })
                    TextFieldPreeditUnderline
                ]
                --
                Node { display: Display::None, padding: UiRect::horizontal(Val::Px(space::S4)) }
                BackgroundColor({ palette.selection_fill() })
                TextFieldSelection
                Children [
                    Text({ String::new() }) TextRole(Role::Body) TextFieldSelectionText
                ]
                --
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(1) Visibility::Hidden
                --
                Text(after) TextRole(Role::Body) TextFieldAfter
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
                Text(placeholder_text) TextRole(Role::Body) TextFieldPlaceholder
                --
                Text(before) TextRole(Role::Body) TextFieldBefore
                --
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(0) Visibility::Hidden
                --
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                TextFieldPreedit
                Children [
                    Text({ String::new() }) TextRole(Role::Body) TextFieldPreeditText
                    --
                    Node {
                        width: percent(100),
                        height: px(palette.hairline_px),
                        flex_shrink: 0.0,
                    }
                    BackgroundColor({ palette.accent })
                    TextFieldPreeditUnderline
                ]
                --
                Node { display: Display::None, padding: UiRect::horizontal(Val::Px(space::S4)) }
                BackgroundColor({ palette.selection_fill() })
                TextFieldSelection
                Children [
                    Text({ String::new() }) TextRole(Role::Body) TextFieldSelectionText
                ]
                --
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(1) Visibility::Hidden
                --
                Text(after) TextRole(Role::Body) TextFieldAfter
            ]
    }
}
