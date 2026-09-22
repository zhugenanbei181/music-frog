//! Shared input-method (IME) grammar for the shell (DUAL-15-11).
//!
//! Chinese input needs two host facts the shell cannot invent: an OS-side
//! *cursor area* the candidate window must avoid, and a *composition
//! lifecycle* (open, preedit, commit, close) the focused field follows. Both
//! surfaces have to deliver them from different places:
//!
//! * Bevy computes the caret rectangle on the widget-layer text field
//!   (`ImeCursorArea`) and writes it to the window
//!   (`Window::ime_position` → winit `set_ime_cursor_area`), so its source is
//!   [`ImeCursorSource::SurfaceComputed`];
//! * Iced's built-in text widgets (`text_input`, `text_editor`) publish the
//!   caret rect through the toolkit input-method strategy
//!   (`InputMethod::Enabled { cursor }` → iced_winit → `set_ime_cursor_area`),
//!   so their source is [`ImeCursorSource::ToolkitProvided`].
//!
//! The shapes here are deliberately toolkit-neutral: one geometry rule
//! ([`ImeCursorRect`] clamped into the window, never degenerate), one
//! single-target rule ([`ImeFocusPlan`]: the focused field's caret or nothing)
//! and one composition vocabulary ([`ImeCompositionEvent`] →
//! [`ImeCompositionAction`]) so both hosts feed their own field state from the
//! same transitions.
//!

/// The smallest caret area an OS IME accepts (a zero-width rect is rejected
/// or falls back to a screen corner on several toolkits).
pub const IME_CURSOR_MIN_SIDE_PX: f32 = 1.0;

/// The rectangle the IME candidate window must stay clear of, in logical
/// pixels relative to the window's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeCursorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ImeCursorRect {
    /// One caret rectangle. Non-finite inputs collapse to `0` and both sides
    /// are raised to [`IME_CURSOR_MIN_SIDE_PX`] so the host never forwards a
    /// degenerate area.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: finite_or_zero(x),
            y: finite_or_zero(y),
            width: finite_or_zero(width).max(IME_CURSOR_MIN_SIDE_PX),
            height: finite_or_zero(height).max(IME_CURSOR_MIN_SIDE_PX),
        }
    }

    /// Whether both sides already meet the minimum area.
    pub fn is_degenerate(self) -> bool {
        self.width < IME_CURSOR_MIN_SIDE_PX || self.height < IME_CURSOR_MIN_SIDE_PX
    }

    /// Move the rectangle inside `viewport` without resizing it.
    ///
    /// The anchor never goes negative (a caret outside the window's top-left
    /// corner is invalid geometry); the upper bound comes from the viewport,
    /// so an unbounded viewport ([`ImeViewport::UNBOUNDED`], the headless
    /// case) only sanitizes the anchor and leaves the rest where the widget
    /// computed it.
    pub fn clamped_into(self, viewport: ImeViewport) -> Self {
        Self {
            x: self.x.clamp(0.0, lower_bound(viewport.width, self.width)),
            y: self.y.clamp(0.0, lower_bound(viewport.height, self.height)),
            ..self
        }
    }

    /// Whether the rectangle is fully inside `viewport` (an unbounded viewport
    /// accepts everything finite).
    pub fn is_inside(self, viewport: ImeViewport) -> bool {
        self.x >= 0.0
            && self.y >= 0.0
            && self.x + self.width <= viewport.width
            && self.y + self.height <= viewport.height
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// The largest anchor position that keeps `side` inside `viewport_side`; an
/// unbounded viewport side imposes no upper bound at all.
fn lower_bound(viewport_side: f32, side: f32) -> f32 {
    if viewport_side.is_finite() {
        (viewport_side - side).max(0.0)
    } else {
        f32::INFINITY
    }
}

/// The window bounds a caret rectangle is kept inside, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeViewport {
    pub width: f32,
    pub height: f32,
}

impl ImeViewport {
    /// No window to clamp against: the surface reports the raw caret.
    pub const UNBOUNDED: Self = Self {
        width: f32::INFINITY,
        height: f32::INFINITY,
    };

    /// A viewport of the given logical size; non-finite sides collapse to `0`.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width: finite_or_zero(width),
            height: finite_or_zero(height),
        }
    }

    /// Whether the window has no usable area (nothing can be positioned).
    pub fn is_degenerate(self) -> bool {
        !(self.width > 0.0 && self.height > 0.0)
    }
}

/// Where the caret rectangle the OS IME needs comes from on this surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImeCursorSource {
    /// The surface computes the caret rect itself and writes it to the host
    /// window (Bevy: `Window::ime_enabled` / `Window::ime_position`).
    SurfaceComputed,
    /// The toolkit's text widget publishes the caret rect through its
    /// input-method strategy (Iced: `InputMethod::Enabled { cursor }`).
    ToolkitProvided,
}

