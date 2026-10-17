//! Total mapping between the persisted owner-switch wire record and the
//! neutral engine-switch vocabulary.
//!
//! The persisted enum [`ActiveOwnerSwitchStage`] is the only wire format: its
//! serde tags and field names must never change. Orchestration reads and
//! compares [`EngineSwitchStage`], which carries the previous/current selection
//! and the exact owner fence. Every field the neutral stage does not model (the
//! transition token, the owner token, the candidate binding token/terminal and,
//! for `BindingNew`, the old-owner fence) is retained by
//! [`PersistedSwitchWire`] so the inverse mapping reproduces the record.
//!
//! The journal revision is the revision of the persisted selection. It advances
//! exactly when the selection changes, which is the only mutation the neutral
//! journal compares against.

use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::owner::{ActiveCoreOwnerState, ActiveOwnerSwitchStage, ActiveOwnerToken};
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRevision, EngineSwitchSnapshot, EngineSwitchStage,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_ports::error::PortError;

/// Wire-only fields the neutral stage vocabulary does not model.
///
/// They are read from an existing persisted record and written back unchanged.
/// `previous_reference`, `owner_identity`, `owner_generation`, `binding_*` only
/// matter for `BindingNew`, because every earlier neutral stage still carries
/// the previous selection and the exact owner fence itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedSwitchWire<C: OwnerSwitchCodec> {
    pub transition_id: ActiveOwnerToken,
    pub owner_token: ActiveOwnerToken,
    pub previous_reference: C::Selection,
    pub owner_identity: CoreIdentity,
    pub owner_generation: CoreGeneration,
    pub binding_terminal_generation: CoreGeneration,
    pub binding_token: ActiveOwnerToken,
}

/// Read one atomic snapshot of the persisted record as a neutral snapshot.
pub fn switch_snapshot<C: OwnerSwitchCodec>(
    record: &C::Record,
) -> Result<EngineSwitchSnapshot<C::Selection>, PortError> {
    let selection = C::selection(record);
    Ok(EngineSwitchSnapshot {
        revision: C::selection_revision(selection),
        selection: selection.clone(),
        owner: owner_state(C::owner(record), selection)?,
    })
}

/// Map the persisted owner state, using `selection` for the stages that keep
/// the committed selection in the record rather than in the stage.
pub fn owner_state<S: Clone>(
    owner: &ActiveCoreOwnerState<S>,
    selection: &S,
) -> Result<EngineOwnerState<S>, PortError> {
    match owner {
        ActiveCoreOwnerState::Unowned => Ok(EngineOwnerState::Unowned),
        ActiveCoreOwnerState::Bound {
            identity,
            generation,
            ..
        } => Ok(EngineOwnerState::Bound {
            identity: identity.clone(),
            generation: *generation,
        }),
        ActiveCoreOwnerState::Switching {
            previous_reference,
            owner_identity,
            owner_generation,
            stage,
            ..
        } => Ok(EngineOwnerState::Switching {
            stage: switch_stage(
                stage,
                previous_reference,
                selection,
                owner_identity,
                *owner_generation,
            ),
        }),
    }
}

/// Map one persisted stage. The `Switching` wrapper owns the previous
/// selection and the exact owner fence; the stage owns the progress.
pub fn switch_stage<S: Clone>(
    stage: &ActiveOwnerSwitchStage,
    previous: &S,
    selection: &S,
    owner_identity: &CoreIdentity,
    owner_generation: CoreGeneration,
) -> EngineSwitchStage<S> {
    match stage {
        ActiveOwnerSwitchStage::ClosingOld => EngineSwitchStage::ClosingOld {
            previous: previous.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
        },
        ActiveOwnerSwitchStage::OldStopped {
            terminal_generation,
        } => EngineSwitchStage::OldStopped {
            previous: previous.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            terminal_generation: *terminal_generation,
        },
        ActiveOwnerSwitchStage::ReferenceChanged {
            terminal_generation,
        } => EngineSwitchStage::SelectionChanged {
            previous: previous.clone(),
            selection: selection.clone(),
            owner_identity: owner_identity.clone(),
            terminal_generation: *terminal_generation,
        },
        ActiveOwnerSwitchStage::BindingNew { engine_id, .. } => EngineSwitchStage::BindingNew {
            selection: selection.clone(),
            engine_id: engine_id.clone(),
        },
    }
}

