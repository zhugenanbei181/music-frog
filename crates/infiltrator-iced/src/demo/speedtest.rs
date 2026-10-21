//! The speedtest surface performs its own progress cancellation and history
//! comparison gestures.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::speedtest::{
    HistoricalSpeedtestRecord, SpeedtestCancelOutcome, SpeedtestProgress, SpeedtestRunId,
    SpeedtestRunState, SpeedtestSnapshot,
};

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
        FeatureId::SpeedtestProgressCancel => activate_progress_cancel(state),
        FeatureId::SpeedtestHistoryComparison => activate_history_comparison(state),
        _ => Task::none(),
    }
}

/// The owner published a running probe, so the surface offers the cancellation
/// control; the user cancels it and the owner accepts the request.
fn activate_progress_cancel(state: &mut AppState) -> Task<Message> {
    let run_id = SpeedtestRunId::new(7).expect("valid run identity");
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    let target = snapshot.state.target().cloned().expect("fixture target");
    let progress = SpeedtestProgress {
        completed_targets: 1,
        total_targets: 3,
    };
    snapshot.state = SpeedtestRunState::Probing {
        run_id,
        target: target.clone(),
        progress: progress.clone(),
    };
    state.diag.speedtest = snapshot;
    // The user cancels the running probe through the surface's own command.
    let cancel = state.update(Message::CancelSpeedtestRun(run_id));
    // The owner accepts the cancellation and publishes its canonical snapshot.
    let mut accepted = state.diag.speedtest.clone();
    accepted.state = SpeedtestRunState::CancelRequested {
        run_id,
        target,
        progress,
    };
    cancel.chain(Task::done(Message::SpeedtestCancelled(Ok((
        SpeedtestCancelOutcome::Requested { run_id },
        accepted,
    )))))
}

/// The owner published several finished runs, so the surface can compare them.
fn activate_history_comparison(state: &mut AppState) -> Task<Message> {
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    let target = snapshot.state.target().cloned().expect("fixture target");
    snapshot.recent_history = vec![
        HistoricalSpeedtestRecord {
            run_id: SpeedtestRunId::new(11).expect("valid run identity"),
            timestamp_epoch_ms: 1_700_000_000_000,
            target: Some(target.clone()),
            total_targets: 3,
            completed_targets: 3,
            avg_latency_ms: Some(50.5),
            avg_jitter_ms: Some(1.5),
        },
        HistoricalSpeedtestRecord {
            run_id: SpeedtestRunId::new(12).expect("valid run identity"),
            timestamp_epoch_ms: 1_700_000_060_000,
            target: Some(target),
            total_targets: 3,
            completed_targets: 2,
            avg_latency_ms: Some(42.0),
            avg_jitter_ms: Some(0.9),
        },
    ];
    state.diag.speedtest = snapshot;
    assert_eq!(
        state.update(Message::OpenSpeedtestDetail).units(),
        0,
        "opening an inspection cannot schedule effects"
    );
    Task::none()
}
