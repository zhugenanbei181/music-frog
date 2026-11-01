//! Typed focus, cadence, and input method (IME) model for the Slint surface.
//!
//! Slint surfaces consume the shared typed contracts from
//! [`infiltrator_contract::ime`] and [`infiltrator_contract::cadence`]:
//! - Window focus tracks whether the window is focused and drives the
//!   shared [`RenderCadence`] (60 FPS Active vs 2 FPS Background).
//! - IME composition sessions track the multi-phase Chinese input lifecycle
//!   ([`ImePhase::Idle`] vs [`ImePhase::Composing`]) via [`ImeCompositionTracker`].
//! - During an active composition session, keyboard events belong to the IME
//!   and must never trigger global shortcut chords (e.g. `Ctrl+K`, `Ctrl+Alt+P`).
//! - Tab and Shift-Tab drive focus cycling across focusable elements.
//! - Escape and Return/Enter map to modal dismissal and action activation.

use infiltrator_contract::cadence::RenderCadence;
use infiltrator_contract::ime::ImeCompositionAction;
use infiltrator_contract::ime::ImeCompositionEvent;
use infiltrator_contract::ime::ImeCompositionTracker;
use infiltrator_contract::ime::ImeCursorRect;
use infiltrator_contract::ime::ImeCursorSource;
use infiltrator_contract::ime::ImeCursorSupport;
use infiltrator_contract::ime::ImeFocusPlan;
use infiltrator_contract::ime::ImePhase;
use infiltrator_contract::ime::ImeViewport;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_contract::shortcuts::ShortcutAction;
use infiltrator_contract::shortcuts::ShortcutChord;
use infiltrator_contract::shortcuts::ShortcutRegistry;

/// Reports the IME cursor support strategy for the Slint toolkit.
///
/// Slint text input widgets manage their own internal cursor and caret area,
/// so the strategy is [`ImeCursorSource::ToolkitProvided`].
pub fn cursor_support() -> ImeCursorSupport {
    ImeCursorSupport::Hosted {
        source: ImeCursorSource::ToolkitProvided,
    }
}

/// Window focus state and cadence calculation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlintWindowFocus {
    focused: bool,
}

impl Default for SlintWindowFocus {
    fn default() -> Self {
        Self { focused: true }
    }
}

impl SlintWindowFocus {
    /// Creates a window focus tracker with an initial state.
    pub const fn new(focused: bool) -> Self {
        Self { focused }
    }

    /// Whether the window is currently focused.
    pub const fn is_focused(self) -> bool {
        self.focused
    }

    /// Updates the focus state and returns the corresponding render cadence.
    pub fn set_focused(&mut self, focused: bool) -> RenderCadence {
        self.focused = focused;
        self.cadence()
    }

    /// The render cadence required for the current focus state.
    pub fn cadence(self) -> RenderCadence {
        RenderCadence::from_focused(self.focused)
    }
}

/// Typed IME composition session for Slint input fields.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SlintImeSession {
    tracker: ImeCompositionTracker,
    preedit: String,
    committed: Option<String>,
    cursor_area: Option<ImeCursorRect>,
}

impl SlintImeSession {
    /// Creates a fresh idle IME session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current phase of the composition session.
    pub const fn phase(&self) -> ImePhase {
        self.tracker.phase()
    }

    /// Whether an active IME composition is in progress.
    pub fn is_composing(&self) -> bool {
        self.tracker.is_composing()
    }

    /// The current preedit text buffer.
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Takes the most recently committed text, if any.
    pub fn take_committed(&mut self) -> Option<String> {
        self.committed.take()
    }

    /// Sets the active caret position for candidate window placement.
    pub fn set_cursor_rect(&mut self, rect: Option<ImeCursorRect>) {
        self.cursor_area = rect;
    }

    /// Computes the focus plan for the given viewport.
    pub fn focus_plan(&self, viewport: ImeViewport) -> ImeFocusPlan {
        ImeFocusPlan::from_focused_cursor(viewport, self.cursor_area)
    }

    /// Applies an IME composition event and returns the resulting action.
    pub fn apply_event(&mut self, event: ImeCompositionEvent) -> ImeCompositionAction {
        match &event {
            ImeCompositionEvent::Opened => {
                self.preedit.clear();
                self.committed = None;
            }
            ImeCompositionEvent::Preedit(text) => {
                self.preedit = text.clone();
            }
            ImeCompositionEvent::Commit(text) => {
                self.preedit.clear();
                self.committed = Some(text.clone());
            }
            ImeCompositionEvent::Closed => {
                self.preedit.clear();
            }
        }
        self.tracker.apply(event)
    }

