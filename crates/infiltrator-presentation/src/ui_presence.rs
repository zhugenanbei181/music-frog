//! The presentation lifecycle and the composable interaction state
//! (EFFECTS-007 #26~#28).
//!
//! A page that opens a drawer, a modal or a row must know three things: is
//! the thing on screen, is it accepting input right now, and which
//! interaction facts are true. The first two are a *phase* — one typed value,
//! never a pair of booleans — and the third is a *set of independent facts*,
//! which compose freely and therefore belong in a bit set.
//!
//! The rule that matters: a thing that is on its way out must not fire a
//! command. A user who clicks a drawer while it closes meant to click the
//! page behind it, and the page behind it is not there yet.

/// The lifecycle phase of one presented thing (EFFECTS-007 #26).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiPresence {
    /// The thing is not presented at all.
    #[default]
    Absent,
    /// The thing is entering: the opening transition runs.
    Entering,
    /// The thing is fully presented and interactive.
    Visible,
    /// The thing is leaving: the closing transition runs.
    Exiting,
}

impl UiPresence {
    /// Every phase of the lifecycle.
    pub const ALL: [Self; 4] = [Self::Absent, Self::Entering, Self::Visible, Self::Exiting];

    /// Whether the thing is on screen in any phase.
    pub const fn is_presented(self) -> bool {
        !matches!(self, Self::Absent)
    }

    /// Whether the thing accepts input right now.
    ///
    /// Only a settled thing accepts input: a thing that is entering or
    /// leaving is mid-transition and must not answer a click.
    pub const fn accepts_interaction(self) -> bool {
        matches!(self, Self::Visible)
    }

    /// Whether a transition is running.
    pub const fn is_transitioning(self) -> bool {
        matches!(self, Self::Entering | Self::Exiting)
    }
}

/// The presence of one presented thing, with the progress of its transition.
///
/// The phase is the lifecycle; the progress is how far the transition got.
/// The progress is a real scalar fact (how much of the opening is on
/// screen), not a second phase, so the two never contradict each other.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PresenceState {
    phase: UiPresence,
    progress: f32,
}

impl PresenceState {
    /// A thing that is not presented.
    pub const fn new() -> Self {
        Self {
            phase: UiPresence::Absent,
            progress: 0.0,
        }
    }

    /// A thing that is fully presented.
    pub const fn visible() -> Self {
        Self {
            phase: UiPresence::Visible,
            progress: 1.0,
        }
    }

    /// The current phase of the lifecycle.
    pub const fn phase(&self) -> UiPresence {
        self.phase
    }

    /// How much of the thing is on screen, in 0.0..=1.0.
    pub const fn progress(&self) -> f32 {
        self.progress
    }

    /// Whether the thing accepts input right now.
    pub const fn accepts_interaction(&self) -> bool {
        self.phase.accepts_interaction()
    }

    /// Open the thing.
    ///
    /// Opening a thing that is on its way out does not snap it back to
    /// [`UiPresence::Visible`]: it turns the closing transition around and
    /// keeps the progress, so the motion continues from where it was instead
    /// of restarting.
    pub fn open(&mut self) {
        match self.phase {
            UiPresence::Absent | UiPresence::Exiting => {
                self.phase = UiPresence::Entering;
            }
            UiPresence::Entering | UiPresence::Visible => {}
        }
    }

    /// Close the thing.
    ///
    /// Closing a thing that is still opening turns the opening transition
    /// around and keeps the progress, so a quick open-close does not restart
    /// the motion.
    pub fn close(&mut self) {
        match self.phase {
            UiPresence::Visible | UiPresence::Entering => {
                self.phase = UiPresence::Exiting;
            }
            UiPresence::Absent | UiPresence::Exiting => {}
        }
    }

    /// Advance the transition by `delta` and report whether it settled.
    pub fn advance(&mut self, delta: f32) -> bool {
        if !delta.is_finite() || delta <= 0.0 {
            return false;
        }
        match self.phase {
            UiPresence::Entering => {
                self.progress = (self.progress + delta).min(1.0);
                if self.progress >= 1.0 {
                    self.phase = UiPresence::Visible;
                    return true;
                }
            }
            UiPresence::Exiting => {
                self.progress = (self.progress - delta).max(0.0);
                if self.progress <= 0.0 {
                    self.phase = UiPresence::Absent;
                    return true;
                }
            }
            UiPresence::Absent | UiPresence::Visible => {}
        }
        false
    }

    /// Finish the running transition right now.
    pub fn settle(&mut self) {
        match self.phase {
            UiPresence::Entering => {
                self.phase = UiPresence::Visible;
                self.progress = 1.0;
            }
            UiPresence::Exiting => {
                self.phase = UiPresence::Absent;
                self.progress = 0.0;
            }
            UiPresence::Absent | UiPresence::Visible => {}
        }
    }
}

