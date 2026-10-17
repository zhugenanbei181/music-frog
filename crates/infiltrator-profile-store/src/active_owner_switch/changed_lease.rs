use super::*;

pub struct ReferenceChangedLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) owner_token: ActiveOwnerToken,
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
}

impl ReferenceChangedLease {
    pub fn new(
        config_root: impl Into<PathBuf>,
        transition_id: ActiveOwnerToken,
        owner_token: ActiveOwnerToken,
        stage: EngineSwitchStage<ActiveProfileReferenceState>,
    ) -> Self {
        Self {
            config_root: config_root.into(),
            transition_id,
            owner_token,
            stage,
        }
    }
}

#[async_trait::async_trait]
impl EngineSwitchSelectionChangedLease<ActiveProfileReferenceState> for ReferenceChangedLease {
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
        candidate_identity: CoreIdentity,
        candidate_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerBindingLease<ActiveProfileReferenceState>>, PortError> {
        let Self {
            config_root,
            transition_id,
            owner_token,
            stage,
        } = *self;
        let (selection, previous_reference, owner_identity, terminal_generation) = match stage {
            EngineSwitchStage::SelectionChanged {
                previous,
                selection,
                owner_identity,
                terminal_generation,
            } => (selection, previous, owner_identity, terminal_generation),
            _ => return Err(transition_rejected("reference CAS stage is missing")),
        };
        let selected_engine = match &selection {
            ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(transition_rejected(
                    "an unselected reference cannot reserve a Core owner",
                ));
            }
        };
        if selected_engine != engine_id
            || candidate_identity.engine != engine_id
            || candidate_generation != CoreGeneration::new(0)
        {
            return Err(transition_rejected(format!(
                "selected engine {} cannot reserve candidate {} at generation {}",
                selected_engine.as_str(),
                candidate_identity.engine.as_str(),
                candidate_generation.value()
            )));
        }
        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (persisted_previous, persisted_owner, owner_generation, persisted_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || persisted_token != owner_token
            || record.selection != selection
            || !matches!(
                stage,
                ActiveOwnerSwitchStage::ReferenceChanged {
                    terminal_generation: persisted_terminal
                } if persisted_terminal == terminal_generation
            )
        {
            return Err(transition_rejected(
                "persisted active reference differs from the switch reservation",
            ));
        }
        let mut owner_proof = sync_read_active_core_owner_proof(&config_root)?;
        if !matches!(
            owner_proof.state,
            ActiveCoreOwnerProof::Terminal {
                ref identity,
                bound_generation,
                terminal_generation: proof_terminal,
                ref token,
            } if identity.same_instance(&owner_identity)
                && bound_generation == owner_generation
                && proof_terminal == terminal_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "old owner has no matching persisted terminal proof",
            ));
        }
        let owner_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .ok_or_else(|| {
                transition_rejected("an active Core owner still holds the owner fence")
            })?;
        let mut registrations = lock_local_owner_registry()?;
        if registrations.contains(&config_root) {
            return Err(transition_rejected(
                "this process already has an active Core owner registration",
            ));
        }
        let token = ActiveOwnerToken::generate()?;
        owner_proof.state = ActiveCoreOwnerProof::Reserved {
            transition_id: transition_id.clone(),
            engine_id: engine_id.clone(),
            token: token.clone(),
        };
        sync_publish_active_core_owner_proof(&config_root, &owner_proof)?;
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token: owner_token.clone(),
            stage: ActiveOwnerSwitchStage::BindingNew {
                terminal_generation,
                engine_id: engine_id.clone(),
                token: token.clone(),
            },
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        registrations.register_binding(
            config_root.clone(),
            token.clone(),
            transition_id.clone(),
            owner_guard,
        )?;
        Ok(Box::new(OwnerBindingLease::new(
            config_root,
            transition_id,
            token,
            EngineSwitchStage::BindingNew {
                selection,
                engine_id,
            },
        )))
    }
}
