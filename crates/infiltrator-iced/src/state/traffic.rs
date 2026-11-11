//! Transport adapter selection; all rate and peak semantics are folded by application.
use crate::state::AppState;
use infiltrator_application::traffic_audit_application::{
    TrafficAuditApplication, TrafficAuditReadback,
};
use infiltrator_contract::shell_readout::ShellReadoutSnapshot;
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
use infiltrator_domain::connection_rate::ConnectionRates;
use infiltrator_domain::connection_view::ConnectionView;
use infiltrator_domain::traffic_audit::{TrafficDimension, TrafficWindow};
use infiltrator_presentation::shell_readout::observed_rate;
use infiltrator_presentation::traffic_readout::{TrafficReadout, project_traffic_readout};
use infiltrator_shared::locales::Lang;

/// The traffic audit's surface half (TRAFFICAUDIT-001).
///
/// The owner records every bucket and reduces every window; the surface holds
/// the shared handle plus the reader's own selection, and asks for a readback.
/// It folds nothing, and the selected window is a query, not a second model of
/// the traffic.
pub struct TrafficAuditState {
    pub audit: TrafficAuditApplication,
    /// The range the reader is looking at. Defaults to the reference product's
    /// own default (`24h`).
    pub window: TrafficWindow,
    /// The dimension the ranking list is grouped by. Defaults to the reference
    /// product's own default (`host`).
    pub dimension: TrafficDimension,
    /// The instant of the last observation, which is what a readback cuts its
    /// window against. A surface never reads the wall clock to ask for a
    /// window: it asks on the data's own clock.
    pub observed_at_secs: Option<u64>,
}

impl Default for TrafficAuditState {
    fn default() -> Self {
        Self {
            audit: TrafficAuditApplication::new(),
            window: TrafficWindow::LastDay,
            dimension: TrafficDimension::Domain,
            observed_at_secs: None,
        }
    }
}

impl TrafficAuditState {
    /// Feed one observation into the owner and remember its instant.
    pub fn observe<C: ConnectionView>(
        &mut self,
        observed_at_secs: u64,
        conns: &[C],
        rates: &ConnectionRates,
    ) {
        self.audit.observe(observed_at_secs, conns, rates);
        // The window is cut against the newest observation, not the last one
        // written: a sample that arrives late must not move every window back
        // in time and hide the traffic that came after it.
        self.observed_at_secs = Some(
            self.observed_at_secs
                .map_or(observed_at_secs, |previous| previous.max(observed_at_secs)),
        );
    }

    /// The readback for the current selection, when an observation has been
    /// seen. Before the first observation there is no instant to cut a window
    /// against, so there is nothing honest to show.
    pub fn readback(&self, limit: usize) -> Option<TrafficAuditReadback> {
        self.observed_at_secs
            .map(|now| self.audit.readback(self.dimension, self.window, now, limit))
    }
}

impl AppState {
    pub(crate) fn traffic_readout(&self) -> TrafficReadout {
        if self.commands.is_some() || self.surface.latest().is_some() {
            return project_traffic_readout(
                &self.shell.readout,
                &self.runtime.traffic_waveform,
                &Lang(&self.shell.lang),
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
        project_traffic_readout(&rates, &waveform, &Lang(&self.shell.lang))
    }
}
