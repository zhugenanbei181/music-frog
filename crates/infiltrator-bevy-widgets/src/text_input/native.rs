//! Opt-in native input for controlled fields; specialized editors retain their own key owner.
use crate::interaction_block::InteractionBlocked;
use crate::text_input::state::TextFieldInput;
use crate::text_input::{TextField, TextFieldFocused};
use bevy::app::{App, Plugin, PreUpdate};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState, InputSystems};
use bevy::input_focus::InputFocus;
use bevy::picking::events::PointerPress;

/// Exactly one native key owner per field. Never attach to a specialized editor field.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct NativeTextField(pub i32);

pub struct NativeTextFieldPlugin;
impl Plugin for NativeTextFieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputFocus>()
            .add_message::<KeyboardInput>()
            .add_observer(focus)
            .add_systems(PreUpdate, keyboard.after(InputSystems));
    }
}
fn focus(
    press: On<PointerPress>,
    native: Query<(&TextField, Has<InteractionBlocked>), With<NativeTextField>>,
    mut fields: Query<&mut TextFieldFocused>,
    mut sdk: ResMut<InputFocus>,
) {
    let Ok((target, blocked)) = native.get(press.entity) else {
        return;
    };
    if target.0.is_disabled() || blocked {
        return;
    }
    sdk.clear();
    for mut focused in &mut fields {
        focused.0 = false;
    }
    if let Ok(mut focused) = fields.get_mut(press.entity) {
        focused.0 = true;
    }
}
fn keyboard(
    mut events: MessageReader<KeyboardInput>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut fields: Query<(
        Entity,
        &NativeTextField,
        &mut TextField,
        &mut TextFieldFocused,
        Has<InteractionBlocked>,
    )>,
) {
    let pressed = |a, b| {
        keys.as_ref()
            .is_some_and(|keys| keys.pressed(a) || keys.pressed(b))
    };
    let shift = pressed(KeyCode::ShiftLeft, KeyCode::ShiftRight);
    let shortcut = pressed(KeyCode::ControlLeft, KeyCode::ControlRight)
        || pressed(KeyCode::SuperLeft, KeyCode::SuperRight);
    let alt = pressed(KeyCode::AltLeft, KeyCode::AltRight);
    for event in events
        .read()
        .filter(|event| event.state == ButtonState::Pressed)
    {
        if event.logical_key == Key::Tab {
            let current = fields.iter().find(|(_, _, field, focused, blocked)| {
                focused.0 && !field.0.is_disabled() && !*blocked
            });
            if let Some((current, _, field, _, _)) = current {
                if field.0.is_in_ime_transaction() {
                    continue;
                }
                let mut enabled: Vec<_> = fields
                    .iter()
                    .filter(|(_, _, field, _, blocked)| !field.0.is_disabled() && !*blocked)
                    .map(|(entity, order, _, _, _)| (order.0, entity))
                    .collect();
                enabled.sort_unstable();
                let position = enabled
                    .iter()
                    .position(|(_, entity)| *entity == current)
                    .expect("focused native field is enabled");
                let next = if shift {
                    (position + enabled.len() - 1) % enabled.len()
                } else {
                    (position + 1) % enabled.len()
                };
                let selected = enabled[next].1;
                for (entity, _, _, mut focused, _) in &mut fields {
                    focused.0 = entity == selected;
                }
            }
            continue;
        }
        for (_, _, mut field, focused, blocked) in &mut fields {
            if blocked || !focused.0 || field.0.is_disabled() || field.0.is_in_ime_transaction() {
                continue;
            }
            let input = match &event.logical_key {
                Key::Character(text) if shortcut && text.eq_ignore_ascii_case("a") => {
                    Some(TextFieldInput::SelectAll)
                }
                Key::ArrowLeft if shortcut => Some(TextFieldInput::WordLeft(shift)),
                Key::ArrowRight if shortcut => Some(TextFieldInput::WordRight(shift)),
                Key::Backspace if shortcut => Some(TextFieldInput::BackspaceWord),
                Key::Delete if shortcut => Some(TextFieldInput::DeleteWord),
                _ if shortcut || alt => None,
                Key::Backspace => Some(TextFieldInput::Backspace),
                Key::Delete => Some(TextFieldInput::Delete),
                Key::ArrowLeft => Some(TextFieldInput::Left(shift)),
                Key::ArrowRight => Some(TextFieldInput::Right(shift)),
                Key::Home => Some(TextFieldInput::Home),
                Key::End => Some(TextFieldInput::End),
                Key::Character(text) => Some(TextFieldInput::Insert(
                    event
                        .text
                        .as_ref()
                        .map_or_else(|| text.to_string(), ToString::to_string),
                )),
                Key::Space => Some(TextFieldInput::Insert(" ".into())),
                _ => None,
            };
            if let Some(input) = input {
                field.0.apply(input);
            }
        }
    }
}

/// Which platform screen reader can drive the widget tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScreenReaderCapability {
    #[default]
    Unsupported,
    TalkBack,
    VoiceOver,
}

impl ScreenReaderCapability {
    pub const fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Opaque semantic node handle a screen reader targets.
///
/// Deliberately not an ECS `Entity`: the bridge seam says *what* to do
/// semantically and never exposes the widget tree's storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticNodeId(u32);

impl SemanticNodeId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// A semantic action a platform screen reader can dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenReaderAction {
    Focus(SemanticNodeId),
    Activate(SemanticNodeId),
    SetValue { node: SemanticNodeId, value: String },
}

/// Typed outcome of a screen reader dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenReaderOutcome {
    Dispatched,
    Unsupported,
}

/// Host seam for TalkBack / VoiceOver.
///
/// A platform host implements this later; the widget layer only ever holds a
/// `Box<dyn ScreenReaderBridge>` behind [`ScreenReaderGate`], so no ECS type
/// crosses the boundary.
pub trait ScreenReaderBridge: Send + Sync {
    fn capability(&self) -> ScreenReaderCapability;
    fn dispatch(&self, action: ScreenReaderAction) -> ScreenReaderOutcome;
}

/// No-op bridge used when no platform screen reader is present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoScreenReader;

impl ScreenReaderBridge for NoScreenReader {
    fn capability(&self) -> ScreenReaderCapability {
        ScreenReaderCapability::Unsupported
    }

    fn dispatch(&self, _action: ScreenReaderAction) -> ScreenReaderOutcome {
        ScreenReaderOutcome::Unsupported
    }
}

/// Capability gate over an optional host bridge.
///
/// An absent bridge is a typed [`ScreenReaderOutcome::Unsupported`], never a
/// silent success; the gate also exposes the advertised capability so callers
/// can branch before dispatching.
#[derive(Resource, Default)]
pub struct ScreenReaderGate {
    bridge: Option<Box<dyn ScreenReaderBridge>>,
}

impl ScreenReaderGate {
    pub fn absent() -> Self {
        Self { bridge: None }
    }

    pub fn install(bridge: impl ScreenReaderBridge + 'static) -> Self {
        Self {
            bridge: Some(Box::new(bridge)),
        }
    }

    pub fn capability(&self) -> ScreenReaderCapability {
        match self.bridge.as_ref() {
            Some(bridge) => bridge.capability(),
            None => ScreenReaderCapability::Unsupported,
        }
    }

    pub fn is_supported(&self) -> bool {
        self.capability().is_supported()
    }

    pub fn dispatch(&self, action: ScreenReaderAction) -> ScreenReaderOutcome {
        match self.bridge.as_ref() {
            Some(bridge) => bridge.dispatch(action),
            None => ScreenReaderOutcome::Unsupported,
        }
    }
}
