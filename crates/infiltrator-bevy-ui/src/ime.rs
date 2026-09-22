//! Bevy host wiring for the real OS IME path (DUAL-15-11).
//!
//! Bevy 0.19 exposes the whole path: `Window::ime_enabled` reaches
//! `winit::Window::set_ime_allowed`, `Window::ime_position` reaches
//! `set_ime_cursor_area` (logical pixels), and `bevy::window::Ime` carries the
//! composition events. Nothing consumed any of it, so this module owns the
//! three shell responsibilities:
//!
//! * **enable + position**: the focused widget-layer text field's
//!   [`ImeCursorArea`] (already computed from the real caret run) becomes the
//!   shared [`ImeFocusPlan`]; the window's IME is enabled at that caret and
//!   disabled again when no field is focused;
//! * **consume**: `Ime::Preedit`/`Commit` map through the shared
//!   [`ImeCompositionTracker`] and land in the focused field's controlled
//!   state (rollback snapshot on open, inline preedit, committed insert);
//! * **report**: [`ImeHostReport`] records the capability and the live plan so
//!   a headless composition can assert the same facts the windowed one
//!   produces, and the widget layer's caret geometry is never faked here.
//!
//! The caret x is the widget layer's advance estimate for the text before the
//! caret (not a shaped glyph advance); that boundary is recorded rather than
//! smoothed over.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, ResMut};
use bevy::math::Vec2;
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_bevy_widgets::text_input::ime::ImeCursorArea;
use infiltrator_bevy_widgets::text_input::state::TextFieldState;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::ime::{
    ImeCompositionAction, ImeCompositionEvent, ImeCompositionTracker, ImeCursorRect,
    ImeCursorSource, ImeCursorSupport, ImeFocusPlan, ImeViewport,
};

/// What this surface can do with the OS IME cursor area: it computes the caret
/// itself and writes it to the real window.
pub const fn cursor_support() -> ImeCursorSupport {
    ImeCursorSupport::Hosted {
        source: ImeCursorSource::SurfaceComputed,
    }
}

/// The shell's live composition session: a Bevy resource wrapper around the
/// shared tracker, so the contract stays free of any toolkit dependency.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellImeComposition(pub ImeCompositionTracker);

impl ShellImeComposition {
    /// Whether the user is composing right now.
    pub fn is_composing(&self) -> bool {
        self.0.is_composing()
    }

    /// The live composition string.
    pub fn preedit(&self) -> &str {
        self.0.preedit()
    }
}

/// The live IME facts of this shell, inserted by [`ShellImePlugin`].
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ImeHostReport {
    /// The capability this host claims (never changes at runtime).
    pub support: ImeCursorSupport,
    /// What the OS IME is told right now.
    pub plan: ImeFocusPlan,
    /// The primary window that received the plan (`None` on a headless
    /// composition without a window entity).
    pub window: Option<Entity>,
    /// The field that owns the IME right now (`None` when nothing is focused).
    pub focused_field: Option<Entity>,
}

impl Default for ImeHostReport {
    fn default() -> Self {
        Self {
            support: cursor_support(),
            plan: ImeFocusPlan::DISABLED,
            window: None,
            focused_field: None,
        }
    }
}

impl ImeHostReport {
    /// Whether the OS is currently asked to run the IME at a caret.
    pub const fn is_enabled(&self) -> bool {
        self.plan.is_enabled()
    }

    /// The caret the OS IME avoids, when enabled.
    pub const fn cursor(&self) -> Option<ImeCursorRect> {
        self.plan.cursor()
    }
}

/// The window an [`Ime`] message belongs to.
pub fn ime_window(event: &Ime) -> Entity {
    match event {
        Ime::Preedit { window, .. }
        | Ime::Commit { window, .. }
        | Ime::Enabled { window }
        | Ime::Disabled { window } => *window,
    }
}

/// Map one Bevy IME message onto the shared composition vocabulary. The window
/// id stays host-local; both surfaces feed the same transitions in.
pub fn shared_composition_event(event: &Ime) -> ImeCompositionEvent {
    match event {
        Ime::Enabled { .. } => ImeCompositionEvent::Opened,
        Ime::Preedit { value, .. } => ImeCompositionEvent::Preedit(value.clone()),
        Ime::Commit { value, .. } => ImeCompositionEvent::Commit(value.clone()),
        Ime::Disabled { .. } => ImeCompositionEvent::Closed,
    }
}

/// The caret rectangle of one widget-layer text field.
pub fn cursor_rect(area: &ImeCursorArea) -> ImeCursorRect {
    ImeCursorRect::new(area.position.x, area.position.y, area.size.x, area.size.y)
}

/// Apply one shared composition action to a controlled field.
///
/// `Begin` snapshots the field so `Cancel` can roll it back; `Commit` replaces
/// the composition with the committed text through the field's own insert path.
pub fn apply_composition_action(field: &mut TextFieldState, action: ImeCompositionAction) -> bool {
    match action {
        ImeCompositionAction::Begin => {
            field.begin_ime_transaction();
            true
        }
        ImeCompositionAction::UpdatePreedit(text) => field.set_preedit(text),
        ImeCompositionAction::Commit(text) => field.commit_ime_transaction(&text),
        ImeCompositionAction::Cancel => field.rollback_ime_transaction(),
    }
}

/// Installs the IME cursor-area sync and composition routing.
pub struct ShellImePlugin;

