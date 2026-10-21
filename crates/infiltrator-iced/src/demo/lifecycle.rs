//! The Doctor page owns the lifecycle recovery panel: the crash watchdog's
//! own recovery state, separate from the diagnostic command phase.
use crate::demo::watchdog::attach_watchdog_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::LifecycleRecoveryPanel => {
            // The owner observed the managed core exit, so the shared watchdog
            // published its recovery state. The panel renders that state; the
            // diagnostic command phase stays untouched, so the two certified
            // doctor states cannot fight over the same phase.
            let owner = attach_watchdog_capture_owner(state);
            owner.chain(state.update(Message::CheckCrashWatchdog))
        }
        _ => Task::none(),
    }
}
