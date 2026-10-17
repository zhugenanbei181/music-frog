use super::*;

pub struct ActiveOwnerStoppedSwitchLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) state: ActiveProfileOwnerSwitchState,
}

impl ActiveOwnerStoppedSwitchLease {
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
impl ActiveProfileOwnerStoppedSwitchLease for ActiveOwnerStoppedSwitchLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn compare_and_set_target(
        self: Box<Self>,
        target: ActiveProfileReferenceTarget,
    ) -> Result<Box<dyn ActiveProfileReferenceChangedLease>, PortError> {
        let Self {
            config_root,
            transition_id,
            state,
        } = *self;
        let (previous_reference, owner_identity, owner_generation, terminal_generation) =
            match state {
                ActiveProfileOwnerSwitchState::OldStopped {
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                } => (
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                ),
                _ => return Err(transition_rejected("old owner terminal stage is missing")),
            };
        let (target_engine, target_profile) = match target {
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => (engine_id, profile_id),
            ActiveProfileReferenceTarget::Unselected => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "an owner switch must select a concrete profile before binding a provider",
                    false,
                )));
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
        let (persisted_previous, persisted_owner, persisted_generation, owner_token, stage) =
            switching_state(&record, &transition_id)?;
        if persisted_previous != previous_reference
            || !persisted_owner.same_instance(&owner_identity)
            || persisted_generation != owner_generation
            || record.selection != previous_reference
            || !matches!(
                stage,
                ActiveOwnerSwitchStage::OldStopped {
                    terminal_generation: persisted_terminal
                } if persisted_terminal == terminal_generation
            )
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
                generation: owner_generation,
            },
        )? {
            // The old owner lease must have been released after its terminal
            // readback; the local registry keeps unconfirmed drops fail-closed.
            let owner_lock =
                ActiveProfileReferenceFile::new(config_root.clone()).try_acquire_owner_lock()?;
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
        let owner_proof = sync_read_active_core_owner_proof(&config_root)?;
        if !matches!(
            owner_proof.state,
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
                "old owner terminal proof does not match the owner switch",
            ));
        }
        let revision = previous_reference
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("active-profile revision exhausted".into()))?;
        record.selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            target_engine,
            target_profile,
            revision,
        ));
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            owner_token,
            stage: ActiveOwnerSwitchStage::ReferenceChanged {
                terminal_generation,
            },
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        let state = ActiveProfileOwnerSwitchState::ReferenceChanged {
            previous_reference,
            selection: record.selection.clone(),
            owner_identity,
            terminal_generation,
        };
        Ok(Box::new(ReferenceChangedLease {
            config_root,
            transition_id,
            state,
        }))
    }
}
