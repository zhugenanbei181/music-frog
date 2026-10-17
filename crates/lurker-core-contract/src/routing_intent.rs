//! Engine-neutral intent for choosing how traffic is routed.

use crate::core_identity::EngineId;
use crate::error::Failure;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

/// A routing policy shared by multiple core engines, or a typed request for an
/// engine-defined extension that an adapter may explicitly reject.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "extension", rename_all = "snake_case")]
pub enum RoutingIntent {
    /// Evaluate each connection against the active routing rules.
    FollowRules,
    /// Route eligible connections through the core's selected proxy.
    RouteAllThroughProxy,
    /// Bypass the core's proxy and connect directly.
    BypassProxy,
    /// Request an engine-owned policy that is outside the common contract.
    EngineDefined(EngineDefinedRoutingIntent),
}

/// Availability of the currently observed routing intent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingIntentStatus {
    #[default]
    Unobserved,
    Ready,
    Pending,
    Unsupported,
    Failed,
}

/// The observed routing policy and the status of the latest read or change.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingIntentSnapshot {
    pub current: Option<RoutingIntent>,
    pub status: RoutingIntentStatus,
    pub failure: Option<Failure>,
}

impl RoutingIntentSnapshot {
    pub fn demo_fixture() -> Self {
        Self {
            current: Some(RoutingIntent::FollowRules),
            status: RoutingIntentStatus::Ready,
            failure: None,
        }
    }

    pub fn is_drawable(&self) -> bool {
        matches!(
            self.status,
            RoutingIntentStatus::Ready | RoutingIntentStatus::Pending
        )
    }

    /// Shared choices are selectable only after a ready observation. Engine
    /// extensions remain adapter-owned and cannot be offered as common choices.
    pub fn is_intent_selectable(&self, intent: &RoutingIntent) -> bool {
        self.status == RoutingIntentStatus::Ready
            && matches!(
                intent,
                RoutingIntent::FollowRules
                    | RoutingIntent::RouteAllThroughProxy
                    | RoutingIntent::BypassProxy
            )
    }
}

/// Names a routing policy owned by one engine adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDefinedRoutingIntent {
    engine: EngineId,
    extension_id: RoutingIntentExtensionId,
}

impl EngineDefinedRoutingIntent {
    pub fn new(engine: EngineId, extension_id: RoutingIntentExtensionId) -> Self {
        Self {
            engine,
            extension_id,
        }
    }

    pub fn engine(&self) -> &EngineId {
        &self.engine
    }

    pub fn extension_id(&self) -> &RoutingIntentExtensionId {
        &self.extension_id
    }
}

/// Stable, lowercase identifier for an engine-owned routing policy.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RoutingIntentExtensionId(String);

impl RoutingIntentExtensionId {
    pub fn new(value: impl Into<String>) -> Result<Self, RoutingIntentExtensionIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RoutingIntentExtensionIdError::Empty);
        }
        if value.len() > 128
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
        {
            return Err(RoutingIntentExtensionIdError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RoutingIntentExtensionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentExtensionIdError {
    Empty,
    Invalid,
}

impl Display for RoutingIntentExtensionIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("routing intent extension ID must not be empty"),
            Self::Invalid => formatter.write_str(
                "routing intent extension ID must use lowercase ASCII letters, digits, dot, underscore, or hyphen",
            ),
        }
    }
}

impl Error for RoutingIntentExtensionIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_defined_intents_serialize_with_their_owner_and_validated_id() {
        let extension = EngineDefinedRoutingIntent::new(
            EngineId::new("mihomo").expect("valid engine ID"),
            RoutingIntentExtensionId::new("script").expect("valid extension ID"),
        );
        let intent = RoutingIntent::EngineDefined(extension);
        let encoded = serde_json::to_string(&intent).expect("serialize routing intent");
        let decoded: RoutingIntent =
            serde_json::from_str(&encoded).expect("deserialize routing intent");
        assert_eq!(decoded, intent);
    }

    #[test]
    fn extension_ids_reject_empty_or_noncanonical_values() {
        assert_eq!(
            RoutingIntentExtensionId::new(""),
            Err(RoutingIntentExtensionIdError::Empty)
        );
        assert_eq!(
            RoutingIntentExtensionId::new("Script"),
            Err(RoutingIntentExtensionIdError::Invalid)
        );
        assert!(serde_json::from_str::<RoutingIntentExtensionId>("\"script mode\"").is_err());
    }
}
