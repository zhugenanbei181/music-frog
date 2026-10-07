//! Toolkit-neutral detail rows. Metrics and ordering are folded in application.
use crate::capability::Availability;
use crate::speedtest::EgressCountryMatch;

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedtestDetailRow {
    pub node_name: String,
    pub proxy_type: String,
    pub delay: String,
    pub jitter: String,
    pub loss: String,
    pub bandwidth: String,
    pub stars: String,
    pub egress: String,
    pub egress_match: EgressCountryMatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedtestDetails {
    pub rows: Vec<SpeedtestDetailRow>,
    pub availability: Option<Availability>,
    pub failure: Option<String>,
    pub reported_egress: usize,
    pub mismatches: usize,
}