/// The composable interaction state of one widget (EFFECTS-007 #28).
///
/// Hover, press, focus, selection and disabled are independent facts: a
/// focused button can be hovered, a selected row can be focused, and none of
/// them implies another. They therefore compose in a bit set instead of
/// standing in for a phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InteractionBits(u16);

impl InteractionBits {
    /// No interaction fact holds.
    pub const EMPTY: Self = Self(0);
    /// The pointer is over the widget.
    pub const HOVER: Self = Self(1 << 0);
    /// The pointer is pressing the widget.
    pub const PRESS: Self = Self(1 << 1);
    /// The widget holds the keyboard focus.
    pub const FOCUS: Self = Self(1 << 2);
    /// The widget is selected.
    pub const SELECT: Self = Self(1 << 3);
    /// The widget is disabled.
    pub const DISABLED: Self = Self(1 << 4);

    /// Every interaction fact the product knows.
    pub const ALL: [Self; 5] = [
        Self::HOVER,
        Self::PRESS,
        Self::FOCUS,
        Self::SELECT,
        Self::DISABLED,
    ];

    /// Whether every fact of `other` holds.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// These facts plus the facts of `other`.
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// These facts without the facts of `other`.
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Whether no fact holds.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether the pointer is over the widget.
    pub const fn is_hovered(self) -> bool {
        self.contains(Self::HOVER)
    }

    /// Whether the pointer is pressing the widget.
    pub const fn is_pressed(self) -> bool {
        self.contains(Self::PRESS)
    }

    /// Whether the widget holds the keyboard focus.
    pub const fn is_focused(self) -> bool {
        self.contains(Self::FOCUS)
    }

    /// Whether the widget is selected.
    pub const fn is_selected(self) -> bool {
        self.contains(Self::SELECT)
    }

    /// Whether the widget is disabled.
    pub const fn is_disabled(self) -> bool {
        self.contains(Self::DISABLED)
    }

    /// Whether a command may be dispatched for this state.
    pub const fn accepts_command(self) -> bool {
        !self.contains(Self::DISABLED)
    }
}

/// The interaction state of one presented widget.
///
/// The two facts of the widget — how far its lifecycle got and which
/// interaction facts hold — meet in one place, so the rule "a thing that is
/// leaving must not fire a command" is stated exactly once.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WidgetInteraction {
    presence: PresenceState,
    bits: InteractionBits,
}

impl WidgetInteraction {
    /// A widget that is not presented and has no interaction fact.
    pub const fn new() -> Self {
        Self {
            presence: PresenceState::new(),
            bits: InteractionBits::EMPTY,
        }
    }

    /// The lifecycle of the widget.
    pub const fn presence(&self) -> &PresenceState {
        &self.presence
    }

    /// The lifecycle of the widget, for the fold that changes it.
    pub fn presence_mut(&mut self) -> &mut PresenceState {
        &mut self.presence
    }

    /// The interaction facts of the widget.
    pub const fn bits(&self) -> InteractionBits {
        self.bits
    }

    /// The interaction facts of the widget, for the fold that changes them.
    pub fn bits_mut(&mut self) -> &mut InteractionBits {
        &mut self.bits
    }

    /// Whether a command may be dispatched for this widget right now.
    pub const fn accepts_command(&self) -> bool {
        self.presence.accepts_interaction() && self.bits.accepts_command()
    }
}

/// The focus stack of the open layers (EFFECTS-013 #49).
///
/// A drawer that opens on top of another drawer takes the focus; closing it
/// hands the focus back to the layer below. The stack is the authority: a
/// surface never keeps "the focused drawer" next to "the drawer that is
/// open", because those two can disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusStack<Id> {
    entries: Vec<Id>,
}

impl<Id> Default for FocusStack<Id> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<Id> FocusStack<Id> {
    /// An empty stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the focus.
    pub fn push(&mut self, id: Id) {
        self.entries.push(id);
    }

    /// Hand the focus back and return the layer that held it.
    pub fn pop(&mut self) -> Option<Id> {
        self.entries.pop()
    }

    /// Drop one layer from the stack, wherever it sits.
    pub fn remove(&mut self, id: &Id) -> bool
    where
        Id: PartialEq,
    {
        let before = self.entries.len();
        self.entries.retain(|entry| entry != id);
        self.entries.len() != before
    }

    /// The layer that holds the focus.
    pub fn current(&self) -> Option<&Id> {
        self.entries.last()
    }

