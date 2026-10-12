//! Typed durable records for the product-wide active profile and Core owner.

use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::error::PortError;
use serde::{Deserialize, Serialize};

pub const ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION: u32 = 1;
pub const ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION: u32 = 1;
pub const ACTIVE_CORE_OWNER_STATE_FILE: &str = "active-core-owner.json";

pub fn transition_rejected(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message.into(), true))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActiveOwnerToken(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "owner", rename_all = "snake_case")]
pub enum ActiveCoreOwnerState {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
        token: ActiveOwnerToken,
    },
    Switching {
        transition_id: ActiveOwnerToken,
        previous_reference: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
        owner_token: ActiveOwnerToken,
        stage: ActiveOwnerSwitchStage,
    },
}

impl Default for ActiveCoreOwnerState {
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
                infiltrator_contract::capability::Capability::Profiles,
                format!(
                    "active Core owner schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum LegacySelectionMigration {
    PendingDesktopMihomoV0,
    MigratedDesktopMihomoV0,
    RejectedDesktopMihomoV0,
    ExplicitSelection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveProfileReferenceRecord {
    pub schema_version: u32,
    pub selection: ActiveProfileReferenceState,
    pub legacy_migration: LegacySelectionMigration,
    #[serde(default)]
    pub owner: ActiveCoreOwnerState,
}

impl ActiveProfileReferenceRecord {
    pub fn pending_legacy_migration() -> Self {
        Self {
            schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
            selection: ActiveProfileReferenceState::Unselected {
                revision: ActiveProfileReferenceRevision::new(0),
            },
            legacy_migration: LegacySelectionMigration::PendingDesktopMihomoV0,
            owner: ActiveCoreOwnerState::Unowned,
        }
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.schema_version != ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                format!(
                    "active-profile reference schema {} is unsupported",
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
                infiltrator_contract::capability::Capability::Profiles,
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
                infiltrator_contract::capability::Capability::Profiles,
                format!("active Core owner schema {version} is unsupported"),
            ));
        }
    };
    record.validate()?;
    Ok(record)
}

#[cfg(test)]
#[path = "active_reference_owner_tests.rs"]
mod tests;