/// Wire-only fields of an existing persisted switching record.
pub fn persisted_wire<C: OwnerSwitchCodec>(
    record: &C::Record,
) -> Result<PersistedSwitchWire<C>, PortError> {
    match C::owner(record) {
        ActiveCoreOwnerState::Switching {
            transition_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage:
                ActiveOwnerSwitchStage::BindingNew {
                    terminal_generation,
                    token,
                    ..
                },
        } => Ok(PersistedSwitchWire {
            transition_id: transition_id.clone(),
            owner_token: owner_token.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation: *owner_generation,
            binding_terminal_generation: *terminal_generation,
            binding_token: token.clone(),
        }),
        ActiveCoreOwnerState::Switching {
            transition_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            ..
        } => Ok(PersistedSwitchWire {
            transition_id: transition_id.clone(),
            owner_token: owner_token.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation: *owner_generation,
            binding_terminal_generation: CoreGeneration::new(0),
            binding_token: owner_token.clone(),
        }),
        ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => Err(
            PortError::Failed("active Core owner is not switching".to_owned()),
        ),
    }
}

/// Map a neutral owner state back to the persisted wire enum.
///
/// `wire` supplies the fields the neutral vocabulary deliberately omits. The
/// resulting value serializes to exactly the same JSON as the record it was
/// read from.
pub fn persisted_owner_state<C: OwnerSwitchCodec>(
    owner: &EngineOwnerState<C::Selection>,
    wire: &PersistedSwitchWire<C>,
) -> Result<ActiveCoreOwnerState<C::Selection>, PortError> {
    match owner {
        EngineOwnerState::Unowned => Ok(ActiveCoreOwnerState::Unowned),
        EngineOwnerState::Bound {
            identity,
            generation,
        } => Ok(ActiveCoreOwnerState::Bound {
            identity: identity.clone(),
            generation: *generation,
            token: wire.owner_token.clone(),
        }),
        EngineOwnerState::Switching { stage } => {
            let (previous_reference, owner_identity, owner_generation, stage) =
                persisted_stage(stage, wire);
            Ok(ActiveCoreOwnerState::Switching {
                transition_id: wire.transition_id.clone(),
                previous_reference,
                owner_identity,
                owner_generation,
                owner_token: wire.owner_token.clone(),
                stage,
            })
        }
    }
}

fn persisted_stage<C: OwnerSwitchCodec>(
    stage: &EngineSwitchStage<C::Selection>,
    wire: &PersistedSwitchWire<C>,
) -> (
    C::Selection,
    CoreIdentity,
    CoreGeneration,
    ActiveOwnerSwitchStage,
) {
    match stage {
        EngineSwitchStage::ClosingOld {
            previous,
            owner_identity,
            owner_generation,
        } => (
            previous.clone(),
            owner_identity.clone(),
            *owner_generation,
            ActiveOwnerSwitchStage::ClosingOld,
        ),
        EngineSwitchStage::OldStopped {
            previous,
            owner_identity,
            owner_generation,
            terminal_generation,
        } => (
            previous.clone(),
            owner_identity.clone(),
            *owner_generation,
            ActiveOwnerSwitchStage::OldStopped {
                terminal_generation: *terminal_generation,
            },
        ),
        EngineSwitchStage::SelectionChanged {
            previous,
            owner_identity,
            terminal_generation,
            ..
        } => (
            previous.clone(),
            owner_identity.clone(),
            wire.owner_generation,
            ActiveOwnerSwitchStage::ReferenceChanged {
                terminal_generation: *terminal_generation,
            },
        ),
        EngineSwitchStage::BindingNew { engine_id, .. } => (
            wire.previous_reference.clone(),
            wire.owner_identity.clone(),
            wire.owner_generation,
            ActiveOwnerSwitchStage::BindingNew {
                terminal_generation: wire.binding_terminal_generation,
                engine_id: engine_id.clone(),
                token: wire.binding_token.clone(),
            },
        ),
    }
}

/// Convenience: the neutral revision of one selection payload.
pub fn selection_revision<C: OwnerSwitchCodec>(selection: &C::Selection) -> EngineSwitchRevision {
    C::selection_revision(selection)
}
