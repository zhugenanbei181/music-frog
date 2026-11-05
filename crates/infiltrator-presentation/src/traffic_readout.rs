//! Shared rates, retained-history peaks and observation state; native renderers replay this fold.
use crate::shell_readout::{ShellQuantityBadge, rate_badge, rate_copy, rate_status};
use infiltrator_contract::shell_readout::{ShellObservation, ShellReadoutSnapshot};
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use lurker_ui_presentation::copy::TextCatalog;

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
    pub upload_display: String,
    pub download_display: String,
    pub upload_peak: Option<String>,
    pub download_peak: Option<String>,
    pub freshness: TrafficFreshness,
    pub failure: String,
    pub status: String,
}

/// A compact rate visualization: unobserved data is a dash, never a sentence.
fn compact_rate(rate: &ShellObservation<f64>, catalog: &dyn TextCatalog) -> String {
    match rate_badge(rate) {
        ShellQuantityBadge::Current(_) | ShellQuantityBadge::Stale(_) => rate_copy(rate, catalog),
        ShellQuantityBadge::Unobserved => "—".into(),
    }
}

pub fn project_traffic_readout(
    rates: &ShellReadoutSnapshot,
    waveform: &TrafficWaveformSnapshot,
    catalog: &dyn TextCatalog,
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
    let status = match freshness {
        TrafficFreshness::Failed => catalog.text("conn_state_unavailable").into_owned(),
        TrafficFreshness::Current => catalog.text("conn_state_live").into_owned(),
        TrafficFreshness::Stale => catalog.text("shell_rate_stale").into_owned(),
        TrafficFreshness::Unobserved => String::new(),
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
                catalog,
            )
        })
    };
    TrafficReadout {
        upload: rate_copy(&rates.upload_bps, catalog),
        download: rate_copy(&rates.download_bps, catalog),
        upload_display: compact_rate(&rates.upload_bps, catalog),
        download_display: compact_rate(&rates.download_bps, catalog),
        upload_peak: peak(TrafficDirection::Upload),
        download_peak: peak(TrafficDirection::Download),
        freshness,
        failure: rate_status(rates, catalog),
        status,
    }
}
