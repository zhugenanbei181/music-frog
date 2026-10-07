//! Transport adapter selection; all rate and peak semantics are folded by application.
use crate::state::AppState;
use infiltrator_application::shell_readout_projection::observed_rate;
use infiltrator_application::traffic_readout_projection::{
    TrafficReadout, project_traffic_readout,
};
use infiltrator_contract::shell_readout::ShellReadoutSnapshot;
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
impl AppState {
    pub(crate) fn traffic_readout(&self) -> TrafficReadout {
        if self.commands.is_some() || self.surface.latest().is_some() {
            return project_traffic_readout(
                &self.shell.readout,
                &self.runtime.traffic_waveform,
                &self.shell.lang,
            );
        }
        let rates = ShellReadoutSnapshot {
            upload_bps: observed_rate(self.diag.traffic.as_ref().map(|data| data.up as f64)),
            download_bps: observed_rate(self.diag.traffic.as_ref().map(|data| data.down as f64)),
            ..Default::default()
        };
        let waveform = TrafficWaveformSnapshot {
            samples: self
                .diag
                .traffic_history
                .iter()
                .map(|(up, down)| TrafficSample {
                    upload_bps: *up as f64,
                    download_bps: *down as f64,
                    sampled_at_epoch_ms: None,
                })
                .collect(),
            ..Default::default()
        };
        project_traffic_readout(&rates, &waveform, &self.shell.lang)
    }
}
