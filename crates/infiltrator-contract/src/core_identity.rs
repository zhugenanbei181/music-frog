//! Engine-neutral identity and fencing types for a single active core.

use crate::command::{CommandIntent, CommandResult};
use crate::session::SessionToken;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreIdentityError {
    EmptyEngineId,
    InvalidEngineId,
    EmptyInstanceId,
    OversizedInstanceId,
    EmptyVersion,
}

/// Monotonic activation counter for a core instance within one owner process.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct CoreGeneration(u64);

impl CoreGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for CoreGeneration {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<CoreGeneration> for u64 {
    fn from(value: CoreGeneration) -> Self {
        value.value()
    }
}

impl Display for CoreIdentityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEngineId => formatter.write_str("engine ID must not be empty"),
            Self::InvalidEngineId => {
                formatter.write_str("engine ID must use lowercase ASCII identifier characters")
            }
            Self::EmptyInstanceId => formatter.write_str("core instance ID must not be empty"),
            Self::OversizedInstanceId => formatter.write_str("core instance ID exceeds 128 bytes"),
            Self::EmptyVersion => formatter.write_str("core version must not be empty"),
        }
    }
}

impl Error for CoreIdentityError {}

/// Stable identifier for an engine adapter, such as `mihomo` or `sing-box`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct EngineId(String);

impl EngineId {
    pub fn mihomo() -> Self {
        Self::new("mihomo").expect("Mihomo engine ID is valid")
    }

    pub fn sing_box() -> Self {
        Self::new("sing-box").expect("sing-box engine ID is valid")
    }

    pub fn new(value: impl Into<String>) -> Result<Self, CoreIdentityError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CoreIdentityError::EmptyEngineId);
        }
        if value.len() > 64
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
            })
        {
            return Err(CoreIdentityError::InvalidEngineId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for EngineId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Stable identity for one installed/configured core instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CoreInstanceId(String);

impl CoreInstanceId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreIdentityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CoreIdentityError::EmptyInstanceId);
        }
        if value.len() > 128 {
            return Err(CoreIdentityError::OversizedInstanceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CoreInstanceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// The engine-reported version string. Parsing policy stays in its adapter;
/// readiness may add or refresh this runtime capability evidence.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CoreVersion(String);

impl CoreVersion {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreIdentityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CoreIdentityError::EmptyVersion);
        }
        Ok(Self(value))
    }

    pub fn raw(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CoreVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Runtime identity observation for one core instance.
///
/// `engine` and `instance` identify the stable owner. `version` is dynamic
/// capability evidence and may be absent before readiness or change when the
/// active process is observed again. Use [`CoreIdentity::same_instance`] for
/// owner fences; full equality remains appropriate for version-scoped
/// observations and requests.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CoreIdentity {
    pub engine: EngineId,
    pub instance: CoreInstanceId,
    pub version: Option<CoreVersion>,
}

impl CoreIdentity {
    pub fn new(engine: EngineId, instance: CoreInstanceId, version: Option<CoreVersion>) -> Self {
        Self {
            engine,
            instance,
            version,
        }
    }

    /// Whether two runtime observations refer to the same stable engine
    /// instance, ignoring version evidence that can change after readiness.
    pub fn same_instance(&self, other: &Self) -> bool {
        self.engine == other.engine && self.instance == other.instance
    }
}

/// One activation of one engine instance. Every scoped request must match it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreScope {
    pub identity: CoreIdentity,
    pub generation: CoreGeneration,
    pub session_token: Option<SessionToken>,
}

/// User intent bound to the engine instance observed by the requesting surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCommandRequest {
    pub scope: CoreScope,
    pub intent: CommandIntent,
}

/// Terminal result plus the scope observed after execution or rejection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCommandOutcome {
    pub scope: Option<CoreScope>,
    pub result: CommandResult,
}

#[cfg(test)]
mod tests {
    use super::{
        CoreGeneration, CoreIdentity, CoreIdentityError, CoreInstanceId, CoreVersion, EngineId,
    };

    #[test]
    fn typed_identity_values_validate_at_the_boundary() {
        assert!(matches!(
            EngineId::new(""),
            Err(CoreIdentityError::EmptyEngineId)
        ));
        assert!(matches!(
            EngineId::new("Mihomo"),
            Err(CoreIdentityError::InvalidEngineId)
        ));
        assert!(matches!(
            CoreInstanceId::new("  "),
            Err(CoreIdentityError::EmptyInstanceId)
        ));
        assert!(matches!(
            CoreVersion::new(""),
            Err(CoreIdentityError::EmptyVersion)
        ));

        let identity = CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new("desktop-main").expect("instance ID"),
            Some(CoreVersion::new("1.19.12").expect("version")),
        );
        let encoded = serde_json::to_string(&identity).expect("serialize identity");
        assert_eq!(
            serde_json::from_str::<CoreIdentity>(&encoded).expect("deserialize identity"),
            identity
        );
        assert!(serde_json::from_str::<EngineId>("\"Mihomo\"").is_err());
        assert_eq!(
            serde_json::to_string(&CoreGeneration::new(42)).expect("serialize generation"),
            "42"
        );
        assert_eq!(
            serde_json::from_str::<CoreGeneration>("42").expect("deserialize generation"),
            CoreGeneration::new(42)
        );
    }

    #[test]
    fn same_instance_ignores_only_dynamic_version_evidence() {
        let engine = EngineId::new("mihomo").expect("engine ID");
        let instance = CoreInstanceId::new("desktop-main").expect("instance ID");
        let before_readiness = CoreIdentity::new(engine.clone(), instance.clone(), None);
        let after_readiness = CoreIdentity::new(
            engine.clone(),
            instance,
            Some(CoreVersion::new("1.19.12").expect("version")),
        );
        let other_instance = CoreIdentity::new(
            engine,
            CoreInstanceId::new("desktop-replacement").expect("instance ID"),
            Some(CoreVersion::new("1.19.12").expect("version")),
        );
        let other_engine = CoreIdentity::new(
            EngineId::new("sing-box").expect("engine ID"),
            before_readiness.instance.clone(),
            None,
        );

        assert_ne!(before_readiness, after_readiness);
        assert!(before_readiness.same_instance(&after_readiness));
        assert!(!before_readiness.same_instance(&other_instance));
        assert!(!before_readiness.same_instance(&other_engine));
    }
}
