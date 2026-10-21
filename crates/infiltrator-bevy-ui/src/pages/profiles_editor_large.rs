//! The profile editor's large-document window (DUAL-09-13).
//!
//! A long document cannot be shown by the inline editor body, which renders a
//! bounded window around the caret. This module owns the *large-document
//! window*: a control that opens it, the bounded window it renders over the
//! same buffer, the scroll controls that move it and the status line that
//! reports where the window stands.
//!
//! The window arithmetic is the shared [`EditorViewport`], so the large window
//! and the inline body can never drift apart.

use crate::pages::profiles_editor_body::editor_rows_scene_for;
use crate::pages::profiles_editor_state::{LARGE_DOCUMENT_WINDOW_LINES, ProfileEditorState};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::BackgroundColor;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, Node, Overflow, UiRect,
    percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{metrics, radius, space};
use infiltrator_contract::editor_viewport::EditorViewport;

/// The control that opens the large-document window.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowButton;

/// The control that closes the large-document window.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowCloseButton;

/// The control that moves the large-document window one page up.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowPrevButton;

/// The control that moves the large-document window one page down.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowNextButton;

/// The large-document window's root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowPanel;

/// The large-document window's status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowStatusText;

/// The container of the large-document window's rendered rows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowBody;

/// The status line of one window: where it stands in the document.
fn window_status(viewport: &EditorViewport, total_lines: usize) -> String {
    format!(
        "lines {}..{} of {} (hidden above {}, below {})",
        viewport.first_line() + 1,
        viewport.last_line() + 1,
        total_lines,
        viewport.hidden_above(),
        viewport.hidden_below()
    )
}

/// The large-document window panel.
pub fn large_document_window_scene(
    state: &ProfileEditorState,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let viewport = state.large_window_viewport();
    let status = window_status(&viewport, state.buffer.line_count());
    let rows = editor_rows_scene_for(state, &viewport, palette);
    Box::new(bsn! {
            Node {
                width: percent(100),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S8),
                padding: UiRect::all(rem(space::S8)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                overflow: Overflow::scroll_y(),
                max_height: rem(LARGE_DOCUMENT_WINDOW_LINES as f32 * 18.0),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            ProfileEditorLargeWindowPanel
            Children [
                Text({ status }) ProfileEditorLargeWindowStatusText TextRole(Role::Mono)
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: rem(space::S8),
                }
                Children [
                    @{ scroll_button_scene(LargeWindowScroll::Up, palette) }
                    --
                    @{ scroll_button_scene(LargeWindowScroll::Down, palette) }
                    --
                    @{ close_button_scene(palette) }
                ]
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                }
                ProfileEditorLargeWindowBody
                Children [
                    @{ rows }
                ]
            ]
    })
}

/// The control that opens the large-document window.
pub fn large_window_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(metrics::CONTROL_HEIGHT_COMPACT),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(radius::CONTROL)),
            }
            BackgroundColor({ palette.surface_elevated })
            Button
            ProfileEditorLargeWindowButton
            Children [
                Text({ "⤢".to_owned() }) TextRole(Role::Caption)
            ]
    })
}

/// One scroll control of the large-document window.
fn scroll_button_scene(direction: LargeWindowScroll, palette: &UiPalette) -> Box<dyn Scene> {
    let label = match direction {
        LargeWindowScroll::Up => "↑",
        LargeWindowScroll::Down => "↓",
    };
    Box::new(bsn! {
            Node {
                min_height: rem(metrics::CONTROL_HEIGHT_COMPACT),
                min_width: rem(metrics::CONTROL_HEIGHT_COMPACT),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(radius::CONTROL)),
            }
            BackgroundColor({ palette.surface_elevated })
            Button
            ProfileEditorLargeWindowScrollButton({ direction })
            Children [
                Text({ label.to_owned() }) TextRole(Role::Body)
            ]
    })
}

/// The direction one scroll control moves the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LargeWindowScroll {
    /// Move the window towards the start of the document.
    Up,
    /// Move the window towards the end of the document.
    #[default]
    Down,
}

/// The control that closes the large-document window.
fn close_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(metrics::CONTROL_HEIGHT_COMPACT),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(radius::CONTROL)),
            }
            BackgroundColor({ palette.surface_elevated })
            Button
            ProfileEditorLargeWindowCloseButton
            Children [
                Text({ "×".to_owned() }) TextRole(Role::Body)
            ]
    })
}

/// The scroll control's typed direction.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorLargeWindowScrollButton(pub LargeWindowScroll);

/// Open the large-document window.
pub(crate) fn on_large_window_open_activated(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorLargeWindowButton>>,
    mut state: ResMut<ProfileEditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    state.open_large_window();
}

/// Close the large-document window.
pub(crate) fn on_large_window_close_activated(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorLargeWindowCloseButton>>,
    mut state: ResMut<ProfileEditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    state.close_large_window();
}

/// Move the large-document window one page up or down.
pub(crate) fn on_large_window_scrolled_activated(
    activate: On<Activate>,
    buttons: Query<&ProfileEditorLargeWindowScrollButton>,
    mut state: ResMut<ProfileEditorState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    match button.0 {
        LargeWindowScroll::Up => state.scroll_large_window(-1),
        LargeWindowScroll::Down => state.scroll_large_window(1),
    }
}

/// Restamp the large-document window: its visibility, its status line and its
/// rendered rows.
pub fn refresh_large_document_window(
    mut state: ResMut<ProfileEditorState>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    bodies: Query<Entity, With<ProfileEditorLargeWindowBody>>,
    mut panels: Query<&mut Node, With<ProfileEditorLargeWindowPanel>>,
    mut statuses: Query<&mut Text, With<ProfileEditorLargeWindowStatusText>>,
) {
    let open = state.large_window_is_open();
    for mut panel in &mut panels {
        let want = if open { Display::Flex } else { Display::None };
        if panel.display != want {
            panel.display = want;
        }
    }
    if state.last_large_rendered == state.generation {
        return;
    }
    state.last_large_rendered = state.generation;
    let viewport = state.large_window_viewport();
    let status = window_status(&viewport, state.buffer.line_count());
    for mut text in &mut statuses {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
    if !open {
        return;
    }
    for entity in &bodies {
        commands.entity(entity).despawn_children();
        let scene = editor_rows_scene_for(&state, &viewport, &palette);
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
}
