//! The shell owns the command palette, the keyboard focus cycle and the
//! Fake-IP cache confirmation.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::shortcuts::KeyModifiers;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ShellCommandPalette => {
            assert_eq!(
                state.update(Message::OpenCommandPalette).units(),
                0,
                "opening an inspection cannot schedule effects"
            );
            assert_eq!(
                state.update(Message::SetCommandQuery("dns".into())).units(),
                0
            );
            Task::none()
        }
        FeatureId::ShellKeyboardFocus => activate_keyboard_focus(state),
        FeatureId::DnsFakeIpFlushConfirm => {
            assert_eq!(
                state.update(Message::FlushFakeIpCache).units(),
                0,
                "opening an inspection cannot schedule effects"
            );
            Task::none()
        }
        _ => Task::none(),
    }
}

/// The user opens the palette with the keyboard and moves the focus ring to
/// the next catalogue row with the arrow key the palette documents.
fn activate_keyboard_focus(state: &mut AppState) -> Task<Message> {
    assert_eq!(
        state.update(Message::OpenCommandPalette).units(),
        0,
        "opening the palette cannot schedule effects"
    );
    state.update(Message::KeyboardChord {
        key: "ArrowDown".to_string(),
        modifiers: KeyModifiers::default(),
    })
}
