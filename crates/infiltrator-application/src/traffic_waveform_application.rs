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
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreLifecycle;

    #[test]
    fn application_records_only_live_core_samples() {
        let application = TrafficWaveformApplication::new();
        let snapshot = application.record(&CoreSnapshot {
            lifecycle: CoreLifecycle::Running,
            generation: 1,
            session_token: None,
            revision: 0,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 10.0,
            download_bps: 20.0,
            active_connections: 0,
            memory_bytes: None,
            watchdog: Default::default(),
        });
        assert_eq!(snapshot.samples.len(), 1);
        assert!(!application.snapshot().is_drawable());
    }
}
