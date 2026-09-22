//! Command-palette runtime wiring for the Bevy shell (DUAL-15-05).
//!
//! The catalogue, the typed targets and the shortcut registry are shared
//! contract; this module mounts the BSN overlay, owns the palette keyboard
//! seam (arrows / Enter / Escape / typing), and dispatches one selected entry
//! into the same handlers the global chords use. Dispatch mirrors the Iced
//! `ExecuteCommand` arm one-to-one.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyCode;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::scene::CommandsSceneExt;
use bevy::ui_widgets::Activate;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_contract::command_catalogue::{CommandCatalogue, CommandTarget};
use infiltrator_contract::shortcuts::ShortcutRegistry;
use infiltrator_contract::system_toggle::SystemToggle;

use crate::app::SidebarToggleProjection;
use crate::appearance::{SystemAppearance, ThemeMode, resolved_skin};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_palette::{
    CommandPaletteOverlayRoot, CommandPaletteRow, CommandPaletteState, ExecutePaletteEntry,
    ExecuteSelectedPaletteAction, command_palette_modal_scene,
};
use crate::mini_hud::ToggleMiniHud;
use crate::pages::profiles::LastProfilesProjection;
use crate::route::{ActiveRoute, Route, RouteChanged};
use crate::shortcuts::ShortcutBindings;

/// The last mounted overlay signature: a remount happens only when the open
/// flag, query, cursor or result set actually changed.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PaletteOverlaySignature {
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub filtered_indices: Vec<usize>,
    pub catalogue_len: usize,
}

/// Rebuild the palette catalogue whenever the stored profile projection
/// changes, so both surfaces list the same live profile rows.
pub fn sync_palette_catalogue(
    profiles: Option<Res<LastProfilesProjection>>,
    mut state: ResMut<CommandPaletteState>,
) {
    let Some(profiles) = profiles else {
        return;
    };
    if !profiles.is_changed() {
        return;
    }
    let choices: Vec<infiltrator_contract::command_catalogue::ProfileChoice> = profiles
        .0
        .as_ref()
        .map(|projection| {
            projection
                .profiles
                .iter()
                .map(|profile| {
                    infiltrator_contract::command_catalogue::ProfileChoice::new(
                        profile.id.clone(),
                        profile.name.clone(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    state.set_catalogue(CommandCatalogue::with_profiles(&choices));
}

/// The palette owns the keyboard while it is open: ↑↓ move (wrapping), Enter
/// executes, Escape closes, Backspace erases and printable keys type into the
/// query. Modified chords still fall through to the global shortcut registry
/// (Ctrl+K toggles the palette closed, Ctrl+Alt+M toggles the HUD, …).
pub fn palette_keyboard_input(
    mut keys: MessageReader<KeyboardInput>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<CommandPaletteState>,
    mut commands: Commands,
) {
    if !state.is_open {
        keys.clear();
        return;
    }
    let modifiers = keyboard
        .as_deref()
        .map(crate::shortcuts::modifiers_from_keyboard)
        .unwrap_or_default();
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            continue;
        }
        match &key.logical_key {
            Key::Escape => commands.trigger(crate::command_palette::CloseCommandPalette),
            Key::Enter => commands.trigger(ExecuteSelectedPaletteAction),
            Key::ArrowDown => state.select_next(),
            Key::ArrowUp => state.select_prev(),
            Key::Backspace => state.pop_query_char(),
            Key::Space => {
                if let Some(text) = &key.text {
                    for character in text.chars() {
                        state.push_query_char(character);
                    }
                }
            }
            Key::Character(text) => {
                if let Some(text) = &key.text {
                    for character in text.chars() {
                        state.push_query_char(character);
                    }
                } else {
                    for character in text.chars() {
                        state.push_query_char(character);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Mount / unmount the modal scene whenever the palette signature changes.
#[allow(clippy::too_many_arguments)]
pub fn sync_palette_overlay(
    mut commands: Commands,
    palette: Res<UiPalette>,
    state: Res<CommandPaletteState>,
    bindings: Res<ShortcutBindings>,
    mut signature: ResMut<PaletteOverlaySignature>,
    mounted: Query<Entity, With<CommandPaletteOverlayRoot>>,
) {
    let next = PaletteOverlaySignature {
        is_open: state.is_open,
        query: state.query.clone(),
        selected_index: state.selected_index,
        filtered_indices: state.filtered_indices.clone(),
        catalogue_len: state.catalogue.len(),
    };
    if next == *signature {
        return;
    }
    *signature = next;
    for entity in &mounted {
        commands.entity(entity).despawn();
    }
    if !state.is_open {
        return;
    }
    let registry: &ShortcutRegistry = &bindings.0;
    commands.spawn_scene(command_palette_modal_scene(&palette, &state, registry));
}

/// Observer: clicking a row executes that row.
pub fn on_palette_row_activated(
    activate: On<Activate>,
    rows: Query<&CommandPaletteRow>,
    mut commands: Commands,
) {
    let Ok(row) = rows.get(activate.entity) else {
        return;
    };
    commands.trigger(ExecutePaletteEntry(row.0));
}

/// Observer: execute the entry at a display index (row click path).
pub fn on_execute_palette_entry(
    trigger: On<ExecutePaletteEntry>,
    mut state: ResMut<CommandPaletteState>,
    mut commands: Commands,
) {
    let display_index = trigger.event().0;
    if display_index < state.filtered_indices.len() {
        state.selected_index = display_index;
    }
    commands.trigger(ExecuteSelectedPaletteAction);
}

/// Observer executing the currently selected catalogue entry.
#[allow(clippy::too_many_arguments)]
pub fn on_execute_selected_palette_action(
    _trigger: On<ExecuteSelectedPaletteAction>,
    mut state: ResMut<CommandPaletteState>,
    mut active_route: ResMut<ActiveRoute>,
    sink: Option<Res<CommandSinkHandle>>,
    theme_mode: Option<ResMut<ThemeMode>>,
    appearance: Option<Res<SystemAppearance>>,
    toggles: Option<Res<SidebarToggleProjection>>,
    mut commands: Commands,
) {
    let Some(entry) = state.current_selected_action().cloned() else {
        state.close();
        return;
    };
    let target = entry.target.clone();
    match target {
        CommandTarget::Navigate(page) => {
            let route = Route::from_shell_page(page);
            active_route.0 = Some(route);
            commands.trigger(RouteChanged(route));
        }
        CommandTarget::SetProxyMode(mode) => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::SetProxyMode(mode));
            }
        }
        CommandTarget::SwitchProfile { id, .. } => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::ActivateProfile { id });
            }
        }
        CommandTarget::ToggleSystemProxy => {
            if let Some(desired) = desired_toggle(toggles.as_deref(), SystemToggle::SystemProxy)
                && let Some(sink) = sink.as_deref()
            {
                sink.submit(UiCommand::SetSystemProxy { enabled: desired });
            }
        }
        CommandTarget::ToggleTun => {
            if let Some(desired) = desired_toggle(toggles.as_deref(), SystemToggle::Tun)
                && let Some(sink) = sink.as_deref()
            {
                sink.submit(UiCommand::ToggleTun { enabled: desired });
            }
        }
        CommandTarget::ToggleMiniHud => commands.trigger(ToggleMiniHud),
        CommandTarget::CycleTheme => {
            let next = theme_mode
                .as_ref()
                .map(|mode| mode.0)
                .unwrap_or_default()
                .next();
            if let Some(mut mode) = theme_mode {
                mode.0 = next;
            }
            commands.trigger(ThemeSwitch(resolved_skin(
                next,
                appearance.as_ref().and_then(|value| value.0),
            )));
        }
        CommandTarget::FlushDnsCache => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::ClearDnsCache);
            }
        }
        CommandTarget::TestAllProxyGroups => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::TestAllProxyGroups);
            }
        }
        CommandTarget::RunDoctor => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::RunDoctorDiagnostics);
            }
        }
        CommandTarget::CloseAllConnections => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::CloseAllConnections);
            }
        }
        CommandTarget::RestartKernel => {
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::RestartCore);
            }
        }
    }
    state.close();
}

