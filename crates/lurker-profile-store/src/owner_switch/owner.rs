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

/// The schema version this crate writes for the owner proof record.
pub const ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION: u32 = 2;
/// The schema version of the legacy owner proof record this crate migrates.
pub const LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION: u32 = 1;

/// Builds the typed "transition rejected" failure of the owner-switch protocol.
///
/// The failure is retryable: a rejected transition usually means the journal
/// moved on and the caller has to re-read it before trying again.
pub fn transition_rejected(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message.into(), true))
}

/// Opaque token that fences one owner transition.
///
/// The value is a random hex string; it is compared, never parsed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActiveOwnerToken(pub String);

impl ActiveOwnerToken {
    /// Mints a fresh, random transition token.
    ///
    /// Fails when the operating system cannot supply randomness.
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
    /// No owner is bound.
    Unowned,
    /// One owner is bound to the current selection.
    Bound {
        /// Stable identity of the bound owner.
        identity: CoreIdentity,
        /// Exact generation the owner is bound to.
        generation: CoreGeneration,
        /// The owner fence token of the binding.
        token: ActiveOwnerToken,
    },
    /// A replacement is in progress.
    Switching {
        /// The token that identifies this transition.
        transition_id: ActiveOwnerToken,
        /// The selection the previous owner was bound to.
        previous_reference: S,
        /// Stable identity of the previous owner.
        owner_identity: CoreIdentity,
        /// Exact generation the previous owner is bound to.
        owner_generation: CoreGeneration,
        /// The owner fence token of the previous binding.
        owner_token: ActiveOwnerToken,
        /// The durable stage of the running transition.
        stage: ActiveOwnerSwitchStage,
    },
}

impl<S> Default for ActiveCoreOwnerState<S> {
    fn default() -> Self {
        Self::Unowned
    }
}

/// Durable stage of one owner-switch transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", content = "details", rename_all = "snake_case")]
pub enum ActiveOwnerSwitchStage {
    /// The previous owner is still live.
    ClosingOld,
    /// The previous owner reached an exact terminal generation.
    OldStopped {
        /// The exact generation observed as terminal.
        terminal_generation: CoreGeneration,
    },
    /// The new reference was committed.
    ReferenceChanged {
        /// The exact generation observed as terminal.
        terminal_generation: CoreGeneration,
    },
    /// The candidate owner admission was reserved.
    BindingNew {
        /// The exact generation observed as terminal.
        terminal_generation: CoreGeneration,
        /// The engine the candidate must be.
        engine_id: EngineId,
        /// The fence token of the candidate admission.
        token: ActiveOwnerToken,
    },
}

/// Durable proof of the current Core owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "owner", rename_all = "snake_case")]
pub enum ActiveCoreOwnerProof {
    /// No owner is bound.
    Unowned,
    /// One owner is bound.
    Bound {
        /// Stable identity of the bound owner.
        identity: CoreIdentity,
        /// Exact generation the owner is bound to.
        generation: CoreGeneration,
        /// The owner fence token of the binding.
        token: ActiveOwnerToken,
    },
    /// The owner reached an exact terminal generation.
    Terminal {
        /// Stable identity of the terminal owner.
        identity: CoreIdentity,
        /// The generation the owner was bound to.
        bound_generation: CoreGeneration,
        /// The exact generation observed as terminal.
        terminal_generation: CoreGeneration,
        /// The owner fence token of the binding.
        token: ActiveOwnerToken,
    },
    /// A candidate owner admission is reserved.
    Reserved {
        /// The token that identifies the transition.
        transition_id: ActiveOwnerToken,
        /// The engine the candidate must be.
        engine_id: EngineId,
        /// The fence token of the candidate admission.
        token: ActiveOwnerToken,
    },
}

/// Durable evidence for the last managed Core process session associated with
/// an owner proof. Legacy records are explicit and cannot authorize recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "context", rename_all = "snake_case")]
pub enum ActiveCoreOwnerLaunchProof {
    /// No managed launch was ever recorded.
    Unstarted,
    /// The last launch used a managed process context.
    Managed(CoreProcessStartContext),
    /// The record predates the managed launch proof and cannot prove anything.
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

/// The durable owner-proof record as it is stored on disk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveCoreOwnerProofRecord {
    /// The schema version the record was written with.
    pub schema_version: u32,
    /// The durable owner proof.
    pub state: ActiveCoreOwnerProof,
    /// The durable managed-launch evidence.
    pub launch: ActiveCoreOwnerLaunchProof,
}

impl ActiveCoreOwnerProofRecord {
    /// Builds an unowned record in the current schema version.
    pub fn unowned() -> Self {
        Self {
            schema_version: ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
            state: ActiveCoreOwnerProof::Unowned,
            launch: ActiveCoreOwnerLaunchProof::Unstarted,
        }
    }

    /// Checks that the record uses the schema version this crate understands.
    ///
    /// Fails with an `Unsupported` port error for any other version; the
    /// record is never interpreted with guessed semantics.
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
