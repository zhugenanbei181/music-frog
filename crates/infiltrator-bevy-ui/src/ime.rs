//! Bevy host wiring for the real OS IME path (DUAL-15-11).
//!
//! Bevy 0.20 exposes the whole path: `Window::ime_enabled` reaches
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

use crate::ime_native::{native_editor_active, report_native_ime};
use crate::pages::connections_clipboard::ClipboardHostPlugin;
use bevy::app::PostUpdate;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::Res;
use bevy::ecs::system::{Query, ResMut};
use bevy::input_focus::InputFocus;
use bevy::math::Vec2;
use bevy::text::EditableText;
use bevy::ui::UiScale;
use bevy::ui_widgets::ImeSystems;
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_bevy_widgets::multiline_editor::MultilineEditor;
use infiltrator_bevy_widgets::text_input::ime::ImeCursorArea;
use infiltrator_bevy_widgets::text_input::render::sync_ime_cursor_areas;
use infiltrator_bevy_widgets::text_input::state::{TextFieldInput, TextFieldState};
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
        ImeCompositionAction::UpdatePreedit(text) => {
            // A window can stay IME-enabled while focus moves between fields.
            // The new field then receives Preedit without another Enabled.
            if !field.is_in_ime_transaction() {
                field.begin_ime_transaction();
            }
            field.set_preedit(text)
        }
        ImeCompositionAction::Commit(text) => field.commit_ime_transaction(&text),
        ImeCompositionAction::Cancel => field.rollback_ime_transaction(),
    }
}

/// One editing command a native text-input adapter can ask the focused
/// controlled field to perform (BANDROID-007). These are the operations an
/// Android `InputConnection` reports through `performEditorAction` /
/// `sendKeyEvent` and are distinct from the composition lifecycle above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEditAction {
    SelectAll,
    MoveLeft,
    MoveRight,
    MoveWordLeft,
    MoveWordRight,
    Home,
    End,
    Backspace,
    Delete,
    BackspaceWord,
    DeleteWord,
}

/// The typed text-input vocabulary a native `InputConnection`/`GameTextInput`
/// adapter feeds into the shell (BANDROID-007).
///
/// This is the mobile sibling of [`shared_composition_event`]: the adapter
/// calls one method per platform callback and the shell applies it to the same
/// [`ImeCompositionTracker`] and controlled [`TextFieldState`] the windowed
/// `bevy::window::Ime` path already drives. Composition lifecycle, selection,
/// deletion, candidate commits and editing actions are distinct variants so an
/// adapter never has to guess how a platform callback maps onto the owner.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum NativeTextInput {
    /// `onCreateInputConnection`: the IME opened a composition session.
    Open,
    /// `setComposingText`: the inline composition changed (empty finishes).
    Preedit(String),
    /// `commitText`: the composition committed `text`.
    Commit(String),
    /// A candidate was selected: commit its `text` (same finalize as `Commit`).
    CandidateCommit(String),
    /// `finishComposingText`/`closeConnection`: drop the uncommitted text.
    Cancel,
    /// `setSelection(start, end)`: replace the controlled selection.
    Selection { start: usize, end: usize },
    /// `deleteSurroundingText`: remove `before`/`after` chars at the caret.
    Delete { before: usize, after: usize },
    /// `performEditorAction`/`sendKeyEvent`: one editing action.
    Edit(NativeEditAction),
    /// Hardware/OS back: close the IME first, then the host may navigate.
    Back,
}

/// What one native text-input event did to the shell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeInputOutcome {
    /// Nothing to do (no focused owner, or an action that changed nothing).
    #[default]
    Ignored,
    /// The controlled owner changed.
    Applied,
    /// The composition closed and the field rolled back to its snapshot.
    RolledBack,
    /// A second commit arrived after the composition already committed.
    DuplicateCommitDropped,
    /// OS back closed the IME first; the host must not navigate.
    BackConsumed,
    /// OS back found no composition; the host may navigate.
    BackIgnored,
}

/// The shell's native text-input session (BANDROID-007).
///
/// `committed_after_composition` latches a finished composition so the
/// duplicate `commitText` an adapter can emit right after a candidate commit is
/// dropped instead of inserting twice. It is only armed when the commit
/// followed a real composition, so plain per-character `commitText` typing is
/// never collapsed.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeImeSession {
    committed_after_composition: bool,
    ime_closed_by_back: bool,
    last_outcome: NativeInputOutcome,
}

impl NativeImeSession {
    /// The outcome of the most recent native text-input event.
    pub const fn last_outcome(&self) -> NativeInputOutcome {
        self.last_outcome
    }

