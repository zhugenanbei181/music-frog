//! Engine-neutral lifecycle vocabulary shared by every surface.

use crate::error::Failure;
use crate::identity::{CoreGeneration, CoreIdentity, CoreScope};
use crate::session::SessionToken;
use serde::{Deserialize, Serialize};

/// Stable lifecycle vocabulary for REST, UI projection, and FFI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreLifecycle {
    Stopped,
    Starting,
    Ready,
    Running,
    Stopping,
    Failed,
}

/// Minimal lifecycle read model shared by every inbound surface. It is
/// derived from the product snapshot so lifecycle, generation, session
/// identity and failure cannot drift between UI implementations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreLifecycleSnapshot {
    #[serde(default)]
    pub identity: Option<CoreIdentity>,
    pub lifecycle: CoreLifecycle,
    pub generation: u64,
    #[serde(default, with = "core_lifecycle_session_token_wire")]
    pub session_token: Option<SessionToken>,
    pub revision: u64,
    pub failure: Option<Failure>,
}

impl Default for CoreLifecycleSnapshot {
    fn default() -> Self {
        Self {
            identity: None,
            lifecycle: CoreLifecycle::Stopped,
            generation: 0,
            session_token: None,
            revision: 0,
            failure: None,
        }
    }
}

impl CoreLifecycleSnapshot {
    pub fn scope(&self) -> Option<CoreScope> {
        self.identity.clone().map(|identity| CoreScope {
            identity,
            generation: CoreGeneration::from(self.generation),
            session_token: self.session_token,
        })
    }
}

mod core_lifecycle_session_token_wire {
    use crate::session::SessionToken;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(token: &Option<SessionToken>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        token
            .map(|value| value.value().to_string())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SessionToken>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                value
                    .parse::<u128>()
                    .map(SessionToken::new)
                    .map_err(D::Error::custom)
            })
            .transpose()
    }
}

/// Exact inactive lifecycle fact carried by operations that persist selected
/// profile content without starting or applying Core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreInactiveLifecycle {
    Stopped,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::{CoreInactiveLifecycle, CoreLifecycle, CoreLifecycleSnapshot};
    use crate::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
    use crate::session::SessionToken;

    #[test]
    fn lifecycle_snapshot_wire_encodes_session_token_as_a_decimal_string() {
        let snapshot = CoreLifecycleSnapshot {
            identity: Some(CoreIdentity::new(
                EngineId::new("mihomo").expect("valid engine"),
                CoreInstanceId::new("primary").expect("valid instance"),
                None,
            )),
            lifecycle: CoreLifecycle::Running,
            generation: 7,
            session_token: Some(SessionToken::new(42)),
            revision: 9,
            failure: None,
        };

        let value = serde_json::to_value(&snapshot).expect("serialize lifecycle snapshot");
        assert_eq!(value["session_token"], serde_json::json!("42"));
        assert_eq!(value["lifecycle"], serde_json::json!("Running"));

        let decoded: CoreLifecycleSnapshot =
            serde_json::from_value(value).expect("deserialize lifecycle snapshot");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn lifecycle_snapshot_scope_reuses_generation_and_session() {
        let identity = CoreIdentity::new(
            EngineId::new("sing-box").expect("valid engine"),
            CoreInstanceId::new("android").expect("valid instance"),
            None,
        );
        let snapshot = CoreLifecycleSnapshot {
            identity: Some(identity.clone()),
            lifecycle: CoreLifecycle::Ready,
            generation: 3,
            session_token: Some(SessionToken::new(11)),
            revision: 4,
            failure: None,
        };

        let scope = snapshot.scope().expect("identity produces a scope");
        assert_eq!(scope.identity, identity);
        assert_eq!(scope.generation, CoreGeneration::new(3));
        assert_eq!(scope.session_token, Some(SessionToken::new(11)));
    }

    #[test]
    fn inactive_lifecycle_stays_a_separate_two_state_vocabulary() {
        assert_eq!(
            serde_json::to_value(CoreInactiveLifecycle::Stopped).expect("serialize"),
            serde_json::json!("stopped")
        );
        assert_eq!(
            serde_json::to_value(CoreInactiveLifecycle::Failed).expect("serialize"),
            serde_json::json!("failed")
        );
    }
}
