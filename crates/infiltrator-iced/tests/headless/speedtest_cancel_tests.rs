//! test-intent: behavior
//! The running speedtest control submits the shared typed cancel command for
//! the observed run, and the owner's typed answer lands in the surface state
//! instead of a locally invented success.
use crate::command_harness::recording_application;
use crate::outbound_fixtures::{OutboundFixture, OutboundFixtureInput, standard_members};
use crate::probe_settings_store::ProbeSettingsStore;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::speedtest_application::SpeedtestApplication;
use infiltrator_contract::speedtest::{
    SpeedtestCancelOutcome, SpeedtestProgress, SpeedtestRunId, SpeedtestRunState, SpeedtestRounds,
    SpeedtestSnapshot, SpeedtestTarget,
};
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::message::Message;
use infiltrator_iced::view::overview_hero::speedtest_cancel_message;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::preferences::{
    OutboundHealthFilter, OutboundLayout, OutboundSortOrder,
};
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use std::sync::Arc;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("typed speedtest cancel task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("the cancel command must deliver a terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}

fn owner() -> (infiltrator_application::core_application::CoreApplication, OutboundFixture, CoreScope) {
    let (application, _handler) = recording_application();
    application
        .install_speedtest_application(SpeedtestApplication::new(SettingsApplication::new(Arc::new(
            ProbeSettingsStore::default(),
        ))))
        .expect("the speedtest owner is installed once");
    let scope = application
        .snapshot()
        .scope()
        .expect("identified owner scope")
        .clone();
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 51,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    (application, fixture, scope)
}

/// Publish the owner's running speedtest so the surface shows a real run.
fn publish_running_speedtest(
    state: &mut AppState,
    scope: &CoreScope,
    run_id: SpeedtestRunId,
) {
    let target = SpeedtestTarget::new(
        scope.clone(),
        SnapshotRevision::new(51),
        OutboundProbeBatchScope::All,
        SpeedtestRounds::new(1).expect("valid rounds"),
    )
    .expect("the owner scope carries the probe batch scope");
    let mut surface = state.surface.latest().cloned().expect("owner surface");
    surface.revision = state.surface.revision() + 1;
    surface.speedtest = SpeedtestSnapshot {
        revision: 1,
        state: SpeedtestRunState::Probing {
            run_id,
            target,
            progress: SpeedtestProgress {
                completed_targets: 1,
                total_targets: 4,
            },
        },
        concurrency: Default::default(),
        results: Vec::new(),
        recent_history: Vec::new(),
    };
    assert!(state.apply_shared_surface_snapshot(surface));
}

#[test]
fn the_running_speedtest_control_cancels_the_observed_run_and_lands_the_owner_outcome() {
    let (application, fixture, scope) = owner();
    let (mut state, _) = AppState::new();
    fixture.attach_to(&mut state, &application);
    let run_id = SpeedtestRunId::new(7).expect("valid run id");
    publish_running_speedtest(&mut state, &scope, run_id);

    // The rendered control takes its action from the observed owner snapshot,
    // and an idle speedtest has nothing to cancel.
    assert!(
        matches!(
            speedtest_cancel_message(&state.diag.speedtest),
            Some(Message::CancelSpeedtestRun(requested)) if requested == run_id
        ),
        "the running control cancels exactly the observed run"
    );
    assert!(
        speedtest_cancel_message(&SpeedtestSnapshot::default()).is_none(),
        "an idle speedtest has nothing to cancel"
    );

    // The command reaches the owner and the typed answer lands in the surface.
    let task = state.update(Message::CancelSpeedtestRun(run_id));
    let result = terminal(task);
    let Message::SpeedtestCancelled(Ok((outcome, snapshot))) = result.clone() else {
        panic!("the owner answered with a typed cancellation outcome: {result:?}")
    };
    assert!(
        matches!(
            outcome,
            SpeedtestCancelOutcome::Requested { run_id: requested }
                | SpeedtestCancelOutcome::AlreadyRequested { run_id: requested }
                | SpeedtestCancelOutcome::RunAlreadyFinished { run_id: requested }
                | SpeedtestCancelOutcome::StaleRun {
                    requested_run_id: requested,
                    ..
                } if requested == run_id
        ),
        "the owner answered about the observed run: {outcome:?}"
    );
    let _ = state.update(result);
    assert!(
        !matches!(
            state.diag.speedtest.state,
            SpeedtestRunState::Probing { .. }
        ),
        "the surface no longer shows the run as probing"
    );
    assert!(
        !matches!(snapshot.state, SpeedtestRunState::Probing { .. }),
        "the owner snapshot no longer shows a probing run: {:?}",
        snapshot.state
    );
}
