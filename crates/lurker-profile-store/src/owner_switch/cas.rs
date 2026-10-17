//! Typed revision compare and owner-fenced selection transition rules.

use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::owner::transition_rejected;
use lurker_core_contract::engine_switch::EngineSwitchRevision;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_ports::error::PortError;

/// Current owner authorization projection derived while its owner-state lock
/// is held. This is a transient decision input, not a second persisted state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerSwitchFence {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
    Switching,
}

/// Proof that the current selection matched the caller's expected revision.
/// The product-root lock must remain held from construction through transition
/// and publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerSwitchCasReady<C: OwnerSwitchCodec> {
    previous: C::Selection,
    expected_revision: EngineSwitchRevision,
}

impl<C: OwnerSwitchCodec> OwnerSwitchCasReady<C> {
    pub fn compare(
        current: &C::Selection,
        expected_revision: EngineSwitchRevision,
    ) -> Result<Self, PortError> {
        if C::selection_revision(current) != expected_revision {
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

    pub fn previous(&self) -> &C::Selection {
        &self.previous
    }

    /// Validate the owner fence and that `target` advances exactly one
    /// revision, then return the committed selection.
    pub fn transition(
        self,
        target: C::Selection,
        owner: OwnerSwitchFence,
    ) -> Result<C::Selection, PortError> {
        match (&owner, C::selection_engine(&target)) {
            (OwnerSwitchFence::Unowned, _) => {}
            (OwnerSwitchFence::Bound { identity, .. }, Some(engine))
                if engine == &identity.engine => {}
            (OwnerSwitchFence::Bound { identity, .. }, None)
                if C::selection_engine(&self.previous)
                    .is_some_and(|engine| engine == &identity.engine) => {}
            (
                OwnerSwitchFence::Bound {
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
            (OwnerSwitchFence::Switching, _) => {
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
            .map_err(|error| PortError::Failed(error.to_string()))?;
        if C::selection_revision(&target) != revision {
            return Err(transition_rejected(
                "active profile reference selection did not advance exactly one revision",
            ));
        }
        Ok(target)
    }
}
