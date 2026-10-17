use super::*;

pub struct ReferenceChangedLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) state: ActiveProfileOwnerSwitchState,
}

impl ReferenceChangedLease {
    pub fn new(
        config_root: impl Into<PathBuf>,
        transition_id: ActiveOwnerToken,
        state: ActiveProfileOwnerSwitchState,
    ) -> Self {
        Self {
            config_root: config_root.into(),
            transition_id,
            state,
        }
    }
}

#[async_trait::async_trait]
impl ActiveProfileReferenceChangedLease for ReferenceChangedLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
    ) -> Result<Box<dyn ActiveProfileOwnerBindingLease>, PortError> {
        let Self {
            config_root,
            transition_id,
            state,
        } = *self;
        let (selection, previous_reference, owner_identity, terminal_generation) = match state {
            ActiveProfileOwnerSwitchState::ReferenceChanged {
                previous_reference,
                selection,
                owner_identity,
                terminal_generation,
            } => (
                selection,
                previous_reference,
                owner_identity,
                terminal_generation,
            ),
            _ => return Err(transition_rejected("reference CAS stage is missing")),
        };
        match &selection {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == engine_id => {}
            ActiveProfileReferenceState::Selected(reference) => {
                return Err(transition_rejected(format!(
                    "selected engine {} does not match requested owner engine {}",
                    reference.engine_id.as_str(),
                    engine_id.as_str()
                )));
            }
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(transition_rejected(
                    "an unselected reference cannot reserve a Core owner",
                ));
            }
        }
        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (persisted_previous, persisted_owner, owner_generation, owner_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
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
        let state = ActiveProfileOwnerSwitchState::BindingNew {
            selection,
            engine_id,
        };
        Ok(Box::new(OwnerBindingLease {
            config_root,
            transition_id,
            token,
            state,
        }))
    }
}
