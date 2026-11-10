//! Opt-in native input for controlled fields; specialized editors retain their own key owner.
use crate::interaction_block::InteractionBlocked;
use crate::text_input::state::TextFieldInput;
use crate::text_input::{TextField, TextFieldFocused};
use bevy::app::{App, Plugin, PreUpdate};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::SystemParam;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState, InputSystems};
use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::TabGroup;
use bevy::picking::events::PointerPress;
use bevy::ui::prelude::{Display, Node};

/// Exactly one native key owner per field. Never attach to a specialized editor field.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct NativeTextField(pub i32);

/// The tab ring one native field belongs to.
///
/// A ring is **not** "every field in the world", which is what made two
/// unrelated fields on one page answer the same `NativeTextField(0)`: the
/// ordering fact was a per-field number compared across the whole surface, so
/// the winner was decided by archetype storage order rather than by anything
/// the user can see. The ring is adjudicated here, in the widget layer, with
/// the official dialog semantics as the scope rule:
///
/// * a field inside a **modal** tab group (Bevy's own `ModalDialog` marks its
///   `TabGroup` `modal`) walks only that dialog's fields, so a presented
///   dialog is never tabbed through and never leaks focus to the page behind
///   it — the same rule the official `TabNavigation` implements for `TabIndex`;
/// * a field outside one walks every field that is not inside a modal dialog;
/// * a field whose branch is `Display::None` is not in any ring: a hidden
///   control is not focusable, which is what keeps a closed dialog's draft
///   fields from stealing a tab stop on the page;
/// * ties on the declared order are broken by **paint order** (the tree
///   position the user reads), never by entity storage order, so a duplicate
///   number is a deterministic sequence instead of a coin flip.
#[derive(SystemParam)]
pub struct NativeTabRing<'w, 's> {
    fields: Query<'w, 's, (Entity, &'static NativeTextField, Option<&'static ChildOf>)>,
    parents: Query<'w, 's, &'static ChildOf>,
    children: Query<'w, 's, &'static Children>,
    nodes: Query<'w, 's, &'static Node>,
    groups: Query<'w, 's, (Entity, &'static TabGroup)>,
}

impl NativeTabRing<'_, '_> {
    /// The fields one tab stroke may move between, in the order the stroke
    /// visits them. `current` is the focused field, so the ring is scoped
    /// against the dialog the user is actually in.
    ///
    /// The answer keeps only the *identity and sequence* of the reachable
    /// fields; whether one of them is currently disabled or interaction-blocked
    /// is read afterwards from the caller's own query, so this parameter never
    /// borrows a component the caller mutates.
    pub fn ring(&self, current: Entity) -> Vec<Entity> {
        let scope = self.modal_scope(current);
        let mut ordered: Vec<(i32, usize, Entity)> = self
            .fields
            .iter()
            .filter(|(entity, _, _)| *entity != current)
            .filter(|(entity, _, _)| self.reachable_in_scope(*entity, scope))
            .map(|(entity, order, _)| (order.0, self.paint_index(entity), entity))
            .collect();
        // The focused field is part of its own ring: the stroke wraps.
        if let Ok((_, order, _)) = self.fields.get(current) {
            ordered.push((order.0, self.paint_index(current), current));
        }
        ordered.sort();
        ordered.into_iter().map(|(_, _, entity)| entity).collect()
    }

    /// The nearest modal tab group above `entity`, if it is inside one.
    fn modal_scope(&self, entity: Entity) -> Option<Entity> {
        let mut current = self
            .fields
            .get(entity)
            .ok()
            .and_then(|(_, _, parent)| parent.map(|parent| parent.parent()))?;
        loop {
            if let Ok((group, tab_group)) = self.groups.get(current)
                && tab_group.modal
            {
                return Some(group);
            }
            current = self.parents.get(current).ok()?.parent();
        }
    }

    /// Whether `entity` is a visible member of the ring `scope` selects.
    fn reachable_in_scope(&self, entity: Entity, scope: Option<Entity>) -> bool {
        if self.hidden(entity) {
            return false;
        }
        let Some(scope) = scope else {
            // Outside a dialog: a field that *is* inside a presented dialog is
            // that dialog's business, not the page's.
            return self.modal_scope(entity).is_none();
        };
        self.is_descendant_of(entity, scope)
    }

    fn is_descendant_of(&self, entity: Entity, ancestor: Entity) -> bool {
        let mut current = self.parents.get(entity).ok().map(|parent| parent.parent());
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.parents.get(parent).ok().map(|parent| parent.parent());
        }
        false
    }

    /// Whether any node from `entity` up to the root is `Display::None`.
    ///
    /// The declared layout is the fact a headless host can read: the render
    /// stack's `InheritedVisibility` does not exist without `UiPlugin`, and a
    /// ring must not depend on whether a renderer happens to be attached.
    fn hidden(&self, entity: Entity) -> bool {
        let mut current = Some(entity);
        while let Some(node) = current {
            if self
                .nodes
                .get(node)
                .is_ok_and(|node| node.display == Display::None)
            {
                return true;
            }
            current = self.parents.get(node).ok().map(|parent| parent.parent());
        }
        false
    }

    /// The position `entity` holds in a depth-first walk of its root subtree:
    /// the order the surface paints it.
    fn paint_index(&self, entity: Entity) -> usize {
        let mut index = 0usize;
        if self.walk(self.root_of(entity), entity, &mut index) {
            index
        } else {
            usize::MAX
        }
    }

    /// The scene root `entity` hangs under: the node the shell spawned.
    fn root_of(&self, entity: Entity) -> Entity {
        let mut current = entity;
        while let Ok(parent) = self.parents.get(current) {
            current = parent.parent();
        }
        current
    }

    /// Depth-first walk; `index` counts the nodes visited before the match, so
    /// two fields under one parent come back in the order they paint.
    fn walk(&self, node: Entity, wanted: Entity, index: &mut usize) -> bool {
        *index += 1;
        if node == wanted {
            return true;
        }
        if let Ok(children) = self.children.get(node) {
            for child in children.iter() {
                if self.walk(*child, wanted, index) {
                    return true;
                }
            }
        }
        false
    }
}

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
    ring: NativeTabRing,
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
                // The ring is the widget layer's answer, not a world-wide sort
                // of every declared number: a dialog scopes it, a hidden
                // branch leaves it, and a tie resolves by paint order.
                let enabled: Vec<_> = ring
                    .ring(current)
                    .into_iter()
                    .filter(|entity| {
                        fields.get(*entity).is_ok_and(|(_, _, field, _, blocked)| {
                            !field.0.is_disabled() && !blocked
                        })
                    })
                    .collect();
                let position = enabled
                    .iter()
                    .position(|entity| *entity == current)
                    .expect("focused native field is enabled");
                let next = if shift {
                    (position + enabled.len() - 1) % enabled.len()
                } else {
                    (position + 1) % enabled.len()
                };
                let selected = enabled[next];
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
