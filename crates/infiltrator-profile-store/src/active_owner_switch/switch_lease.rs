use super::*;

pub struct ActiveOwnerSwitchLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) owner_token: ActiveOwnerToken,
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
}

impl ActiveOwnerSwitchLease {
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
impl EngineOwnerSwitchLease<ActiveProfileReferenceState> for ActiveOwnerSwitchLease {
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let Self {
            config_root,
            transition_id,
            owner_token,
            stage,
        } = *self;
        let (previous_reference, owner_identity, owner_generation) = match stage {
            EngineSwitchStage::ClosingOld {
                previous,
                owner_identity,
                owner_generation,
            } => (previous, owner_identity, owner_generation),
            _ => {
                return Err(transition_rejected(
                    "only a ClosingOld owner switch can be canceled before close",
                ));
            }
        };

        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (persisted_previous, persisted_owner, persisted_generation, persisted_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || persisted_generation != owner_generation
            || persisted_token != owner_token
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
        stage_mapping::switch_snapshot(&record)
    }

    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>, PortError>
    {
        let Self {
            config_root,
            transition_id,
            owner_token,
            stage,
        } = *self;
        let (previous_reference, owner_identity, owner_generation) = match stage {
            EngineSwitchStage::ClosingOld {
                previous,
                owner_identity,
                owner_generation,
            } => (previous, owner_identity, owner_generation),
            _ => {
                return Err(transition_rejected(
                    "owner switch is not closing the old owner",
                ));
            }
        };
        verify_terminal_generation(terminal_generation, owner_generation)?;
        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (persisted_previous, persisted_owner, persisted_generation, persisted_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || persisted_generation != owner_generation
            || persisted_token != owner_token
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
                owner_token: owner_token.clone(),
                stage: ActiveOwnerSwitchStage::OldStopped {
                    terminal_generation,
                },
            };
            sync_publish_active_profile_record(&config_root, &record)?;
        }
        Ok(Box::new(ActiveOwnerStoppedSwitchLease::new(
            config_root,
            transition_id,
            owner_token,
            EngineSwitchStage::OldStopped {
                previous: previous_reference,
                owner_identity,
                owner_generation,
                terminal_generation,
            },
        )))
    }
}
