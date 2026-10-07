//! Observed shell facts; unavailable observations never become believable defaults.
use crate::error::Failure;
use crate::mini_hud::MiniHudWaveformStrip;
use crate::session::SessionToken;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShellObservation<T> {
    pub value: Option<T>,
    pub current: bool,
}
impl<T> Default for ShellObservation<T> {
    fn default() -> Self {
        Self {
            value: None,
            current: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShellProfile {
    pub id: String,
    pub name: String,
    pub subscription: bool,
    pub used_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub usage_fraction: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShellReadoutSnapshot {
    pub waveform: MiniHudWaveformStrip,
    pub rate_failure: Option<Failure>,
    pub generation: u64,
    #[serde(default)]
    pub session_token: Option<SessionToken>,
    pub revision: u64,
    pub proxies: ShellObservation<usize>,
    pub rules: ShellObservation<usize>,
    pub connections: ShellObservation<usize>,
    pub dns: ShellObservation<usize>,
    pub profile: ShellObservation<ShellProfile>,
    pub upload_bps: ShellObservation<f64>,
    pub download_bps: ShellObservation<f64>,
}