/// The next desired value of a system toggle, resolved through the shared
/// application policy (a pending/unknown control accepts no press).
fn desired_toggle(toggles: Option<&SidebarToggleProjection>, toggle: SystemToggle) -> Option<bool> {
    let toggles = toggles?;
    let desired = !toggles.0.state(toggle).is_enabled();
    SystemToggleApplication::intent(&toggles.0, toggle, desired)
        .ok()
        .map(|_| desired)
}

/// Observer to open/close/toggle the palette (registered by the plugin).
pub fn on_open_command_palette(
    _trigger: On<crate::command_palette::OpenCommandPalette>,
    mut state: ResMut<CommandPaletteState>,
) {
    state.open();
}

pub fn on_close_command_palette(
    _trigger: On<crate::command_palette::CloseCommandPalette>,
    mut state: ResMut<CommandPaletteState>,
) {
    state.close();
}

pub fn on_toggle_command_palette(
    _trigger: On<crate::command_palette::ToggleCommandPalette>,
    mut state: ResMut<CommandPaletteState>,
) {
    state.toggle();
}

/// The palette plugin: resource, observers, keyboard seam and overlay mount.
pub struct CommandPalettePlugin;

impl Plugin for CommandPalettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandPaletteState>();
        app.init_resource::<PaletteOverlaySignature>();
        // The accelerators render from the shared registry; a host that mounts
        // the palette without the shortcut plugin still gets the product
        // defaults.
        app.init_resource::<ShortcutBindings>();
        // The palette owns its keyboard seam. A headless composition has no
        // InputPlugin, so register the message type here; with a windowed
        // composition the same resource is reused.
        app.add_message::<KeyboardInput>();
        app.add_observer(on_open_command_palette);
        app.add_observer(on_close_command_palette);
        app.add_observer(on_toggle_command_palette);
        app.add_observer(on_palette_row_activated);
        app.add_observer(on_execute_palette_entry);
        app.add_observer(on_execute_selected_palette_action);
        app.add_systems(
            Update,
            (
                sync_palette_catalogue,
                palette_keyboard_input,
                sync_palette_overlay,
            )
                .chain(),
        );
    }
}