    /// How many layers are on the stack.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether the layer is on the stack.
    pub fn contains(&self, id: &Id) -> bool
    where
        Id: PartialEq,
    {
        self.entries.iter().any(|entry| entry == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lifecycle_is_one_phase_and_never_a_flag_pair() {
        let mut presence = PresenceState::new();
        assert_eq!(presence.phase(), UiPresence::Absent);
        assert!(!presence.accepts_interaction());

        presence.open();
        assert_eq!(presence.phase(), UiPresence::Entering);
        assert!(
            !presence.accepts_interaction(),
            "a thing that is entering is not settled"
        );

        presence.advance(1.0);
        assert_eq!(presence.phase(), UiPresence::Visible);
        assert_eq!(presence.progress(), 1.0);
        assert!(presence.accepts_interaction());

        presence.close();
        assert_eq!(presence.phase(), UiPresence::Exiting);
        assert!(
            !presence.accepts_interaction(),
            "a thing that is leaving must not accept input"
        );

        presence.advance(1.0);
        assert_eq!(presence.phase(), UiPresence::Absent);
        assert_eq!(presence.progress(), 0.0);
    }

    #[test]
    fn a_closing_thing_does_not_fire_a_command() {
        let mut widget = WidgetInteraction::new();
        widget.presence_mut().open();
        widget.presence_mut().advance(1.0);
        *widget.bits_mut() = InteractionBits::HOVER.with(InteractionBits::FOCUS);
        assert!(widget.accepts_command(), "a settled widget answers a click");

        widget.presence_mut().close();
        assert!(
            !widget.accepts_command(),
            "a closing widget must not answer a click"
        );
    }

    #[test]
    fn a_disabled_widget_does_not_fire_a_command() {
        let mut widget = WidgetInteraction::new();
        *widget.presence_mut() = PresenceState::visible();
        *widget.bits_mut() = InteractionBits::HOVER.with(InteractionBits::DISABLED);
        assert!(!widget.accepts_command());
    }

    #[test]
    fn reopening_a_closing_thing_continues_the_motion() {
        let mut presence = PresenceState::new();
        presence.open();
        presence.advance(0.6);
        assert_eq!(presence.progress(), 0.6);

        presence.close();
        presence.advance(0.2);
        assert_eq!(presence.phase(), UiPresence::Exiting);
        assert_eq!(presence.progress(), 0.4);

        // Reopening turns the transition around instead of restarting it.
        presence.open();
        assert_eq!(presence.phase(), UiPresence::Entering);
        assert_eq!(
            presence.progress(),
            0.4,
            "the reversed motion continues from where it was"
        );
        presence.advance(0.6);
        assert_eq!(presence.phase(), UiPresence::Visible);
        assert_eq!(presence.progress(), 1.0);
    }

    #[test]
    fn the_interaction_facts_compose_without_implying_each_other() {
        let bits = InteractionBits::HOVER.with(InteractionBits::FOCUS);
        assert!(bits.is_hovered());
        assert!(bits.is_focused());
        assert!(!bits.is_pressed());
        assert!(!bits.is_selected());
        assert!(!bits.is_disabled());
        assert!(bits.contains(InteractionBits::HOVER));

        let pressed = bits.with(InteractionBits::PRESS);
        assert!(pressed.is_hovered() && pressed.is_pressed());
        let released = pressed.without(InteractionBits::PRESS);
        assert!(!released.is_pressed());
        assert!(released.is_hovered());
        assert!(InteractionBits::EMPTY.is_empty());
        for fact in InteractionBits::ALL {
            assert!(fact.with(InteractionBits::EMPTY) == fact);
        }
    }

    #[test]
    fn the_presence_settles_only_in_its_own_direction() {
        let mut presence = PresenceState::new();
        assert!(!presence.advance(0.5), "an absent thing never animates");
        assert!(!presence.advance(-1.0));
        assert_eq!(presence.phase(), UiPresence::Absent);

        presence.open();
        presence.settle();
        assert_eq!(presence.phase(), UiPresence::Visible);
        assert_eq!(presence.progress(), 1.0);

        presence.close();
        presence.settle();
        assert_eq!(presence.phase(), UiPresence::Absent);
        assert_eq!(presence.progress(), 0.0);
    }

    #[test]
    fn the_focus_stack_hands_the_focus_back() {
        let mut stack = FocusStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.current(), None);

        stack.push("drawer");
        stack.push("confirm");
        assert_eq!(stack.current(), Some(&"confirm"));
        assert_eq!(stack.len(), 2);
        assert!(stack.contains(&"drawer"));

        assert_eq!(stack.pop(), Some("confirm"));
        assert_eq!(stack.current(), Some(&"drawer"));
        assert!(stack.remove(&"drawer"));
        assert!(stack.is_empty());
        assert_eq!(stack.pop(), None);
    }
}
