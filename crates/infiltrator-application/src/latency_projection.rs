//! One latency caption/band fold; measured zero and unresolved history zero are distinct.
use infiltrator_contract::latency_display::{LatencyBand, LatencyReading};
use infiltrator_shared::i18n_interpolator::interpolate;

pub struct LatencyPresentation {
    pub key: &'static str,
    pub params: Vec<(&'static str, String)>,
    pub band: LatencyBand,
}
impl LatencyPresentation {
    pub fn render(&self, tr: &impl Fn(&str) -> String) -> String {
        let values: Vec<_> = self
            .params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        interpolate(&tr(self.key), &values)
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

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_shared::locales::{Lang, Localizer};

    #[test]
    fn provenance_keeps_measured_zero_history_zero_and_absence_distinct_in_each_language() {
        for language in ["en-US", "zh-CN"] {
            let tr = |key: &str| Lang(language).tr(key).into_owned();
            let zero = project_measured_latency(Some(0));
            assert_eq!(zero.band, LatencyBand::Fast);
            assert_eq!(zero.render(&tr), "0 ms");
            let ambiguous = project_proxy_latency(Some(0));
            assert_eq!(ambiguous.band, LatencyBand::UnconfirmedZero);
            assert_eq!(
                ambiguous.render(&tr),
                tr("proxy_inspection_unconfirmed_zero")
            );
            let missing = project_proxy_latency(None);
            assert_eq!(missing.band, LatencyBand::NotObserved);
            assert_eq!(missing.render(&tr), tr("proxy_inspection_unmeasured"));
            assert_ne!(ambiguous.render(&tr), missing.render(&tr));
        }
    }
}
