//! The shell owns the command palette and the Fake-IP cache confirmation.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    let message = match feature {
        FeatureId::ShellCommandPalette => Message::OpenCommandPalette,
        FeatureId::DnsFakeIpFlushConfirm => Message::FlushFakeIpCache,
        _ => return Task::none(),
    };
    assert_eq!(
        state.update(message).units(),
        0,
        "opening an inspection cannot schedule effects"
    );
    if feature == FeatureId::ShellCommandPalette {
        assert_eq!(
            state.update(Message::SetCommandQuery("dns".into())).units(),
            0
        );
    }
    Task::none()
}
