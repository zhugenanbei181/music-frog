//! Headless tests for the typed `ClearTrafficHistory` owner command.
//!
//! The command used to acknowledge success without touching the waveform
//! owner at all, and its capability route claimed `connection.close`. The
//! real curve is owned by the shared `TrafficWaveformApplication`, which the
//! surface reader records into and the command handler clears.

use super::*;
use crate::traffic_waveform_application::TrafficWaveformApplication;
use infiltrator_contract::snapshot::CoreSessionObservation;
use lurker_core_contract::lifecycle::CoreLifecycle;

fn core_snapshot() -> infiltrator_contract::snapshot::CoreSnapshot {
    infiltrator_contract::snapshot::CoreSnapshot {
        session: CoreSessionObservation::Unknown,
        identity: None,
        lifecycle: CoreLifecycle::Running,
        generation: 1,
        session_token: None,
        revision: 0,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: Some(1),
        failure: None,
        upload_bps: 10.0,
        download_bps: 20.0,
        active_connections: 0,
        watchdog: Default::default(),
    }
}

#[tokio::test]
async fn clearing_traffic_history_resets_the_shared_waveform_owner() {
    let waveform = TrafficWaveformApplication::new();
    assert_eq!(waveform.record(&core_snapshot()).samples.len(), 1);
    let commands = CommandApplication::new().with_traffic_waveform(waveform.clone());

    commands
        .execute(CommandIntent::ClearTrafficHistory)
        .await
        .expect("the shared waveform owner answers the clear command");

    // The reader and the command handler share one Arc<Mutex<_>>: a clear
    // must be visible through the reader's own snapshot, not just acknowledged.
    assert!(waveform.snapshot().samples.is_empty());
    assert_eq!(waveform.snapshot().generation, 1);
}

#[tokio::test]
async fn clearing_traffic_history_without_an_owner_refuses_instead_of_faking_success() {
    let commands = CommandApplication::new();
    let failure = commands
        .execute(CommandIntent::ClearTrafficHistory)
        .await
        .expect_err("no composed waveform owner");
    assert!(failure.message.contains("waveform owner"), "{failure:?}");
}
