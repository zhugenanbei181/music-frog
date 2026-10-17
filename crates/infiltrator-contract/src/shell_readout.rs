//! Observed shell facts; unavailable observations never become believable defaults.
use crate::error::Failure;
use crate::mini_hud::MiniHudWaveformStrip;
use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum ShellObservation<T> {
    /// No value has been observed for this session.
    #[default]
    Unobserved,
    /// The value belongs to the accepted snapshot.
    Current(T),
    /// The accepted page is current and explicitly contains no profile.
    CurrentEmpty,
    /// The last accepted value is retained after the reader became unavailable.
    Stale(T),
}

impl<T> ShellObservation<T> {
    /// Return any observed value, including a retained stale value.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Current(value) | Self::Stale(value) => Some(value),
            Self::Unobserved | Self::CurrentEmpty => None,
        }
    }

    /// Return a value only when it belongs to the current accepted snapshot.
    pub fn current_value(&self) -> Option<&T> {
        match self {
            Self::Current(value) => Some(value),
            Self::Unobserved | Self::CurrentEmpty | Self::Stale(_) => None,
        }
    }

    /// Retain an observed value as stale; empty/unobserved states stay unobserved.
    pub fn into_stale(self) -> Self {
        match self {
            Self::Current(value) | Self::Stale(value) => Self::Stale(value),
            Self::Unobserved | Self::CurrentEmpty => Self::Unobserved,
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

#[cfg(test)]
#[path = "shell_readout_tests.rs"]
mod tests;
