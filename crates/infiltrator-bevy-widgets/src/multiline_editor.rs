//! Native SDK multiline input: one keyboard/selection/clipboard/IME owner.
use crate::interaction_block::InteractionBlocked;
use crate::localization::LocalizedLabel;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::text_input::TextFieldFocused;
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, PostUpdate};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, QueryData, With};
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::gamepad::GamepadButtonChangedEvent;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::input_focus::pointer_focus::PointerFocusPlugin;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{InputDispatchPlugin, InputFocus, InputFocusPlugin};
use bevy::picking::events::{PointerPress, PointerRelease, PointerState};
use bevy::scene::{Scene, bsn};
use bevy::text::{
    EditableText, EditableTextSystems, TextCursorStyle, TextPlugin, TextReadWriteMode,
};
use bevy::ui::{
    BackgroundColor, BorderColor, BorderRadius, InteractionDisabled, Node, Overflow, UiRect,
    UiScale, UiSystems, percent, px,
};
use bevy::ui_widgets::{TextInput, TextInputPlugin};
use bevy::window::Ime;

#[derive(Component, Clone, Copy, Default)]
#[require(TextInput)]
pub struct MultilineEditor;
#[derive(Component, Clone, Copy, Default)]
pub struct MultilineReadOnly(pub bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub struct MultilineModeSet;
pub struct MultilineEditorPlugin;
impl Plugin for MultilineEditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TextPlugin>() {
            app.add_plugins(TextPlugin);
        }
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<InputDispatchPlugin>() {
            app.add_plugins(InputDispatchPlugin);
        }
        if !app.is_plugin_added::<PointerFocusPlugin>() {
            app.add_plugins(PointerFocusPlugin);
        }
        if !app.is_plugin_added::<TextInputPlugin>() {
            app.add_plugins(TextInputPlugin);
        }
        app.add_message::<KeyboardInput>()
            .add_message::<PointerRelease>()
            .add_message::<MouseWheel>()
            .add_message::<GamepadButtonChangedEvent>()
            .init_resource::<ButtonInput<Key>>()
            .add_message::<Ime>()
            .init_resource::<UiScale>()
            .init_resource::<PointerState>()
            .add_observer(clear_legacy_focus)
            .add_systems(
                PostUpdate,
                (sync_style, sync_mode, ApplyDeferred)
                    .chain()
                    .in_set(MultilineModeSet)
                    .after(EditableTextSystems)
                    .before(UiSystems::PostLayout),
            );
    }
}
pub fn multiline_editor_scene(
    initial: &str,
    label: &'static str,
    order: i32,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut editable = EditableText::new(initial);
    editable.allow_newlines = true;
    editable.visible_lines = Some(8.0);
    editable.cursor_width = 0.12;
    let palette = *palette;
    let cursor_style = TextCursorStyle {
        color: palette.ink,
        selection_color: palette.selection_fill(),
        unfocused_selection_color: palette.hover_bg,
        selected_text_color: Some(palette.ink),
        ..Default::default()
    };
    bsn! {
        Node { width: percent(100), min_width: px(0), min_height: px(160), max_width: percent(100),
            overflow: Overflow::clip(), padding: UiRect::all(px(8)),
            border: UiRect::all(px(palette.hairline_px)), border_radius: BorderRadius::all(px(palette.control_radius_px)) }
        BackgroundColor({palette.surface_elevated})
        BorderColor::all(palette.border)
        MultilineEditor MultilineReadOnly(false) TabIndex(order)
        EditableText { ..{editable} } TextRole(Role::Mono)
        cursor_style
        LocalizedLabel::plain(label)
    }
}
fn clear_legacy_focus(
    event: On<PointerPress>,
    native: Query<(), With<MultilineEditor>>,
    mut fields: Query<&mut TextFieldFocused>,
) {
    if native.contains(event.entity) {
        for mut field in &mut fields {
            field.0 = false;
        }
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct EditorMode {
    entity: Entity,
    readonly: &'static MultilineReadOnly,
    blocked: Has<InteractionBlocked>,
    sdk_disabled: Has<InteractionDisabled>,
    mode: &'static mut TextReadWriteMode,
    accessibility: &'static mut AccessibilityNode,
}
fn sync_mode(
    mut query: Query<EditorMode, With<MultilineEditor>>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for mut editor in &mut query {
        *editor.mode = if editor.blocked {
            TextReadWriteMode::Static
        } else if editor.readonly.0 {
            TextReadWriteMode::ReadOnly
        } else {
            TextReadWriteMode::Editable
        };
        if editor.blocked {
            editor.accessibility.0.set_disabled();
            if !editor.sdk_disabled {
                commands.entity(editor.entity).insert(InteractionDisabled);
            }
            if focus.get() == Some(editor.entity) {
                focus.clear();
            }
        } else {
            editor.accessibility.0.clear_disabled();
            if editor.sdk_disabled {
                commands
                    .entity(editor.entity)
                    .remove::<InteractionDisabled>();
            }
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct EditorStyle {
    background: &'static mut BackgroundColor,
    border: &'static mut BorderColor,
    cursor: &'static mut TextCursorStyle,
}
fn sync_style(palette: Res<UiPalette>, mut fields: Query<EditorStyle, With<MultilineEditor>>) {
    if !palette.is_changed() {
        return;
    }
    for mut field in &mut fields {
        field.background.0 = palette.surface_elevated;
        *field.border = BorderColor::all(palette.border);
        field.cursor.color = palette.ink;
        field.cursor.selection_color = palette.selection_fill();
        field.cursor.unfocused_selection_color = palette.hover_bg;
        field.cursor.selected_text_color = Some(palette.ink);
    }
}
