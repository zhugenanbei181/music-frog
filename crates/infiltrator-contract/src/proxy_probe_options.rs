//! Persisted latency-probe parameters and the neutral raw editor draft.
use crate::error::Failure;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PROBE_URL: &str = "http://www.gstatic.com/generate_204";
pub const DEFAULT_PROBE_TIMEOUT_MS: u32 = 5000;
/// mihomo v1.19.18 parses the HTTP timeout as a signed 16-bit millisecond value.
pub const MAX_PROBE_TIMEOUT_MS: u32 = 32767;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyProbeOptions {
    pub test_url: String,
    pub timeout_ms: u32,
}
impl Default for ProxyProbeOptions {
    fn default() -> Self {
        Self {
            test_url: DEFAULT_PROBE_URL.into(),
            timeout_ms: DEFAULT_PROBE_TIMEOUT_MS,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyProbeDraft {
    pub test_url: String,
    pub timeout_ms: String,
}
impl From<&ProxyProbeOptions> for ProxyProbeDraft {
    fn from(options: &ProxyProbeOptions) -> Self {
        Self {
            test_url: options.test_url.clone(),
            timeout_ms: options.timeout_ms.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyProbeSettingsSnapshot {
    /// None means the settings read failed; no applied parameters may be invented.
    pub options: Option<ProxyProbeOptions>,
    pub can_persist: bool,
    pub failure: Option<Failure>,
}
impl Default for ProxyProbeSettingsSnapshot {
    fn default() -> Self {
        Self {
            options: Some(ProxyProbeOptions::default()),
            can_persist: false,
            failure: None,
        }
    }
}
