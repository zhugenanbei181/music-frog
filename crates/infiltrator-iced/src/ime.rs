//! Iced-side IME boundary (DUAL-15-11).
//!
//! The toolkit owns more of the Chinese-input path than the shell does, and
//! the honest division of labour is:
//!
//! * **enable + cursor area**: `iced::widget::text_input` implements the
//!   toolkit input-method strategy (`InputMethod::Enabled { cursor, purpose,
//!   preedit }`), the runtime merges it into the shell strategy during a
//!   redraw, and iced_winit forwards it to the real window (`set_ime_allowed`
//!   / `set_ime_cursor_area` / `set_ime_purpose`). The surface's text widgets
//!   are the toolkit's own (`text_input` for single-line fields, `text_editor`
//!   for the document editors), so the candidate box follows the real caret
//!   and the shell must *not* fake a second cursor area — there is no public
//!   API to set one by hand.
//! * **consume**: this surface subscribes to `Event::InputMethod` and feeds
//!   the shared [`ImeCompositionTracker`], so "the user is composing" is one
//!   shared fact. While composing, composing keys are not treated as global
//!   chords (the input method owns the keyboard).
//!
//! Boundaries recorded rather than smoothed over: no candidate-list control,
//! no programmatic cursor area for custom widgets, no AccessKit (labels stay
//! localized text; see `crate::accessibility`), and a headless test cannot
//! observe the OS candidate window — it pins the toolkit strategy seam instead.

use iced::advanced::input_method;
use infiltrator_contract::ime::{ImeCompositionEvent, ImeCursorSource, ImeCursorSupport};

use crate::types::message::Message;

/// Where this surface gets the caret rectangle the OS IME needs: the toolkit's
/// own text widget publishes it.
pub const fn cursor_support() -> ImeCursorSupport {
    ImeCursorSupport::Hosted {
        source: ImeCursorSource::ToolkitProvided,
    }
}

/// Map one raw toolkit IME event onto the shared composition vocabulary.
///
/// The byte range and the preedit text size stay toolkit-local: the shared
/// tracker only needs the composition string so both surfaces can answer
/// "is the user composing, and with what?".
pub fn composition_event(event: &input_method::Event) -> Option<ImeCompositionEvent> {
    match event {
        input_method::Event::Opened => Some(ImeCompositionEvent::Opened),
        input_method::Event::Preedit(content, _) => {
            Some(ImeCompositionEvent::Preedit(content.clone()))
        }
        input_method::Event::Commit(text) => Some(ImeCompositionEvent::Commit(text.clone())),
        input_method::Event::Closed => Some(ImeCompositionEvent::Closed),
    }
}

/// The shell message for one raw toolkit event, when it is an IME event.
pub fn composition_message(event: &iced::Event) -> Option<Message> {
    let iced::Event::InputMethod(event) = event else {
        return None;
    };
    composition_event(event).map(Message::ImeComposition)
}
