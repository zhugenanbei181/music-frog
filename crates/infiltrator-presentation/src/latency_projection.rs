//! One latency caption/band fold; measured zero and unresolved history zero are distinct.
use infiltrator_contract::latency_display::{LatencyBand, LatencyReading};
use lurker_ui_presentation::copy::{TextCatalog, interpolate};

pub struct LatencyPresentation {
    pub key: &'static str,
    pub params: Vec<(&'static str, String)>,
    pub band: LatencyBand,
}
impl LatencyPresentation {
    pub fn render(&self, catalog: &dyn TextCatalog) -> String {
        let values: Vec<_> = self
            .params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        interpolate(catalog.text(self.key).as_ref(), &values)
    }
}

pub fn project_latency(reading: LatencyReading) -> LatencyPresentation {
    let value = match reading {
        LatencyReading::NotObserved => {
            return LatencyPresentation {
                key: "proxy_inspection_unmeasured",
                params: vec![],
                band: LatencyBand::NotObserved,
            };
        }
        LatencyReading::ControllerHistory(0) => {
            return LatencyPresentation {
                key: "proxy_inspection_unconfirmed_zero",
                params: vec![],
                band: LatencyBand::UnconfirmedZero,
            };
        }
        LatencyReading::Measured(value) | LatencyReading::ControllerHistory(value) => value,
    };
    LatencyPresentation {
        key: "latency_value_ms",
        params: vec![("value", value.to_string())],
        band: if value < 100 {
            LatencyBand::Fast
        } else if value < 250 {
            LatencyBand::Medium
        } else {
            LatencyBand::Slow
        },
    }
}

pub fn project_proxy_latency(delay: Option<u32>) -> LatencyPresentation {
    project_latency(delay.map_or(
        LatencyReading::NotObserved,
        LatencyReading::ControllerHistory,
    ))
}

pub fn project_measured_latency(delay: Option<u32>) -> LatencyPresentation {
    project_latency(delay.map_or(LatencyReading::NotObserved, LatencyReading::Measured))
}