impl Plugin for ShellImePlugin {
    fn build(&self, app: &mut App) {
        // The headless composition needs the message channel registered; a
        // windowed one already has it via `WindowPlugin` (registration is
        // idempotent).
        app.add_message::<Ime>()
            .init_resource::<ImeHostReport>()
            .init_resource::<ShellImeComposition>()
            // The widget layer recomputes the caret rect after a preedit
            // change, so the pipeline is: route → widget caret → window plan.
            .add_systems(
                Update,
                route_ime_composition
                    .before(infiltrator_bevy_widgets::text_input::sync_ime_cursor_areas),
            )
            .add_systems(
                Update,
                sync_window_ime.after(infiltrator_bevy_widgets::text_input::sync_ime_cursor_areas),
            );
    }
}

/// Feed every IME message into the shared tracker and the focused field.
fn route_ime_composition(
    mut events: MessageReader<Ime>,
    mut tracker: ResMut<ShellImeComposition>,
    mut fields: Query<&mut TextField>,
    focused: Query<(Entity, &TextFieldFocused)>,
    primary: Query<Entity, With<PrimaryWindow>>,
) {
    let primary = primary.single().ok();
    let target = focused
        .iter()
        .find(|(_, focused)| focused.0)
        .map(|(entity, _)| entity);
    for event in events.read() {
        if primary.is_some_and(|primary| ime_window(event) != primary) {
            continue;
        }
        let action = tracker.0.apply(shared_composition_event(event));
        let Some(entity) = target else {
            continue;
        };
        if let Ok(mut field) = fields.get_mut(entity) {
            apply_composition_action(&mut field.0, action);
        }
    }
}

/// Project the focused field's real caret onto the window's IME slot.
fn sync_window_ime(
    mut report: ResMut<ImeHostReport>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    fields: Query<(Entity, &TextFieldFocused, &ImeCursorArea)>,
) {
    let target = fields
        .iter()
        .find(|(_, focused, _)| focused.0)
        .map(|(entity, _, area)| (entity, cursor_rect(area)));
    let focused_field = target.map(|(entity, _)| entity);
    let cursor = target.map(|(_, rect)| rect);

    let Ok((window_entity, mut window)) = windows.single_mut() else {
        // No host window: report the honest plan without touching an OS slot.
        report.window = None;
        report.focused_field = focused_field;
        report.plan = ImeFocusPlan::from_focused_cursor(ImeViewport::UNBOUNDED, cursor);
        return;
    };

    let plan = ImeFocusPlan::from_focused_cursor(
        ImeViewport::new(window.width(), window.height()),
        cursor,
    );
    if window.ime_enabled != plan.enabled {
        window.ime_enabled = plan.enabled;
    }
    if let Some(rect) = plan.cursor {
        let position = Vec2::new(rect.x, rect.y);
        if window.ime_position != position {
            window.ime_position = position;
        }
    }
    report.support = cursor_support();
    report.plan = plan;
    report.window = Some(window_entity);
    report.focused_field = focused_field;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bevy_ime_message_maps_to_one_shared_transition() {
        let entity = Entity::PLACEHOLDER;
        assert_eq!(
            shared_composition_event(&Ime::Enabled { window: entity }),
            ImeCompositionEvent::Opened
        );
        assert_eq!(
            shared_composition_event(&Ime::Preedit {
                window: entity,
                value: "ni hao".to_owned(),
                cursor: Some((3, 6)),
            }),
            ImeCompositionEvent::Preedit("ni hao".to_owned())
        );
        assert_eq!(
            shared_composition_event(&Ime::Commit {
                window: entity,
                value: "你好".to_owned(),
            }),
            ImeCompositionEvent::Commit("你好".to_owned())
        );
        assert_eq!(
            shared_composition_event(&Ime::Disabled { window: entity }),
            ImeCompositionEvent::Closed
        );
        assert_eq!(ime_window(&Ime::Disabled { window: entity }), entity);
    }

    #[test]
    fn a_commit_inserts_the_composed_text_through_the_controlled_state() {
        let mut field = TextFieldState::new("代理");
        assert!(apply_composition_action(
            &mut field,
            ImeCompositionAction::Begin
        ));
        assert!(apply_composition_action(
            &mut field,
            ImeCompositionAction::UpdatePreedit("ni hao".to_owned())
        ));
        assert_eq!(field.preedit(), "ni hao");
        assert!(apply_composition_action(
            &mut field,
            ImeCompositionAction::Commit("你好".to_owned())
        ));
        assert_eq!(field.text(), "代理你好");
        assert_eq!(field.preedit(), "");
        assert!(!field.is_in_ime_transaction());

        // A cancel rolls the field back to the snapshot taken at Begin.
        let mut cancelled = TextFieldState::new("代理");
        assert!(apply_composition_action(
            &mut cancelled,
            ImeCompositionAction::Begin
        ));
        assert!(apply_composition_action(
            &mut cancelled,
            ImeCompositionAction::UpdatePreedit("ni".to_owned())
        ));
        assert!(apply_composition_action(
            &mut cancelled,
            ImeCompositionAction::Cancel
        ));
        assert_eq!(cancelled.text(), "代理");
        assert_eq!(cancelled.preedit(), "");
    }

    #[test]
    fn the_shell_reports_the_surface_computed_cursor_source() {
        let report = ImeHostReport::default();
        assert!(report.support.is_hosted());
        assert_eq!(
            report.support.source(),
            Some(ImeCursorSource::SurfaceComputed)
        );
        assert!(!report.is_enabled(), "a cold shell disables the IME");
    }

    #[test]
    fn the_widget_caret_becomes_the_shared_rect() {
        let area = ImeCursorArea::from_rect(96.0, 210.0, 2.0, 18.0);
        assert_eq!(
            cursor_rect(&area),
            ImeCursorRect::new(96.0, 210.0, 2.0, 18.0)
        );
        assert!(!cursor_rect(&area).is_degenerate());
    }
}
