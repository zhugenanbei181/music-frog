//! The speedtest surface performs its own detail gesture.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::speedtest::SpeedtestSnapshot;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::SpeedtestDetailsModal => {
            state.diag.speedtest = SpeedtestSnapshot::demo_fixture();
            assert_eq!(
                state.update(Message::OpenSpeedtestDetail).units(),
                0,
                "opening an inspection cannot schedule effects"
            );
            Task::none()
        }
        _ => Task::none(),
    }
}