    /// Whether OS back closed the IME and no new composition has reopened it.
    pub const fn is_ime_closed_by_back(&self) -> bool {
        self.ime_closed_by_back
    }
}

/// Apply one native text-input event to the shared tracker and controlled
/// field, returning what happened. Pure so the adapter's callback order can be
/// asserted without a window.
pub fn apply_native_text_input(
    session: &mut NativeImeSession,
    tracker: &mut ImeCompositionTracker,
    event: NativeTextInput,
    field: &mut TextFieldState,
) -> NativeInputOutcome {
    let outcome = match event {
        NativeTextInput::Open => {
            session.committed_after_composition = false;
            session.ime_closed_by_back = false;
            let action = tracker.apply(ImeCompositionEvent::Opened);
            apply_composition_action(field, action);
            NativeInputOutcome::Applied
        }
        NativeTextInput::Preedit(text) => {
            if !text.is_empty() {
                session.committed_after_composition = false;
            }
            session.ime_closed_by_back = false;
            let action = tracker.apply(ImeCompositionEvent::Preedit(text));
            apply_composition_action(field, action);
            NativeInputOutcome::Applied
        }
        NativeTextInput::Commit(text) | NativeTextInput::CandidateCommit(text) => {
            if session.committed_after_composition {
                return finish_native(session, NativeInputOutcome::DuplicateCommitDropped);
            }
            let was_composing = tracker.is_composing();
            let action = tracker.apply(ImeCompositionEvent::Commit(text));
            apply_composition_action(field, action);
            session.committed_after_composition = was_composing;
            NativeInputOutcome::Applied
        }
        NativeTextInput::Cancel => {
            let action = tracker.apply(ImeCompositionEvent::Closed);
            let rolled = apply_composition_action(field, action);
            session.committed_after_composition = false;
            if rolled {
                NativeInputOutcome::RolledBack
            } else {
                NativeInputOutcome::Ignored
            }
        }
        NativeTextInput::Selection { start, end } => {
            if apply_native_selection(field, start, end) {
                NativeInputOutcome::Applied
            } else {
                NativeInputOutcome::Ignored
            }
        }
        NativeTextInput::Delete { before, after } => {
            if apply_native_delete(field, before, after) {
                NativeInputOutcome::Applied
            } else {
                NativeInputOutcome::Ignored
            }
        }
        NativeTextInput::Edit(action) => {
            if field.apply(native_edit_input(action)) {
                NativeInputOutcome::Applied
            } else {
                NativeInputOutcome::Ignored
            }
        }
        NativeTextInput::Back => {
            if tracker.is_composing() || field.is_in_ime_transaction() {
                let action = tracker.apply(ImeCompositionEvent::Closed);
                apply_composition_action(field, action);
                session.committed_after_composition = false;
                session.ime_closed_by_back = true;
                NativeInputOutcome::BackConsumed
            } else {
                NativeInputOutcome::BackIgnored
            }
        }
    };
    finish_native(session, outcome)
}

fn finish_native(
    session: &mut NativeImeSession,
    outcome: NativeInputOutcome,
) -> NativeInputOutcome {
    session.last_outcome = outcome;
    outcome
}

fn native_edit_input(action: NativeEditAction) -> TextFieldInput {
    match action {
        NativeEditAction::SelectAll => TextFieldInput::SelectAll,
        NativeEditAction::MoveLeft => TextFieldInput::Left(false),
        NativeEditAction::MoveRight => TextFieldInput::Right(false),
        NativeEditAction::MoveWordLeft => TextFieldInput::WordLeft(false),
        NativeEditAction::MoveWordRight => TextFieldInput::WordRight(false),
        NativeEditAction::Home => TextFieldInput::Home,
        NativeEditAction::End => TextFieldInput::End,
        NativeEditAction::Backspace => TextFieldInput::Backspace,
        NativeEditAction::Delete => TextFieldInput::Delete,
        NativeEditAction::BackspaceWord => TextFieldInput::BackspaceWord,
        NativeEditAction::DeleteWord => TextFieldInput::DeleteWord,
    }
}

/// Place the caret at `start` and extend the selection to `end` using the
/// controlled field's own navigation, so the field's state stays authoritative.
fn apply_native_selection(field: &mut TextFieldState, start: usize, end: usize) -> bool {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let len = field.text().chars().count();
    let start = start.min(len);
    let end = end.min(len);
    let mut changed = field.apply(TextFieldInput::Home);
    for _ in 0..start {
        changed |= field.apply(TextFieldInput::Right(false));
    }
    for _ in start..end {
        changed |= field.apply(TextFieldInput::Right(true));
    }
    changed
}

