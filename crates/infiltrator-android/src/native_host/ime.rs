//! BANDROID-007: typed native text-input (IME) adapter and its toolchain
//! support declaration.
//!
//! The locked Android toolchain cannot currently deliver composition events
//! into the Bevy surface, so this module declares the typed unsupported
//! boundary instead of faking an IME. The normalization below is the real
//! seam a native `InputConnection`/`GameTextInput` bridge would feed once the
//! toolchain exposes one; it is unit-tested here so the mapping is not a
//! placeholder.
//!
//! Blocker (verified against the locked sources): `winit` 0.30.13's Android
//! backend (`src/platform_impl/android/mod.rs`) matches only `MotionEvent` and
//! `KeyEvent`, stubs `set_ime_cursor_area` to a no-op and never emits
//! `WindowEvent::Ime`; `android-activity` 0.6.1 is compiled with the
//! `native-activity` feature only (the `game-activity`/`GameTextInput` feature
//! that would provide a text-input channel is not enabled in `Cargo.lock`),
//! and `bevy_android` 0.20 depends on `android-activity` without enabling it.
//! A `NativeActivity` therefore has no `InputConnection` composition path into
//! the Bevy UI.

use infiltrator_contract::ime::{
    ImeCompositionAction, ImeCompositionEvent, ImeCompositionTracker, ImeCursorSupport,
};

/// The exact reason the native IME path is unavailable on this toolchain.
pub const ANDROID_IME_UNSUPPORTED_REASON: &str = "winit 0.30.13 Android backend maps only MotionEvent/KeyEvent and stubs set_ime_cursor_area, so no WindowEvent::Ime is emitted; android-activity 0.6.1 is built with native-activity only (game-activity/GameTextInput disabled) and bevy_android 0.20 does not enable it, so no InputConnection composition reaches the Bevy UI";

/// What this Android host can honestly do with the OS IME.
pub const fn android_ime_support() -> ImeCursorSupport {
    ImeCursorSupport::Unsupported {
        reason: ANDROID_IME_UNSUPPORTED_REASON,
    }
}

/// One normalized native text-input event, as an `InputConnection` or
/// `GameTextInput` bridge would report it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeTextEvent {
    /// A composition session opened.
    CompositionStarted,
    /// The marked/composing text changed.
    Composing(String),
    /// The composition committed `text`.
    Commit(String),
    /// The composition was finished/cancelled without a commit.
    CompositionFinished,
    /// The selection changed (surface-local; not a composition transition).
    Selection { start: i32, end: i32 },
    /// Surrounding text should be deleted (surface-local).
    DeleteSurrounding { before: i32, after: i32 },
}

/// The typed action a surface applies for one [`NativeTextEvent`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeTextAction {
    /// A shared composition transition the focused field applies.
    Composition(ImeCompositionAction),
    /// A selection change for the focused field.
    Selection { start: i32, end: i32 },
    /// A surrounding-text deletion request.
    DeleteSurrounding { before: i32, after: i32 },
}

/// Normalizes native text events into the shared IME vocabulary and tracks the
/// live composition phase. This is the typed port the platform adapter feeds;
/// it holds no toolkit or JNI types.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeTextInputBridge {
    tracker: ImeCompositionTracker,
}

impl NativeTextInputBridge {
    pub const fn new() -> Self {
        Self {
            tracker: ImeCompositionTracker::new(),
        }
    }

    /// Apply one native event, returning the typed action.
    pub fn apply(&mut self, event: NativeTextEvent) -> NativeTextAction {
        match event {
            NativeTextEvent::CompositionStarted => {
                NativeTextAction::Composition(self.tracker.apply(ImeCompositionEvent::Opened))
            }
            NativeTextEvent::Composing(text) => NativeTextAction::Composition(
                self.tracker.apply(ImeCompositionEvent::Preedit(text)),
            ),
            NativeTextEvent::Commit(text) => {
                NativeTextAction::Composition(self.tracker.apply(ImeCompositionEvent::Commit(text)))
            }
            NativeTextEvent::CompositionFinished => {
                NativeTextAction::Composition(self.tracker.apply(ImeCompositionEvent::Closed))
            }
            NativeTextEvent::Selection { start, end } => NativeTextAction::Selection { start, end },
            NativeTextEvent::DeleteSurrounding { before, after } => {
                NativeTextAction::DeleteSurrounding { before, after }
            }
        }
    }

    /// Whether a composition is in progress.
    pub fn is_composing(&self) -> bool {
        self.tracker.is_composing()
    }

    /// The live composition string (`""` when idle).
    pub fn preedit(&self) -> &str {
        self.tracker.preedit()
    }

    /// Drop any session state (e.g. the focused field disappeared).
    pub fn reset(&mut self) {
        self.tracker.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_native_ime_path_is_declared_unsupported_with_the_exact_blocker() {
        let support = android_ime_support();
        assert!(!support.is_hosted());
        assert_eq!(support.source(), None);
        assert_eq!(support.reason(), Some(ANDROID_IME_UNSUPPORTED_REASON));
        assert!(ANDROID_IME_UNSUPPORTED_REASON.contains("winit 0.30.13"));
        assert!(ANDROID_IME_UNSUPPORTED_REASON.contains("native-activity"));
    }

    #[test]
    fn a_chinese_composition_maps_through_the_shared_transitions() {
        let mut bridge = NativeTextInputBridge::new();
        assert_eq!(
            bridge.apply(NativeTextEvent::CompositionStarted),
            NativeTextAction::Composition(ImeCompositionAction::Begin)
        );
        assert!(bridge.is_composing());

        assert_eq!(
            bridge.apply(NativeTextEvent::Composing("ni hao".to_owned())),
            NativeTextAction::Composition(ImeCompositionAction::UpdatePreedit("ni hao".to_owned()))
        );
        assert_eq!(bridge.preedit(), "ni hao");

        assert_eq!(
            bridge.apply(NativeTextEvent::Commit("你好".to_owned())),
            NativeTextAction::Composition(ImeCompositionAction::Commit("你好".to_owned()))
        );
        assert!(!bridge.is_composing());
        assert_eq!(bridge.preedit(), "");
    }

    #[test]
    fn a_cancelled_composition_rolls_back() {
        let mut bridge = NativeTextInputBridge::new();
        bridge.apply(NativeTextEvent::CompositionStarted);
        bridge.apply(NativeTextEvent::Composing("hao".to_owned()));
        assert_eq!(
            bridge.apply(NativeTextEvent::CompositionFinished),
            NativeTextAction::Composition(ImeCompositionAction::Cancel)
        );
        assert!(!bridge.is_composing());

        bridge.apply(NativeTextEvent::Composing("stale".to_owned()));
        bridge.reset();
        assert!(!bridge.is_composing());
        assert_eq!(bridge.preedit(), "");
    }

    #[test]
    fn selection_and_deletion_stay_surface_local() {
        let mut bridge = NativeTextInputBridge::new();
        assert_eq!(
            bridge.apply(NativeTextEvent::Selection { start: 1, end: 4 }),
            NativeTextAction::Selection { start: 1, end: 4 }
        );
        assert_eq!(
            bridge.apply(NativeTextEvent::DeleteSurrounding {
                before: 2,
                after: 0
            }),
            NativeTextAction::DeleteSurrounding {
                before: 2,
                after: 0
            }
        );
        assert!(!bridge.is_composing());
    }
}
