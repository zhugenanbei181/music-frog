//! Behaviour of the isolated speedtest-history port fixture.

use infiltrator_contract::speedtest::{SpeedtestRunState, SpeedtestSnapshot};
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use infiltrator_test_support::speedtest_history_fixtures::{
    CAPTURE_FIRST_RUN, CAPTURE_SECOND_RUN, SpeedtestHistoryCapture, capture_speedtest_history,
    running_speedtest_snapshot,
};

#[test]
fn the_history_publishes_two_comparable_runs() {
    let store = SpeedtestHistoryCapture::new();

    let loaded = store.load().expect("speedtest history");

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].run_id.get(), CAPTURE_FIRST_RUN);
    assert_eq!(loaded[1].run_id.get(), CAPTURE_SECOND_RUN);
    assert!(
        loaded[0].timestamp_epoch_ms < loaded[1].timestamp_epoch_ms,
        "the history must stay ordered oldest first"
    );
    assert!(loaded[0].target.is_some());
    assert!(loaded[1].target.is_some());
    assert!(
        loaded[0].avg_latency_ms != loaded[1].avg_latency_ms,
        "two comparable runs must carry a real difference"
    );
}

#[test]
fn saving_replaces_the_published_history() {
    let store = SpeedtestHistoryCapture::new();

    store.save(&[]).expect("save history");

    assert!(store.load().expect("speedtest history").is_empty());
}

#[test]
fn an_empty_store_publishes_no_runs() {
    let store = SpeedtestHistoryCapture::empty();

    assert!(store.load().expect("speedtest history").is_empty());
}

#[test]
fn the_running_snapshot_is_a_real_probing_run() {
    let snapshot: SpeedtestSnapshot = running_speedtest_snapshot();

    match &snapshot.state {
        SpeedtestRunState::Probing {
            run_id, progress, ..
        } => {
            assert_eq!(run_id.get(), CAPTURE_SECOND_RUN);
            assert_eq!(progress.completed_targets, 1);
            assert_eq!(progress.total_targets, 2);
        }
        other => panic!("expected a probing run, got {other:?}"),
    }
    assert_eq!(snapshot.recent_history, capture_speedtest_history());
}
