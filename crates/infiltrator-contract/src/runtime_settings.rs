//! Engine-neutral runtime settings shared by persistence and every UI surface.

use crate::error::Failure;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;
use url::Url;

pub const DEFAULT_RUNTIME_PROBE_URL: &str = "http://www.gstatic.com/generate_204";
pub const DEFAULT_RUNTIME_PROBE_TIMEOUT_MS: u32 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProbeSettingsError {
    InvalidUrl,
    UnsupportedScheme,
    MissingHost,
    ZeroTimeout,
}

impl Display for RuntimeProbeSettingsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUrl => "probe URL must be an absolute HTTP or HTTPS URL",
            Self::UnsupportedScheme => "probe URL must use HTTP or HTTPS",
            Self::MissingHost => "probe URL must include a host",
            Self::ZeroTimeout => "probe timeout must be greater than zero milliseconds",
        })
    }
}

impl Error for RuntimeProbeSettingsError {}

/// Validated absolute HTTP(S) URL used by outbound latency probes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ProbeUrl(String);

impl ProbeUrl {
    pub fn new(value: impl Into<String>) -> Result<Self, RuntimeProbeSettingsError> {
        let value = value.into();
        let value = value.trim();
        let parsed = Url::parse(value).map_err(|_| RuntimeProbeSettingsError::InvalidUrl)?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(RuntimeProbeSettingsError::UnsupportedScheme);
        }
        if parsed.host_str().is_none() {
            return Err(RuntimeProbeSettingsError::MissingHost);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProbeUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Positive probe timeout. Engine-specific upper limits belong to adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ProbeTimeoutMs(NonZeroU32);

impl ProbeTimeoutMs {
    pub fn new(value: u32) -> Result<Self, RuntimeProbeSettingsError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(RuntimeProbeSettingsError::ZeroTimeout)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl<'de> Deserialize<'de> for ProbeTimeoutMs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// The single persisted and cross-surface owner for latency-probe parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProbeSettings {
    url: ProbeUrl,
    timeout_ms: ProbeTimeoutMs,
}

impl RuntimeProbeSettings {
    pub fn new(url: impl Into<String>, timeout_ms: u32) -> Result<Self, RuntimeProbeSettingsError> {
        Ok(Self {
            url: ProbeUrl::new(url)?,
            timeout_ms: ProbeTimeoutMs::new(timeout_ms)?,
        })
    }

    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    pub const fn timeout_ms(&self) -> u32 {
        self.timeout_ms.get()
    }
}

impl Default for RuntimeProbeSettings {
    fn default() -> Self {
        Self::new(DEFAULT_RUNTIME_PROBE_URL, DEFAULT_RUNTIME_PROBE_TIMEOUT_MS)
            .expect("the built-in runtime probe settings are valid")
    }
}

/// Runtime diagnostic-list ordering, intentionally independent of outbound-page preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDelaySortOrder {
    #[default]
    LatencyAscending,
    LatencyDescending,
    NameAscending,
    NameDescending,
}

impl RuntimeDelaySortOrder {
    pub const ALL: [Self; 4] = [
        Self::LatencyAscending,
        Self::LatencyDescending,
        Self::NameAscending,
        Self::NameDescending,
    ];
}

/// Read state for the persisted probe settings; unavailable never invents defaults.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RuntimeProbeSettingsState {
    Available { settings: RuntimeProbeSettings },
    Unavailable { failure: Failure },
}

impl Default for RuntimeProbeSettingsState {
    fn default() -> Self {
        Self::Unavailable {
            failure: Failure::unsupported("runtime probe settings have not been observed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validated_probe_settings_round_trip_and_invalid_values_fail_closed() {
        let settings = RuntimeProbeSettings::new(" https://probe.example.test/check ", 60_000)
            .expect("engine-neutral settings permit adapter-specific upper bounds");
        let encoded = serde_json::to_string(&settings).unwrap();
        let decoded: RuntimeProbeSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, settings);

        for invalid in [
            r#"{"url":"file:///etc/passwd","timeout_ms":5000}"#,
            r#"{"url":"https://probe.example.test","timeout_ms":0}"#,
            r#"{"url":"relative/path","timeout_ms":1000}"#,
        ] {
            assert!(serde_json::from_str::<RuntimeProbeSettings>(invalid).is_err());
        }
    }

    #[test]
    fn runtime_delay_sort_order_has_a_typed_stable_serialized_form() {
        for order in RuntimeDelaySortOrder::ALL {
            let encoded = serde_json::to_string(&order).unwrap();
            let decoded: RuntimeDelaySortOrder = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, order);
        }
        assert!(serde_json::from_str::<RuntimeDelaySortOrder>("\"delay_asc\"").is_err());
    }
}
