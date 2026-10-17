//! Typed revision compare and owner-fenced active-reference transition rules.

use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget,
};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_ports::error::PortError;

/// Current owner authorization projection derived while its owner-state lock
/// is held. This is a transient decision input, not a second persisted state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceOwnerFence {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
    Switching,
}

/// Evidence that the target document belongs to this engine-scoped store.
/// A foreign-engine target is validated by that engine's own store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceTargetEvidence {
    Unselected,
    OtherEngine,
    LocalWorkspace(ProfileWorkspaceIdentity),
}

/// Proof that the current reference matched the caller's expected revision.
/// The product-root lock must remain held from construction through transition
/// and publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveProfileReferenceCasReady {
    previous: ActiveProfileReferenceState,
    expected_revision: ActiveProfileReferenceRevision,
}

impl ActiveProfileReferenceCasReady {
    pub fn compare(
        current: &ActiveProfileReferenceState,
        expected_revision: ActiveProfileReferenceRevision,
    ) -> Result<Self, PortError> {
        if current.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        Ok(Self {
            previous: current.clone(),
            expected_revision,
        })
    }

    pub fn previous(&self) -> &ActiveProfileReferenceState {
        &self.previous
    }

    pub fn transition(
        self,
        target: ActiveProfileReferenceTarget,
        owner: ActiveProfileReferenceOwnerFence,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        match (&owner, &target) {
            (ActiveProfileReferenceOwnerFence::Unowned, _) => {}
            (
                ActiveProfileReferenceOwnerFence::Bound { identity, .. },
                ActiveProfileReferenceTarget::Unselected,
            ) if matches!(
                &self.previous,
                ActiveProfileReferenceState::Selected(reference)
                    if reference.engine_id == identity.engine
            ) => {}
            (
                ActiveProfileReferenceOwnerFence::Bound { identity, .. },
                ActiveProfileReferenceTarget::Selected { engine_id, .. },
            ) if engine_id == &identity.engine => {}
            (
                ActiveProfileReferenceOwnerFence::Bound {
                    identity,
                    generation,
                },
                _,
            ) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!(
                        "active Core {} at generation {} must reach terminal before changing its engine selection",
                        identity.instance.as_str(),
                        generation.value()
                    ),
                    true,
                )));
            }
            (ActiveProfileReferenceOwnerFence::Switching, _) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "an active Core owner switch is in progress",
                    true,
                )));
            }
        }

        let revision = self
            .expected_revision
            .next()
            .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
        Ok(match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        })
    }
}

#[cfg(test)]
#[path = "active_reference_cas_tests.rs"]
mod tests;
