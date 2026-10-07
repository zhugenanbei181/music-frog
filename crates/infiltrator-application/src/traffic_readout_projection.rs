//! Shared rates, retained-history peaks and observation state; native renderers replay this fold.
use crate::shell_readout_projection::{rate_copy, rate_status};
use infiltrator_contract::shell_readout::{ShellObservation, ShellReadoutSnapshot};
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct TrafficReadout {
    pub upload: String,
    pub download: String,
    pub upload_peak: Option<String>,
    pub download_peak: Option<String>,
    pub current: bool,
    pub observed: bool,
    pub failure: String,
    pub status: String,
}
pub fn project_traffic_readout(
    rates: &ShellReadoutSnapshot,
    waveform: &TrafficWaveformSnapshot,
    language: &str,
) -> TrafficReadout {
    let current = rates.upload_bps.current && rates.download_bps.current;
    let observed = rates.upload_bps.value.is_some() || rates.download_bps.value.is_some();
    let status_key = if rates.rate_failure.is_some() {
        "conn_state_unavailable"
    } else if current {
        "conn_state_live"
    } else if observed {
        "shell_rate_stale"
    } else {
        "shell_readout_unknown"
    };
    let peak = |upload: bool| {
        let value = (waveform.generation == rates.generation)
            .then(|| {
                waveform
                    .samples
                    .iter()
                    .map(|sample| {
                        if upload {
                            sample.upload_bps
                        } else {
                            sample.download_bps
                        }
                    })
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .max_by(f64::total_cmp)
            })
            .flatten();
        value.map(|value| {
            rate_copy(
                &ShellObservation {
                    value: Some(value),
                    current: rates.upload_bps.current && rates.download_bps.current,
                },
                language,
            )
        })
    };
    TrafficReadout {
        upload: rate_copy(&rates.upload_bps, language),
        download: rate_copy(&rates.download_bps, language),
        upload_peak: peak(true),
        download_peak: peak(false),
        current,
        observed,
        failure: rate_status(rates, language),
        status: Lang(language).tr(status_key).into_owned(),
    }
}

#[cfg(test)]
#[path = "traffic_readout_projection_tests.rs"]
mod tests;
