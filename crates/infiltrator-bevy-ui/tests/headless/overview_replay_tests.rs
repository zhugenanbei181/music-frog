//! test-intent: behavior
//! BEVY-036: the Overview traffic chart's bounded time-series store and its
//! time-travel scrubber. Pure behavior over the rate-history owner.

use super::*;

/// The time-series store never grows past its ceiling, and eviction keeps the
/// newest samples with their original values (no fabrication, no drift).
#[test]
fn traffic_store_stays_bounded_under_growth() {
    let mut history = TrafficHistory::default();
    assert_eq!(history.store_len(), 0);
    assert!(history.retained_samples().is_empty());

    for tick in 0..10_000u64 {
        history.push(tick as f64, (tick * 2) as f64);
    }

    assert_eq!(
        history.len(),
        TRAFFIC_HISTORY_CAPACITY,
        "chart ring bounded"
    );
    assert_eq!(
        history.store_len(),
        history.store_capacity(),
        "store sits at its ceiling, not above it"
    );
    assert_eq!(history.store_capacity(), DEFAULT_STORE_CAPACITY);

    let retained = history.retained_samples();
    assert_eq!(retained.len(), DEFAULT_STORE_CAPACITY);
    let expected_first = 10_000 - DEFAULT_STORE_CAPACITY as u64;
    assert_eq!(retained.first().unwrap().timestamp_sec, expected_first);
    assert_eq!(retained.first().unwrap().upload_bytes, expected_first);
    assert_eq!(retained.last().unwrap().timestamp_sec, 9_999);
    assert_eq!(retained.last().unwrap().upload_bytes, 9_999);
    assert_eq!(retained.last().unwrap().download_bytes, 19_998);
}

/// Stepping the scrubber walks the frozen snapshot one sample at a time and
/// the window's right edge follows the cursor exactly.
#[test]
fn scrubber_step_shows_the_right_window() {
    let mut history = TrafficHistory::default();
    for tick in 0..11u64 {
        history.push(tick as f64, (tick * 10) as f64);
    }
    let mut replay = TrafficReplay::default();
    assert!(replay.begin(&history), "history retained");
    assert_eq!(
        replay_state(OverviewOrigin::LiveCore, &replay),
        ReplayState::Historical { index: 10, len: 11 }
    );

    replay.apply(&history, ScrubberAction::StepBackward);
    assert_eq!(
        replay_state(OverviewOrigin::LiveCore, &replay),
        ReplayState::Historical { index: 9, len: 11 }
    );
    let (up, down) = replay.replay_series().expect("historical window");
    assert_eq!(up.last(), Some(&9.0), "window ends at the cursor");
    assert_eq!(down.last(), Some(&90.0));
    assert_eq!(replay.current().unwrap().timestamp_sec, 9);

    replay.apply(&history, ScrubberAction::StepForward);
    let (up, _) = replay.replay_series().unwrap();
    assert_eq!(up.last(), Some(&10.0), "step forward returns to newest");
}

/// A large frozen range is downsampled to the chart budget, and the reduced
/// window still ends on the cursor sample.
#[test]
fn scrubber_seek_downsamples_a_large_range() {
    let mut history = TrafficHistory::default();
    for tick in 0..200u64 {
        history.push(tick as f64, (tick * 10) as f64);
    }
    let mut replay = TrafficReplay::default();
    assert!(replay.begin(&history));
    replay.apply(&history, ScrubberAction::StepBackward);

    assert_eq!(
        replay.current().unwrap().timestamp_sec,
        198,
        "cursor sample"
    );
    let (up, down) = replay.replay_series().expect("historical window");
    assert_eq!(up.len(), REPLAY_WINDOW_POINTS, "reduced to the budget");
    assert!(up.len() < replay.snapshot_len(), "genuinely downsampled");
    let last_upload = *up.last().unwrap();
    let last_download = *down.last().unwrap();
    assert!(
        (195.0..=198.0).contains(&last_upload),
        "the final bucket averages the samples up to the cursor"
    );
    assert!(
        (1950.0..=1980.0).contains(&last_download),
        "the download lane shares the final bucket"
    );
}

/// A seek jumps a fixed fraction of the snapshot, in both directions.
#[test]
fn scrubber_seek_moves_by_a_fraction() {
    let mut history = TrafficHistory::default();
    for tick in 0..11u64 {
        history.push(tick as f64, tick as f64);
    }
    let mut replay = TrafficReplay::default();
    replay.apply(&history, ScrubberAction::SeekBackward);
    assert_eq!(
        replay_state(OverviewOrigin::LiveCore, &replay),
        ReplayState::Historical { index: 9, len: 11 },
        "one tenth of ten steps lands one sample back"
    );
    replay.apply(&history, ScrubberAction::SeekForward);
    assert_eq!(
        replay_state(OverviewOrigin::LiveCore, &replay),
        ReplayState::Historical { index: 10, len: 11 }
    );
}

/// Returning to live drops the frozen snapshot and resumes the live window.
#[test]
fn return_to_live_resumes_the_live_window() {
    let mut history = TrafficHistory::default();
    for tick in 0..20u64 {
        history.push(tick as f64, tick as f64);
    }
    let mut replay = TrafficReplay::default();
    replay.apply(&history, ScrubberAction::StepBackward);
    assert!(replay.is_scrubbing());
    assert!(replay.snapshot_len() > 0);

    replay.apply(&history, ScrubberAction::ReturnToLive);
    assert!(!replay.is_scrubbing());
    assert_eq!(replay.snapshot_len(), 0, "snapshot released");
    assert_eq!(
        replay_state(OverviewOrigin::LiveCore, &replay),
        ReplayState::Live
    );
    assert!(
        replay.replay_series().is_none(),
        "live draws its own series"
    );
    assert!(replay_chart_inputs(&replay).is_none());
}

/// With no retained history the scrubber reports a typed empty state and
/// draws nothing — it never pads a fabricated series.
#[test]
fn scrubber_empty_state_is_typed_and_fabricates_nothing() {
    let history = TrafficHistory::default();
    let mut replay = TrafficReplay::default();
    assert!(!replay.begin(&history), "nothing retained");
    assert!(replay.is_scrubbing());
    assert_eq!(
        replay_state(OverviewOrigin::LiveCore, &replay),
        ReplayState::Empty
    );
    assert!(replay.current().is_none());
    assert!(replay.replay_series().is_none());
    assert!(replay_chart_inputs(&replay).is_none());

    // The demo fixture's synthetic trend has no measured history to replay.
    assert_eq!(
        replay_state(OverviewOrigin::Demo, &replay),
        ReplayState::Unsupported
    );
}