/// What one surface can honestly do with the OS IME cursor area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImeCursorSupport {
    /// The surface reaches a real `set_ime_cursor_area`-equivalent path.
    Hosted {
        /// Whether the caret rect is computed by the surface or by the widget.
        source: ImeCursorSource,
    },
    /// The surface has no cursor-area path at all; it must not fake one.
    Unsupported { reason: &'static str },
}

impl ImeCursorSupport {
    /// Whether this surface hosts the cursor-area path.
    pub const fn is_hosted(self) -> bool {
        matches!(self, Self::Hosted { .. })
    }

    /// The caret source when hosted.
    pub const fn source(self) -> Option<ImeCursorSource> {
        match self {
            Self::Hosted { source } => Some(source),
            Self::Unsupported { .. } => None,
        }
    }

    /// The typed reason when not hosted.
    pub const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Hosted { .. } => None,
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

/// What the OS IME should be told right now.
///
/// Exactly one field owns the IME at a time: when a field with a caret is
/// focused the plan enables the IME at that caret; when nothing is focused the
/// IME is disabled instead of tracking a stale caret.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeFocusPlan {
    pub enabled: bool,
    pub cursor: Option<ImeCursorRect>,
}

impl ImeFocusPlan {
    /// Nothing focused: disable the IME entirely.
    pub const DISABLED: Self = Self {
        enabled: false,
        cursor: None,
    };

    /// The plan for the focused field's caret rect, clamped into the window.
    pub fn from_focused_cursor(viewport: ImeViewport, cursor: Option<ImeCursorRect>) -> Self {
        match cursor {
            Some(cursor) => Self {
                enabled: true,
                cursor: Some(cursor.clamped_into(viewport)),
            },
            None => Self::DISABLED,
        }
    }

    /// Whether the OS IME should be active.
    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    /// The caret the IME should avoid, when enabled.
    pub const fn cursor(self) -> Option<ImeCursorRect> {
        self.cursor
    }
}

/// One toolkit-neutral IME composition event.
///
/// `Preedit` carries the raw composition string (pinyin syllables, marked
/// text); Unicode ranges and candidate lists stay surface-local.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeCompositionEvent {
    /// The IME opened a composition session.
    Opened,
    /// The composition string changed.
    Preedit(String),
    /// The composition committed `text` (may be empty for a cancelled
    /// candidate selection).
    Commit(String),
    /// The IME closed and any uncommitted composition is dropped.
    Closed,
}

/// The one action a surface applies to its focused field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeCompositionAction {
    /// Snapshot the field so a cancel can roll back.
    Begin,
    /// Show `text` as the inline composition.
    UpdatePreedit(String),
    /// Replace the composition with the committed `text`.
    Commit(String),
    /// Drop the composition (rollback when a snapshot exists).
    Cancel,
}

/// The composition phase the shell tracks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImePhase {
    /// No composition in progress.
    #[default]
    Idle,
    /// The IME is composing and a preedit may be visible.
    Composing,
}

/// The shell's toolbar-level view of one IME composition session.
///
/// Both surfaces feed the same events in and apply the returned action to
/// their own field state, so "is the user composing?" has one meaning (the
/// Iced shell refuses to treat composing keys as global chords, Bevy keeps the
/// caret area in sync while composing).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImeCompositionTracker {
    phase: ImePhase,
    preedit: String,
}

impl ImeCompositionTracker {
    /// An idle tracker.
    pub const fn new() -> Self {
        Self {
            phase: ImePhase::Idle,
            preedit: String::new(),
        }
    }

    /// Feed one composition event, returning the field action it implies.
    pub fn apply(&mut self, event: ImeCompositionEvent) -> ImeCompositionAction {
        match event {
            ImeCompositionEvent::Opened => {
                self.phase = ImePhase::Composing;
                ImeCompositionAction::Begin
            }
            ImeCompositionEvent::Preedit(text) => {
                self.phase = ImePhase::Composing;
                self.preedit = text.clone();
                ImeCompositionAction::UpdatePreedit(text)
            }
            ImeCompositionEvent::Commit(text) => {
                self.phase = ImePhase::Idle;
                self.preedit.clear();
                ImeCompositionAction::Commit(text)
            }
            ImeCompositionEvent::Closed => {
                self.phase = ImePhase::Idle;
                self.preedit.clear();
                ImeCompositionAction::Cancel
            }
        }
    }

    /// The current composition phase.
    pub const fn phase(&self) -> ImePhase {
        self.phase
    }

    /// Whether the user is composing right now.
    pub fn is_composing(&self) -> bool {
        self.phase == ImePhase::Composing
    }

    /// The live composition string (`""` when idle).
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Drop any session state (e.g. the focused field disappeared).
    pub fn reset(&mut self) {
        self.phase = ImePhase::Idle;
        self.preedit.clear();
    }

