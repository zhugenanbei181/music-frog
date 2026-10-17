use super::*;

pub struct ActiveOwnerSwitchLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) expected_owner: CoreIdentity,
    pub(crate) expected_generation: CoreGeneration,
    pub(crate) state: ActiveProfileOwnerSwitchState,
}

impl ActiveOwnerSwitchLease {
    pub fn new(
        config_root: impl Into<PathBuf>,
        transition_id: ActiveOwnerToken,
        expected_owner: CoreIdentity,
        expected_generation: CoreGeneration,
        state: ActiveProfileOwnerSwitchState,
    ) -> Self {
        Self {
            config_root: config_root.into(),
            transition_id,
            expected_owner,
            expected_generation,
            state,
        }
    }
}

#[async_trait::async_trait]
impl ActiveProfileOwnerSwitchLease for ActiveOwnerSwitchLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let Self {
            config_root,
            transition_id,
            expected_owner,
            expected_generation,
            state,
        } = *self;
        let (previous_reference, owner_identity, owner_generation) = match state {
            ActiveProfileOwnerSwitchState::ClosingOld {
                previous_reference,
                owner_identity,
                owner_generation,
            } => (previous_reference, owner_identity, owner_generation),
            _ => {
                return Err(transition_rejected(
                    "only a ClosingOld owner switch can be canceled before close",
                ));
            }
        };
        if !owner_identity.same_instance(&expected_owner) || owner_generation != expected_generation
        {
            return Err(transition_rejected("owner switch lease identity changed"));
        }

        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (persisted_previous, persisted_owner, persisted_generation, owner_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || persisted_generation != owner_generation
            || record.selection != previous_reference
            || !matches!(stage, ActiveOwnerSwitchStage::ClosingOld)
        {
            return Err(transition_rejected(
                "persisted ClosingOld reservation differs from cancellation request",
            ));
        }
        if !matches!(
            sync_read_active_core_owner_proof(&config_root)?.state,
            ActiveCoreOwnerProof::Bound {
                ref identity,
                generation,
                ref token,
            } if identity.same_instance(&owner_identity)
                && generation == owner_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "old owner has already released or changed its persisted binding",
            ));
        }
        let switching_registration = LocalOwnerRegistrationState::Switching {
            transition_id: transition_id.clone(),
            identity: owner_identity.clone(),
            generation: owner_generation,
        };
        let mut registrations = lock_local_owner_registry()?;
        if !registrations.matches(&config_root, &owner_token, &switching_registration) {
            return Err(transition_rejected(
                "only the process holding the old owner fence can cancel",
            ));
        }
        if let Some(owner_probe) =
            ActiveProfileReferenceFile::new(config_root.clone()).try_acquire_owner_lock()?
        {
            drop(owner_probe);
            return Err(transition_rejected(
                "old owner no longer holds the cross-process owner fence",
            ));
        }

        // Restore the in-process lease first. If publication fails, the
        // durable ClosingOld marker still blocks other selectors and recovery
        // can promote this exact Bound registration back to Switching.
        registrations.cancel_switch_before_close(
            &config_root,
            &owner_token,
            &transition_id,
            owner_identity.clone(),
            owner_generation,
        )?;
        record.owner = ActiveCoreOwnerState::Bound {
            identity: owner_identity,
            generation: owner_generation,
            token: owner_token,
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        Ok(record.selection)
    }

    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        let Self {
            config_root,
            transition_id,
            expected_owner,
            expected_generation,
            state,
        } = *self;
        let (previous_reference, owner_identity, owner_generation) = match state {
            ActiveProfileOwnerSwitchState::ClosingOld {
                previous_reference,
                owner_identity,
                owner_generation,
            } => (previous_reference, owner_identity, owner_generation),
            _ => {
                return Err(transition_rejected(
                    "owner switch is not closing the old owner",
                ));
            }
        };
        if !owner_identity.same_instance(&expected_owner) || owner_generation != expected_generation
        {
            return Err(transition_rejected("owner switch lease identity changed"));
        }
        let terminal_generation =
            validate_terminal_for_owner(terminal, &owner_identity, owner_generation)?;
        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (persisted_previous, persisted_owner, persisted_generation, owner_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || persisted_generation != owner_generation
        {
            return Err(transition_rejected(
                "persisted owner switch identity changed",
            ));
        }
        match &stage {
            ActiveOwnerSwitchStage::ClosingOld => {}
            ActiveOwnerSwitchStage::OldStopped {
                terminal_generation: persisted_generation,
            } if *persisted_generation == terminal_generation => {}
            ActiveOwnerSwitchStage::OldStopped { .. } => {
                return Err(transition_rejected(
                    "old owner terminal generation differs from the reserved readback",
                ));
            }
            ActiveOwnerSwitchStage::ReferenceChanged { .. }
            | ActiveOwnerSwitchStage::BindingNew { .. } => {
                return Err(transition_rejected(
                    "owner switch already advanced past old close",
                ));
            }
        }
        if local_owner_registration_matches(
            &config_root,
            &owner_token,
            &LocalOwnerRegistrationState::Switching {
                transition_id: transition_id.clone(),
                identity: owner_identity.clone(),
                generation: owner_generation,
            },
        )? {
            return Err(transition_rejected(
                "old Core owner kernel lease remains held after terminal readback",
            ));
        }
        let proof = sync_read_active_core_owner_proof(&config_root)?;
        if !matches!(
            proof.state,
            ActiveCoreOwnerProof::Terminal {
                ref identity,
                bound_generation,
                terminal_generation: proof_terminal_generation,
                ref token,
            } if identity.same_instance(&owner_identity)
                && bound_generation == owner_generation
                && proof_terminal_generation == terminal_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "old owner terminal proof does not match this switch reservation",
            ));
        }
        if local_owner_registration_state(&config_root, &owner_token)?.is_some() {
            return Err(transition_rejected(
                "old Core owner registration remains after terminal release",
            ));
        }
        if let Some(probe) =
            ActiveProfileReferenceFile::new(config_root.clone()).try_acquire_owner_lock()?
        {
            drop(probe);
        } else {
            return Err(transition_rejected(
                "another process still holds the active Core owner fence",
            ));
        }
        if matches!(stage, ActiveOwnerSwitchStage::ClosingOld) {
            record.owner = ActiveCoreOwnerState::Switching {
                transition_id: transition_id.clone(),
                previous_reference: previous_reference.clone(),
                owner_identity: owner_identity.clone(),
                owner_generation,
                owner_token,
                stage: ActiveOwnerSwitchStage::OldStopped {
                    terminal_generation,
                },
            };
            sync_publish_active_profile_record(&config_root, &record)?;
        }
        let state = ActiveProfileOwnerSwitchState::OldStopped {
            previous_reference,
            owner_identity,
            owner_generation,
            terminal_generation,
        };
        Ok(Box::new(ActiveOwnerStoppedSwitchLease {
            config_root,
            transition_id,
            state,
        }))
    }
}
