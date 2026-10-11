//! Shared rates, retained-history peaks and observation state; native renderers replay this fold.
use crate::shell_readout::{rate_copy, rate_status};
use infiltrator_contract::shell_readout::{ShellObservation, ShellReadoutSnapshot};
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficFreshness {
    Failed,
    Current,
    Stale,
    Unobserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrafficDirection {
    Upload,
    Download,
}

pub struct TrafficReadout {
    pub upload: String,
    pub download: String,
    pub upload_peak: Option<String>,
    pub download_peak: Option<String>,
    pub freshness: TrafficFreshness,
    pub failure: String,
    pub status: String,
}
pub fn project_traffic_readout(
    rates: &ShellReadoutSnapshot,
    waveform: &TrafficWaveformSnapshot,
    language: &str,
) -> TrafficReadout {
    let freshness = match (
        rates.rate_failure.as_ref(),
        &rates.upload_bps,
        &rates.download_bps,
    ) {
        (Some(_), _, _) => TrafficFreshness::Failed,
        (None, ShellObservation::Current(_), ShellObservation::Current(_)) => {
            TrafficFreshness::Current
        }
        (
            None,
            ShellObservation::Unobserved | ShellObservation::CurrentEmpty,
            ShellObservation::Unobserved | ShellObservation::CurrentEmpty,
        ) => TrafficFreshness::Unobserved,
        (None, _, _) => TrafficFreshness::Stale,
    };
    let status_key = match freshness {
        TrafficFreshness::Failed => "conn_state_unavailable",
        TrafficFreshness::Current => "conn_state_live",
        TrafficFreshness::Stale => "shell_rate_stale",
        TrafficFreshness::Unobserved => "shell_readout_unknown",
    };
    let peak = |direction| {
        let value = (waveform.generation == rates.generation)
            .then(|| {
                waveform
                    .samples
                    .iter()
                    .map(|sample| match direction {
                        TrafficDirection::Upload => sample.upload_bps,
                        TrafficDirection::Download => sample.download_bps,
                    })
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .max_by(f64::total_cmp)
            })
            .flatten();
        value.map(|value| {
            rate_copy(
                &match freshness {
                    TrafficFreshness::Current => ShellObservation::Current(value),
                    TrafficFreshness::Failed
                    | TrafficFreshness::Stale
                    | TrafficFreshness::Unobserved => ShellObservation::Stale(value),
                },
                language,
            )
        })
    };
    TrafficReadout {
        upload: rate_copy(&rates.upload_bps, language),
        download: rate_copy(&rates.download_bps, language),
        upload_peak: peak(TrafficDirection::Upload),
        download_peak: peak(TrafficDirection::Download),
        freshness,
        failure: rate_status(rates, language),
        status: Lang(language).tr(status_key).into_owned(),
    }
}