    /// Resets the session to idle.
    pub fn reset(&mut self) {
        self.tracker.reset();
        self.preedit.clear();
        self.committed = None;
        self.cursor_area = None;
    }
}

/// The outcome of dispatching a key event through the Slint keyboard model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyDispatchOutcome {
    /// Consumed by an active IME composition session; global chords suppressed.
    ConsumedByIme,
    /// Tab or Shift-Tab focus navigation.
    FocusCycle { reverse: bool, next_index: usize },
    /// Escape key pressed (cancels active modal, sheet, or draft).
    Escape,
    /// Enter or Return pressed (activates focused control or submits).
    Enter,
    /// Matched a registered global shortcut action.
    Shortcut(ShortcutAction),
    /// Unhandled regular key stroke.
    Unhandled,
}

/// Slint keyboard, focus cycle, and shortcut dispatcher.
#[derive(Clone, Debug)]
pub struct SlintKeyboardDispatcher {
    window_focus: SlintWindowFocus,
    ime: SlintImeSession,
    shortcuts: ShortcutRegistry,
    current_focus_index: usize,
    focusable_count: usize,
}

impl Default for SlintKeyboardDispatcher {
    fn default() -> Self {
        Self {
            window_focus: SlintWindowFocus::default(),
            ime: SlintImeSession::default(),
            shortcuts: ShortcutRegistry::with_defaults(),
            current_focus_index: 0,
            focusable_count: 0,
        }
    }
}

impl SlintKeyboardDispatcher {
    /// Creates a dispatcher with default product shortcut bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current window focus tracker.
    pub const fn window_focus(&self) -> SlintWindowFocus {
        self.window_focus
    }

    /// Updates window focus and returns the corresponding cadence.
    pub fn set_window_focus(&mut self, focused: bool) -> RenderCadence {
        self.window_focus.set_focused(focused)
    }

    /// The current IME session.
    pub const fn ime(&self) -> &SlintImeSession {
        &self.ime
    }

    /// Mutable access to the IME session.
    pub fn ime_mut(&mut self) -> &mut SlintImeSession {
        &mut self.ime
    }

    /// Sets the number of focusable elements in the current page/dialog.
    pub fn set_focusable_count(&mut self, count: usize) {
        self.focusable_count = count;
        if count == 0 {
            self.current_focus_index = 0;
        } else if self.current_focus_index >= count {
            self.current_focus_index = count - 1;
        }
    }

    /// The current focused element index.
    pub const fn current_focus_index(&self) -> usize {
        self.current_focus_index
    }

    /// Sets the focused element index directly.
    pub fn set_focus_index(&mut self, index: usize) {
        if self.focusable_count > 0 {
            self.current_focus_index = index % self.focusable_count;
        } else {
            self.current_focus_index = 0;
        }
    }

    /// Dispatches a key event, taking into account IME composition guarding,
    /// Tab focus navigation, Esc/Enter semantics, and registered shortcuts.
    pub fn dispatch_key(&mut self, key: &str, modifiers: KeyModifiers) -> KeyDispatchOutcome {
        // Red-line: an active composition session owns all keystrokes.
        // Global shortcut chords must never fire during IME composition.
        if self.ime.is_composing() {
            return KeyDispatchOutcome::ConsumedByIme;
        }

        if key == "Tab" {
            let next_index = if self.focusable_count == 0 {
                0
            } else if modifiers.shift {
                if self.current_focus_index == 0 {
                    self.focusable_count - 1
                } else {
                    self.current_focus_index - 1
                }
            } else {
                (self.current_focus_index + 1) % self.focusable_count
            };
            self.current_focus_index = next_index;
            return KeyDispatchOutcome::FocusCycle {
                reverse: modifiers.shift,
                next_index,
            };
        }

        if key == "Escape" {
            return KeyDispatchOutcome::Escape;
        }

        if key == "Return" || key == "Enter" {
            return KeyDispatchOutcome::Enter;
        }

        let chord = ShortcutChord::new(key, modifiers);
        if let Some(action) = self.shortcuts.resolve(&chord) {
            return KeyDispatchOutcome::Shortcut(action);
        }

        KeyDispatchOutcome::Unhandled
    }
}
