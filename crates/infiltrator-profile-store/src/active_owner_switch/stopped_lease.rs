use super::*;

pub struct ActiveOwnerStoppedSwitchLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) owner_token: ActiveOwnerToken,
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
}

impl ActiveOwnerStoppedSwitchLease {
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
impl EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState> for ActiveOwnerStoppedSwitchLease {
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn compare_and_set_target(
        self: Box<Self>,
        selection: ActiveProfileReferenceState,
        engine_id: EngineId,
    ) -> Result<Box<dyn EngineSwitchSelectionChangedLease<ActiveProfileReferenceState>>, PortError>
    {
        let Self {
            config_root,
            transition_id,
            owner_token,
            stage,
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
        let selected_engine = match &selection {
            ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "an owner switch must select a concrete profile before binding a provider",
                    false,
                )));
            }
        };
        if selected_engine != engine_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "owner switch selection must belong to the target engine",
                false,
            )));
        }
        if !already_applied {
            let Some(target_revision) = previous_reference.revision().next() else {
                return Err(PortError::Failed(
                    "active-profile revision exhausted".into(),
                ));
            };
            if selection.revision() != target_revision {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "owner switch selection must advance exactly one revision",
                    false,
                )));
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
        let (persisted_previous, persisted_owner, persisted_generation, persisted_token, stage) =
            switching_state(&record, &transition_id)?;
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
            record.selection == selection
        } else {
            record.selection == previous_reference
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
                && bound_generation == persisted_generation
                && proof_terminal_generation == terminal_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "old owner terminal proof does not match the owner switch",
            ));
        }
        if !already_applied {
            record.selection = selection.clone();
            record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
            record.owner = ActiveCoreOwnerState::Switching {
                transition_id: transition_id.clone(),
                previous_reference: previous_reference.clone(),
                owner_identity: owner_identity.clone(),
                owner_generation: persisted_generation,
                owner_token: owner_token.clone(),
                stage: ActiveOwnerSwitchStage::ReferenceChanged {
                    terminal_generation,
                },
            };
            sync_publish_active_profile_record(&config_root, &record)?;
        }
        Ok(Box::new(ReferenceChangedLease::new(
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
