use super::changed_lease::ReferenceChangedLease;
use super::*;
use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::owner::transition_rejected;
use crate::owner_switch::registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches,
};
use lurker_core_contract::engine_switch::EngineSwitchStage;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, EngineId};
use lurker_core_ports::engine_switch::{
    EngineOwnerStoppedSwitchLease, EngineSwitchSelectionChangedLease,
};
use lurker_core_ports::error::PortError;
use std::marker::PhantomData;
use std::path::PathBuf;

pub struct ActiveOwnerStoppedSwitchLease<C: OwnerSwitchCodec> {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) owner_token: ActiveOwnerToken,
    pub(crate) stage: EngineSwitchStage<C::Selection>,
    pub(crate) codec: PhantomData<C>,
}

impl<C: OwnerSwitchCodec> ActiveOwnerStoppedSwitchLease<C> {
    pub fn new(
        config_root: impl Into<PathBuf>,
        transition_id: ActiveOwnerToken,
        owner_token: ActiveOwnerToken,
        stage: EngineSwitchStage<C::Selection>,
    ) -> Self {
        Self {
            config_root: config_root.into(),
            transition_id,
            owner_token,
            stage,
            codec: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<C: OwnerSwitchCodec> EngineOwnerStoppedSwitchLease<C::Selection>
    for ActiveOwnerStoppedSwitchLease<C>
{
    fn stage(&self) -> &EngineSwitchStage<C::Selection> {
        &self.stage
    }

    async fn compare_and_set_target(
        self: Box<Self>,
        selection: C::Selection,
        engine_id: EngineId,
    ) -> Result<Box<dyn EngineSwitchSelectionChangedLease<C::Selection>>, PortError> {
        let Self {
            config_root,
            transition_id,
            owner_token,
            stage,
            ..
        } = *self;
        let (
            previous_reference,
            owner_identity,
            owner_generation,
            terminal_generation,
            already_applied,
        ) = match stage {
            EngineSwitchStage::OldStopped {
                previous,
                owner_identity,
                owner_generation,
                terminal_generation,
            } => (
                previous,
                owner_identity,
                owner_generation,
                terminal_generation,
                false,
            ),
            // A recovered `SelectionChanged` journal is already past the CAS:
            // repeating the same target is idempotent and must not advance the
            // selection revision again.
            EngineSwitchStage::SelectionChanged {
                previous,
                selection: applied,
                owner_identity,
                terminal_generation,
            } => {
                if selection != applied {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::InvalidState,
                        "durable owner switch target differs from the retry request",
                        false,
                    )));
                }
                (
                    previous,
                    owner_identity,
                    CoreGeneration::new(0),
                    terminal_generation,
                    true,
                )
            }
            _ => return Err(transition_rejected("old owner terminal stage is missing")),
        };
        let Some(selected_engine) = C::selection_engine(&selection) else {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "an owner switch must select a concrete profile before binding a provider",
                false,
            )));
        };
        if selected_engine != &engine_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "owner switch selection must belong to the target engine",
                false,
            )));
        }
        if !already_applied {
            let target_revision = C::selection_revision(&previous_reference)
                .next()
                .map_err(|error| PortError::Failed(error.to_string()))?;
            if C::selection_revision(&selection) != target_revision {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "owner switch selection must advance exactly one revision",
                    false,
                )));
            }
        }
        let _root_guard = OwnerSwitchFile::<C>::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = OwnerSwitchFile::<C>::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record::<C>(&config_root)?;
        let (persisted_previous, persisted_owner, persisted_generation, persisted_token, stage) =
            switching_state::<C>(&record, &transition_id)?;
        let stage_matches = match &stage {
            ActiveOwnerSwitchStage::OldStopped {
                terminal_generation: persisted_terminal,
            } => !already_applied && *persisted_terminal == terminal_generation,
            ActiveOwnerSwitchStage::ReferenceChanged {
                terminal_generation: persisted_terminal,
            } => already_applied && *persisted_terminal == terminal_generation,
            ActiveOwnerSwitchStage::ClosingOld | ActiveOwnerSwitchStage::BindingNew { .. } => false,
        };
        let selection_matches = if already_applied {
            C::selection(&record) == &selection
        } else {
            C::selection(&record) == &previous_reference
        };
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || (!already_applied && persisted_generation != owner_generation)
            || persisted_token != owner_token
            || !selection_matches
            || !stage_matches
        {
            return Err(transition_rejected(
                "persisted old-owner terminal does not match the switch lease",
            ));
        }
        if !local_owner_registration_matches(
            &config_root,
            &owner_token,
            &LocalOwnerRegistrationState::Bound {
                identity: owner_identity.clone(),
                generation: persisted_generation,
            },
        )? {
            // The old owner lease must have been released after its terminal
            // readback; the local registry keeps unconfirmed drops fail-closed.
            let owner_lock =
                OwnerSwitchFile::<C>::new(config_root.clone()).try_acquire_owner_lock()?;
            if owner_lock.is_none() {
                return Err(transition_rejected(
                    "old Core owner lock is still held by another process",
                ));
            }
            drop(owner_lock);
        } else {
            return Err(transition_rejected(
                "old Core owner lease has not released its kernel fence",
            ));
        }
        let owner_proof = sync_read_active_core_owner_proof::<C>(&config_root)?;
        if !matches!(
            owner_proof.state,
            ActiveCoreOwnerProof::Terminal {
                ref identity,
                bound_generation,
                terminal_generation: proof_terminal_generation,
                ref token,
            } if identity.same_instance(&owner_identity)
                && bound_generation == persisted_generation
                && proof_terminal_generation == terminal_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "old owner terminal proof does not match the owner switch",
            ));
        }
        if !already_applied {
            C::set_selection(&mut record, selection.clone());
            C::set_owner(
                &mut record,
                ActiveCoreOwnerState::Switching {
                    transition_id: transition_id.clone(),
                    previous_reference: previous_reference.clone(),
                    owner_identity: owner_identity.clone(),
                    owner_generation: persisted_generation,
                    owner_token: owner_token.clone(),
                    stage: ActiveOwnerSwitchStage::ReferenceChanged {
                        terminal_generation,
                    },
                },
            );
            sync_publish_active_profile_record::<C>(&config_root, &record)?;
        }
        Ok(Box::new(ReferenceChangedLease::<C>::new(
            config_root,
            transition_id,
            owner_token,
            EngineSwitchStage::SelectionChanged {
                previous: previous_reference,
                selection,
                owner_identity,
                terminal_generation,
            },
        )))
    }
}