    /// The line a live region should display for `label`: the bare label while
    /// idle, the label plus the live composition while composing.
    pub fn announcement(&self, label: &str) -> String {
        if self.preedit.is_empty() {
            label.to_owned()
        } else {
            format!("{label}: {}", self.preedit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_focused_caret_enables_the_plan_and_no_focus_disables_it() {
        let viewport = ImeViewport::new(800.0, 600.0);
        let caret = ImeCursorRect::new(120.0, 40.0, 2.0, 18.0);
        let plan = ImeFocusPlan::from_focused_cursor(viewport, Some(caret));
        assert!(plan.is_enabled());
        assert_eq!(plan.cursor(), Some(caret));
        assert!(caret.is_inside(viewport));

        let unfocused = ImeFocusPlan::from_focused_cursor(viewport, None);
        assert!(!unfocused.is_enabled());
        assert_eq!(unfocused.cursor(), None);
        assert_eq!(unfocused, ImeFocusPlan::DISABLED);
    }

    #[test]
    fn a_caret_is_clamped_inside_the_window_and_never_degenerate() {
        let viewport = ImeViewport::new(320.0, 200.0);
        let off_screen = ImeCursorRect::new(400.0, -12.0, 2.0, 18.0);
        let clamped = off_screen.clamped_into(viewport);
        assert_eq!(clamped.x, 318.0);
        assert_eq!(clamped.y, 0.0);
        assert!(clamped.is_inside(viewport));

        let zero = ImeCursorRect::new(f32::NAN, 0.0, 0.0, f32::NAN);
        assert_eq!(zero.width, IME_CURSOR_MIN_SIDE_PX);
        assert_eq!(zero.height, IME_CURSOR_MIN_SIDE_PX);
        assert_eq!(zero.x, 0.0);
        assert!(!zero.is_degenerate());

        // An unbounded (headless) viewport has no upper bound; only the
        // invalid negative anchor is sanitized.
        let unbounded = off_screen.clamped_into(ImeViewport::UNBOUNDED);
        assert_eq!(unbounded.x, 400.0, "no window means no upper clamp");
        assert_eq!(unbounded.y, 0.0, "a negative anchor is never forwarded");
        assert!(ImeViewport::new(0.0, 0.0).is_degenerate());
    }

    #[test]
    fn a_composition_session_maps_to_one_action_per_event() {
        let mut tracker = ImeCompositionTracker::new();
        assert!(!tracker.is_composing());
        assert_eq!(tracker.phase(), ImePhase::Idle);

        assert_eq!(
            tracker.apply(ImeCompositionEvent::Opened),
            ImeCompositionAction::Begin
        );
        assert!(tracker.is_composing());
        assert_eq!(tracker.preedit(), "");

        assert_eq!(
            tracker.apply(ImeCompositionEvent::Preedit("ni hao".to_owned())),
            ImeCompositionAction::UpdatePreedit("ni hao".to_owned())
        );
        assert_eq!(tracker.preedit(), "ni hao");
        assert_eq!(tracker.announcement("文本输入框"), "文本输入框: ni hao");

        let mut cancelled = tracker.clone();
        assert_eq!(
            cancelled.apply(ImeCompositionEvent::Closed),
            ImeCompositionAction::Cancel
        );
        assert!(!cancelled.is_composing());
        assert_eq!(cancelled.preedit(), "");
        assert_eq!(cancelled.announcement("文本输入框"), "文本输入框");

        assert_eq!(
            tracker.apply(ImeCompositionEvent::Commit("你好".to_owned())),
            ImeCompositionAction::Commit("你好".to_owned())
        );
        assert!(!tracker.is_composing());
        assert_eq!(tracker.phase(), ImePhase::Idle);

        // A commit without a preceding open still commits, and reset clears.
        let mut late = ImeCompositionTracker::new();
        assert_eq!(
            late.apply(ImeCompositionEvent::Commit("好".to_owned())),
            ImeCompositionAction::Commit("好".to_owned())
        );
        late.reset();
        assert_eq!(late, ImeCompositionTracker::new());
    }

    #[test]
    fn hosts_report_where_their_cursor_area_comes_from() {
        let bevy = ImeCursorSupport::Hosted {
            source: ImeCursorSource::SurfaceComputed,
        };
        assert!(bevy.is_hosted());
        assert_eq!(bevy.source(), Some(ImeCursorSource::SurfaceComputed));
        assert_eq!(bevy.reason(), None);

        let iced = ImeCursorSupport::Hosted {
            source: ImeCursorSource::ToolkitProvided,
        };
        assert_eq!(iced.source(), Some(ImeCursorSource::ToolkitProvided));
        assert_ne!(bevy, iced, "the two surfaces must not claim one source");

        let absent = ImeCursorSupport::Unsupported {
            reason: "host has no ime path",
        };
        assert!(!absent.is_hosted());
        assert_eq!(absent.source(), None);
        assert_eq!(absent.reason(), Some("host has no ime path"));
    }
}