/// Remove `before` chars behind and `after` chars ahead of the caret through the
/// field's own grapheme-aware backspace/delete.
fn apply_native_delete(field: &mut TextFieldState, before: usize, after: usize) -> bool {
    let mut changed = false;
    for _ in 0..before {
        changed |= field.apply(TextFieldInput::Backspace);
    }
    for _ in 0..after {
        changed |= field.apply(TextFieldInput::Delete);
    }
    changed
}

/// Installs the IME cursor-area sync and composition routing.
#[derive(Resource, Default)]
struct CompositionTarget(Option<Entity>);

pub struct ShellImePlugin;
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShellImeSet {
    Composition,
}

impl Plugin for ShellImePlugin {
    fn build(&self, app: &mut App) {
        // The headless composition needs the message channel registered; a
        // windowed one already has it via `WindowPlugin` (registration is
        // idempotent).
        app.add_message::<Ime>()
            .add_message::<NativeTextInput>()
            .init_resource::<InputFocus>()
            .init_resource::<UiScale>()
            .init_resource::<ImeHostReport>()
            .init_resource::<ShellImeComposition>()
            .init_resource::<NativeImeSession>()
            .init_resource::<CompositionTarget>()
            // BANDROID-007/008: the native text-input host seams (IME
            // composition and system clipboard paste) share this shell install
            // point, so headless compositions see the same ports as the host.
            .add_plugins(ClipboardHostPlugin::default())
            // The widget layer recomputes the caret rect after a preedit
            // change, so the pipeline is: route → widget caret → window plan.
            .add_systems(
                Update,
                (route_ime_composition, route_native_text_input)
                    .chain()
                    .in_set(ShellImeSet::Composition)
                    .before(sync_ime_cursor_areas),
            )
            .add_systems(Update, sync_window_ime.after(sync_ime_cursor_areas))
            .add_systems(
                PostUpdate,
                report_native_ime.after(ImeSystems::UpdatePosition),
            );
    }
}

/// Feed the focused field the native adapter's typed events through the shared
/// tracker, and record whether OS back consumed the event (BANDROID-007).
fn route_native_text_input(
    mut events: MessageReader<NativeTextInput>,
    mut session: ResMut<NativeImeSession>,
    mut tracker: ResMut<ShellImeComposition>,
    mut fields: Query<(&TextFieldFocused, &mut TextField)>,
) {
    for event in events.read() {
        let Some((_, mut field)) = fields.iter_mut().find(|(focused, _)| focused.0) else {
            continue;
        };
        apply_native_text_input(&mut session, &mut tracker.0, event.clone(), &mut field.0);
    }
}

/// Feed every IME message into the shared tracker and the focused field.
fn route_ime_composition(
    mut events: MessageReader<Ime>,
    mut tracker: ResMut<ShellImeComposition>,
    mut owner: ResMut<CompositionTarget>,
    mut fields: Query<&mut TextField>,
    focused: Query<(Entity, &TextFieldFocused)>,
    primary: Query<Entity, With<PrimaryWindow>>,
) {
    let primary = primary.single().ok();
    let target = focused
        .iter()
        .find(|(_, focused)| focused.0)
        .map(|(entity, _)| entity);
    if owner.0 != target {
        let action = tracker.0.apply(ImeCompositionEvent::Closed);
        if let Some(previous) = owner.0
            && let Ok(mut field) = fields.get_mut(previous)
        {
            apply_composition_action(&mut field.0, action);
        }
        owner.0 = target;
    }
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
    session: Res<NativeImeSession>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    fields: Query<(Entity, &TextFieldFocused, &ImeCursorArea)>,
    sdk: Res<InputFocus>,
    native: Query<&EditableText, With<MultilineEditor>>,
) {
    if native_editor_active(&sdk, &native) {
        return;
    }
    let focused_field = fields
        .iter()
        .find(|(_, focused, _)| focused.0)
        .map(|(entity, _, _)| entity);
    if session.is_ime_closed_by_back() {
        // OS back closed the keyboard first; keep the OS IME disabled until a
        // new composition or field focus reopens it.
        if let Ok((window_entity, mut window)) = windows.single_mut() {
            if window.ime_enabled {
                window.ime_enabled = false;
            }
            report.window = Some(window_entity);
        } else {
            report.window = None;
        }
        report.focused_field = focused_field;
        report.plan = ImeFocusPlan::DISABLED;
        return;
    }
    let target = fields
        .iter()
        .find(|(_, focused, _)| focused.0)
        .map(|(entity, _, area)| (entity, cursor_rect(area)));
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
