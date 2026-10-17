//! Persisted owner-switch wire types shared by every selection payload.
//!
//! [`ActiveCoreOwnerState`] is generic over the selection payload `S`; the
//! owner proof and the stage enum carry no product selection at all. The serde
//! tags and field names are the historical durable format.

use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::error::PortError;
use serde::{Deserialize, Serialize};

pub const ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION: u32 = 1;

pub fn transition_rejected(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message.into(), true))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActiveOwnerToken(pub String);

impl ActiveOwnerToken {
    pub fn generate() -> Result<Self, PortError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes)
            .map_err(|_| PortError::Failed("could not create active owner fence token".into()))?;
        let value = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Self(value))
    }
}

/// Durable owner state over the product selection payload `S`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "owner", rename_all = "snake_case")]
pub enum ActiveCoreOwnerState<S> {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
        token: ActiveOwnerToken,
    },
    Switching {
        transition_id: ActiveOwnerToken,
        previous_reference: S,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
        owner_token: ActiveOwnerToken,
        stage: ActiveOwnerSwitchStage,
    },
}

impl<S> Default for ActiveCoreOwnerState<S> {
    fn default() -> Self {
        Self::Unowned
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", content = "details", rename_all = "snake_case")]
pub enum ActiveOwnerSwitchStage {
    ClosingOld,
    OldStopped {
        terminal_generation: CoreGeneration,
    },
    ReferenceChanged {
        terminal_generation: CoreGeneration,
    },
    BindingNew {
        terminal_generation: CoreGeneration,
        engine_id: EngineId,
        token: ActiveOwnerToken,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "owner", rename_all = "snake_case")]
pub enum ActiveCoreOwnerProof {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
        token: ActiveOwnerToken,
    },
    Terminal {
        identity: CoreIdentity,
        bound_generation: CoreGeneration,
        terminal_generation: CoreGeneration,
        token: ActiveOwnerToken,
    },
    Reserved {
        transition_id: ActiveOwnerToken,
        engine_id: EngineId,
        token: ActiveOwnerToken,
    },
}

/// Durable evidence for the last managed Core process session associated with
/// an owner proof. Legacy records are explicit and cannot authorize recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "context", rename_all = "snake_case")]
pub enum ActiveCoreOwnerLaunchProof {
    Unstarted,
    Managed(CoreProcessStartContext),
    LegacyUnverified,
}

impl Default for ActiveCoreOwnerLaunchProof {
    fn default() -> Self {
        Self::Unstarted
    }
}

impl Default for ActiveCoreOwnerProof {
    fn default() -> Self {
        Self::Unowned
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveCoreOwnerProofRecord {
    pub schema_version: u32,
    pub state: ActiveCoreOwnerProof,
    pub launch: ActiveCoreOwnerLaunchProof,
}

impl ActiveCoreOwnerProofRecord {
    pub fn unowned() -> Self {
        Self {
            schema_version: ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
            state: ActiveCoreOwnerProof::Unowned,
            launch: ActiveCoreOwnerLaunchProof::Unstarted,
        }
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.schema_version != ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                Capability::Profiles,
                format!(
                    "active Core owner schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct ActiveCoreOwnerProofSchemaHeader {
    schema_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyActiveCoreOwnerProofRecordV1 {
    schema_version: u32,
    state: LegacyActiveCoreOwnerProofV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "state", content = "owner", rename_all = "snake_case")]
enum LegacyActiveCoreOwnerProofV1 {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
        token: ActiveOwnerToken,
    },
    Terminal {
        identity: CoreIdentity,
        bound_generation: CoreGeneration,
        terminal_generation: CoreGeneration,
        token: ActiveOwnerToken,
    },
    Reserved {
        transition_id: ActiveOwnerToken,
        engine_id: EngineId,
        token: ActiveOwnerToken,
    },
}

impl LegacyActiveCoreOwnerProofRecordV1 {
    fn migrate(self) -> Result<ActiveCoreOwnerProofRecord, PortError> {
        if self.schema_version != LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                Capability::Profiles,
                format!(
                    "active Core owner schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        let (state, launch) = match self.state {
            LegacyActiveCoreOwnerProofV1::Unowned => (
                ActiveCoreOwnerProof::Unowned,
                ActiveCoreOwnerLaunchProof::Unstarted,
            ),
            LegacyActiveCoreOwnerProofV1::Bound {
                identity,
                generation,
                token,
            } => (
                ActiveCoreOwnerProof::Bound {
                    identity,
                    generation,
                    token,
                },
                ActiveCoreOwnerLaunchProof::LegacyUnverified,
            ),
            LegacyActiveCoreOwnerProofV1::Terminal {
                identity,
                bound_generation,
                terminal_generation,
                token,
            } => (
                ActiveCoreOwnerProof::Terminal {
                    identity,
                    bound_generation,
                    terminal_generation,
                    token,
                },
                ActiveCoreOwnerLaunchProof::LegacyUnverified,
            ),
            LegacyActiveCoreOwnerProofV1::Reserved {
                transition_id,
                engine_id,
                token,
            } => (
                ActiveCoreOwnerProof::Reserved {
                    transition_id,
                    engine_id,
                    token,
                },
                ActiveCoreOwnerLaunchProof::Unstarted,
            ),
        };
        Ok(ActiveCoreOwnerProofRecord {
            schema_version: ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
            state,
            launch,
        })
    }
}

/// Parse the durable owner proof, upgrading only the explicit v1 format.
pub fn parse_active_core_owner_proof_record(
    text: &str,
) -> Result<ActiveCoreOwnerProofRecord, PortError> {
    let header: ActiveCoreOwnerProofSchemaHeader = serde_json::from_str(text)
        .map_err(|error| PortError::Io(format!("invalid active Core owner proof: {error}")))?;
    let record = match header.schema_version {
        LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION => {
            let legacy: LegacyActiveCoreOwnerProofRecordV1 =
                serde_json::from_str(text).map_err(|error| {
                    PortError::Io(format!("invalid legacy active Core owner proof: {error}"))
                })?;
            legacy.migrate()?
        }
        ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION => serde_json::from_str(text)
            .map_err(|error| PortError::Io(format!("invalid active Core owner proof: {error}")))?,
        version => {
            return Err(PortError::unsupported(
                Capability::Profiles,
                format!("active Core owner schema {version} is unsupported"),
            ));
        }
    };
    record.validate()?;
    Ok(record)
}
