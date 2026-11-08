//! Application-owned live traffic waveform history.

use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_domain::traffic_waveform::TrafficWaveformBuffer;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct TrafficWaveformApplication {
    buffer: Arc<Mutex<TrafficWaveformBuffer>>,
}

impl TrafficWaveformApplication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, core: &CoreSnapshot) -> TrafficWaveformSnapshot {
        self.buffer
            .lock()
            .expect("traffic waveform lock")
            .record(core)
    }

    pub fn snapshot(&self) -> TrafficWaveformSnapshot {
        self.buffer
            .lock()
            .expect("traffic waveform lock")
            .snapshot()
    }

    /// Clear the live traffic history on an explicit owner command. The
    /// readback is published with a new revision, never a fake zero sample.
    pub fn reset(&self) -> TrafficWaveformSnapshot {
        self.buffer.lock().expect("traffic waveform lock").reset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreSessionObservation;
    use lurker_core_contract::lifecycle::CoreLifecycle;

    #[test]
    fn application_records_only_live_core_samples() {
        let application = TrafficWaveformApplication::new();
        let snapshot = application.record(&CoreSnapshot {
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
        });
        assert_eq!(snapshot.samples.len(), 1);
        assert!(!application.snapshot().is_drawable());
    }

    #[test]
    fn an_explicit_reset_publishes_an_empty_readback_with_a_new_revision() {
        let application = TrafficWaveformApplication::new();
        let core = CoreSnapshot {
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
        };
        let before = application.record(&core);
        assert_eq!(before.samples.len(), 1);
        let after = application.reset();
        assert!(after.samples.is_empty());
        assert!(after.revision > before.revision);
        assert_eq!(application.snapshot().samples, after.samples);
    }
}
