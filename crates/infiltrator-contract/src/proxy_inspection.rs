//! Observed proxy details, independent of UI, filtering and selection.
use crate::capability::Availability;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyLatencySample {
    pub time: String,
    pub delay_ms: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Positive reported delays only; zero history entries have an unresolved outcome.
pub struct ProxyRttStatistics {
    pub min_ms: Option<u32>,
    pub max_ms: Option<u32>,
    pub avg_ms: Option<u32>,
    pub valid_count: usize,
}
impl ProxyRttStatistics {
    pub fn from_history(history: &[u32]) -> Self {
        let valid: Vec<_> = history.iter().copied().filter(|delay| *delay > 0).collect();
        if valid.is_empty() {
            return Self::default();
        }
        let sum: u64 = valid.iter().map(|delay| *delay as u64).sum();
        Self {
            min_ms: valid.iter().copied().min(),
            max_ms: valid.iter().copied().max(),
            avg_ms: Some((sum / valid.len() as u64) as u32),
            valid_count: valid.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyInspectionSnapshot {
    pub name: String,
    pub node_type: String,
    pub server: Option<String>,
    pub port: Option<u16>,
    pub cipher: Option<String>,
    pub udp: Option<bool>,
    pub alive: Option<bool>,
    pub delay_ms: Option<u32>,
    /// Controller order, oldest first; no interpolated or synthesized samples.
    pub history: Vec<ProxyLatencySample>,
    pub history_total: usize,
    #[serde(default)]
    pub rtt: ProxyRttStatistics,
    /// Explicitly a historical speedtest record, never an inferred/current server IP.
    #[serde(default)]
    pub egress_ip: Option<String>,
    #[serde(default)]
    pub egress_country: Option<String>,
    #[serde(default)]
    pub egress_recorded_at_epoch_ms: Option<u64>,
    pub timing: Availability,
    #[serde(default)]
    pub can_probe: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyDetailField {
    #[default]
    Protocol,
    Server,
    Port,
    Cipher,
    Udp,
    Health,
    Delay,
    RttMin,
    RttMax,
    RttAvg,
    Egress,
    EgressTime,
}
impl ProxyDetailField {
    pub const ALL: [Self; 12] = [
        Self::Protocol,
        Self::Server,
        Self::Port,
        Self::Cipher,
        Self::Udp,
        Self::Health,
        Self::Delay,
        Self::RttMin,
        Self::RttMax,
        Self::RttAvg,
        Self::Egress,
        Self::EgressTime,
    ];
    pub const fn key(self) -> &'static str {
        match self {
            Self::Protocol => "proxy_inspection_protocol",
            Self::Server => "modal_server_addr",
            Self::Port => "modal_port",
            Self::Cipher => "modal_cipher",
            Self::Udp => "modal_udp",
            Self::Health => "proxy_inspection_health",
            Self::Delay => "proxy_inspection_delay",
            Self::RttMin => "proxy_inspection_rtt_min",
            Self::RttMax => "proxy_inspection_rtt_max",
            Self::RttAvg => "proxy_inspection_rtt_avg",
            Self::Egress => "proxy_inspection_egress",
            Self::EgressTime => "proxy_inspection_egress_time",
        }
    }
}
