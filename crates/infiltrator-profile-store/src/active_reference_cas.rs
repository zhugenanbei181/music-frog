//! Typed revision compare and owner-fenced active-reference transition rules.
//!
//! The revision/owner-fence CAS is the neutral
//! [`lurker_profile_store::owner_switch::cas`] machinery. This module keeps the
//! product target payload and the product evidence enum, and adapts the neutral
//! CAS to the product revision type.

use crate::active_reference_owner::ProductCodec;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget,
};
use lurker_core_contract::engine_switch::EngineSwitchRevision;
use lurker_core_contract::profile_document_identity::ProfileWorkspaceIdentity;
use lurker_core_ports::error::PortError;
use lurker_profile_store::owner_switch::cas::{OwnerSwitchCasReady, OwnerSwitchFence};
use lurker_profile_store::owner_switch::owner::ActiveCoreOwnerState;

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
    inner: OwnerSwitchCasReady<ProductCodec>,
    expected_revision: ActiveProfileReferenceRevision,
}

impl ActiveProfileReferenceCasReady {
    pub fn compare(
        current: &ActiveProfileReferenceState,
        expected_revision: ActiveProfileReferenceRevision,
    ) -> Result<Self, PortError> {
        Ok(Self {
            inner: OwnerSwitchCasReady::<ProductCodec>::compare(
                current,
                EngineSwitchRevision::new(expected_revision.value()),
            )?,
            expected_revision,
        })
    }

    pub fn previous(&self) -> &ActiveProfileReferenceState {
        self.inner.previous()
    }

    pub fn transition(
        self,
        target: ActiveProfileReferenceTarget,
        owner: OwnerSwitchFence,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let revision = self
            .expected_revision
            .next()
            .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
        let selection = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        self.inner.transition(selection, owner)
    }
}

/// The owner fence is also consumed by product recovery paths that reason in
/// terms of the same two owner facts.
pub fn owner_fence(owner: &ActiveCoreOwnerState<ActiveProfileReferenceState>) -> OwnerSwitchFence {
    match owner {
        ActiveCoreOwnerState::Unowned => OwnerSwitchFence::Unowned,
        ActiveCoreOwnerState::Bound {
            identity,
            generation,
            ..
        } => OwnerSwitchFence::Bound {
            identity: identity.clone(),
            generation: *generation,
        },
        ActiveCoreOwnerState::Switching { .. } => OwnerSwitchFence::Switching,
    }
}

#[cfg(test)]
#[path = "active_reference_cas_tests.rs"]
mod tests;
