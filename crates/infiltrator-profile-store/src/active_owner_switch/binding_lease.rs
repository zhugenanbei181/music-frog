use super::*;

pub struct OwnerBindingLease {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) token: ActiveOwnerToken,
    pub(crate) state: ActiveProfileOwnerSwitchState,
}

impl OwnerBindingLease {
    pub fn new(
        config_root: impl Into<PathBuf>,
        transition_id: ActiveOwnerToken,
        token: ActiveOwnerToken,
        state: ActiveProfileOwnerSwitchState,
    ) -> Self {
        Self {
            config_root: config_root.into(),
            transition_id,
            token,
            state,
        }
    }
}

#[derive(Clone, Copy)]
enum CandidateDiscardAuthorization<'a> {
    BeforeCoreApplication,
    Stopped(&'a CoreLifecycleSnapshot),
}

async fn clear_candidate_owner_reservation(
    config_root: PathBuf,
    transition_id: ActiveOwnerToken,
    token: ActiveOwnerToken,
    selection: ActiveProfileReferenceState,
    engine_id: EngineId,
    authorization: CandidateDiscardAuthorization<'_>,
) -> Result<ActiveProfileReferenceState, PortError> {
    match authorization {
        CandidateDiscardAuthorization::BeforeCoreApplication => {}
        CandidateDiscardAuthorization::Stopped(terminal) => {
            let identity = terminal
                .identity
                .as_ref()
                .ok_or_else(|| transition_rejected("candidate terminal has no Core identity"))?;
            if identity.engine != engine_id
                || terminal.lifecycle != CoreLifecycle::Stopped
                || terminal.generation != 0
                || terminal.session_token.is_some()
            {
                return Err(transition_rejected(
                    "candidate can be discarded only after a never-started Stopped readback without a session",
                ));
            }
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
    let (old_owner_identity, old_owner_generation, old_owner_token, old_terminal_generation) =
        match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id: persisted_id,
                owner_identity,
                owner_generation,
                owner_token,
                stage:
                    ActiveOwnerSwitchStage::BindingNew {
                        engine_id: persisted_engine,
                        token: persisted_token,
                        terminal_generation,
                    },
                ..
            } if persisted_id == &transition_id
                && persisted_engine == &engine_id
                && persisted_token == &token =>
            {
                (
                    owner_identity.clone(),
                    *owner_generation,
                    owner_token.clone(),
                    *terminal_generation,
                )
            }
            _ => return Err(transition_rejected("new owner reservation is stale")),
        };
    if record.selection != selection
        || !local_owner_registration_matches(
            &config_root,
            &token,
            &LocalOwnerRegistrationState::Binding {
                transition_id: transition_id.clone(),
            },
        )?
    {
        return Err(transition_rejected(
            "candidate owner reservation does not match the persisted switch",
        ));
    }
    let mut proof = sync_read_active_core_owner_proof(&config_root)?;
    match (&proof.state, authorization) {
        (
            ActiveCoreOwnerProof::Reserved {
                transition_id: persisted_id,
                engine_id: persisted_engine,
                token: persisted_token,
            },
            _,
        ) if persisted_id == &transition_id
            && persisted_engine == &engine_id
            && persisted_token == &token => {}
        (
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            },
            CandidateDiscardAuthorization::Stopped(terminal),
        ) if terminal
            .identity
            .as_ref()
            .is_some_and(|identity| proof_identity.same_instance(identity))
            && proof_identity.engine == engine_id
            && *proof_generation == CoreGeneration::new(terminal.generation)
            && *proof_generation == CoreGeneration::new(0)
            && proof_token == &token => {}
        (
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_bound_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            },
            _,
        ) if proof_identity.same_instance(&old_owner_identity)
            && *proof_bound_generation == old_owner_generation
            && *proof_terminal_generation == old_terminal_generation
            && proof_token == &old_owner_token => {}
        _ => {
            return Err(transition_rejected(
                "candidate owner process reservation is stale",
            ));
        }
    }
    if let Some(probe) =
        ActiveProfileReferenceFile::new(config_root.clone()).try_acquire_owner_lock()?
    {
        drop(probe);
        return Err(transition_rejected(
            "candidate owner kernel admission was lost",
        ));
    }
    // Restore the exact old terminal proof before clearing the root stage.
    // A crash between these writes therefore leaves a BindingNew marker that
    // can still request candidate terminal evidence; it never leaves an
    // unowned root with an unproven Reserved/Bound owner proof.
    proof.state = ActiveCoreOwnerProof::Terminal {
        identity: old_owner_identity,
        bound_generation: old_owner_generation,
        terminal_generation: old_terminal_generation,
        token: old_owner_token,
    };
    sync_publish_active_core_owner_proof(&config_root, &proof)?;
    record.owner = ActiveCoreOwnerState::Unowned;
    sync_publish_active_profile_record(&config_root, &record)?;
    remove_local_owner_registration(&config_root, &token)?;
    Ok(record.selection)
}

#[async_trait::async_trait]
impl ActiveProfileOwnerBindingLease for OwnerBindingLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        let Self {
            config_root,
            transition_id,
            token,
            state,
        } = *self;
        let (selection, engine_id) = match state {
            ActiveProfileOwnerSwitchState::BindingNew {
                selection,
                engine_id,
            } => (selection, engine_id),
            _ => {
                return Err(transition_rejected(
                    "new owner binding reservation is missing",
                ));
            }
        };
        if identity.engine != engine_id {
            return Err(transition_rejected(
                "new Core identity does not match the reserved engine",
            ));
        }
        let _root_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let stage_token = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id: persisted_id,
                stage:
                    ActiveOwnerSwitchStage::BindingNew {
                        engine_id: persisted_engine,
                        token: persisted_token,
                        ..
                    },
                ..
            } if persisted_id == &transition_id
                && persisted_engine == &engine_id
                && persisted_token == &token =>
            {
                persisted_token.clone()
            }
            _ => return Err(transition_rejected("new owner reservation is stale")),
        };
        if record.selection != selection || stage_token != token {
            return Err(transition_rejected(
                "active reference changed after the new owner was reserved",
            ));
        }
        if local_owner_registration_matches(
            &config_root,
            &token,
            &LocalOwnerRegistrationState::Binding {
                transition_id: transition_id.clone(),
            },
        )? == false
        {
            return Err(transition_rejected(
                "new owner kernel admission is no longer held by this process",
            ));
        }
        let mut owner_proof = sync_read_active_core_owner_proof(&config_root)?;
        if !matches!(
            &owner_proof.state,
            ActiveCoreOwnerProof::Reserved {
                transition_id: persisted_id,
                engine_id: persisted_engine,
                token: persisted_token,
            } if persisted_id == &transition_id
                && persisted_engine == &engine_id
                && persisted_token == &token
        ) {
            return Err(transition_rejected(
                "new owner process reservation is stale",
            ));
        }
        if let Some(probe) =
            ActiveProfileReferenceFile::new(config_root.clone()).try_acquire_owner_lock()?
        {
            drop(probe);
            return Err(transition_rejected("new owner kernel admission was lost"));
        }
        owner_proof.state = ActiveCoreOwnerProof::Bound {
            identity: identity.clone(),
            generation,
            token: token.clone(),
        };
        owner_proof.launch = ActiveCoreOwnerLaunchProof::Unstarted;
        sync_publish_active_core_owner_proof(&config_root, &owner_proof)?;
        record.owner = ActiveCoreOwnerState::Bound {
            identity: identity.clone(),
            generation,
            token: token.clone(),
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        {
            let mut registrations = lock_local_owner_registry()?;
            let binding_state = LocalOwnerRegistrationState::Binding {
                transition_id: transition_id.clone(),
            };
            if !registrations.matches(&config_root, &token, &binding_state) {
                return Err(transition_rejected("new owner registration is stale"));
            }
            registrations.finalize_binding(
                &config_root,
                &token,
                &transition_id,
                identity.clone(),
                generation,
            )?;
        }
        Ok(Box::new(ActiveCoreOwnerLease::new(
            identity,
            generation,
            token,
            config_root,
        )))
    }

    async fn discard_unstarted_candidate(
        self: Box<Self>,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let Self {
            config_root,
            transition_id,
            token,
            state,
        } = *self;
        let (selection, engine_id) = match state {
            ActiveProfileOwnerSwitchState::BindingNew {
                selection,
                engine_id,
            } => (selection, engine_id),
            _ => {
                return Err(transition_rejected(
                    "new owner binding reservation is missing",
                ));
            }
        };
        clear_candidate_owner_reservation(
            config_root,
            transition_id,
            token,
            selection,
            engine_id,
            CandidateDiscardAuthorization::Stopped(terminal),
        )
        .await
    }

    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let Self {
            config_root,
            transition_id,
            token,
            state,
        } = *self;
        let (selection, engine_id) = match state {
            ActiveProfileOwnerSwitchState::BindingNew {
                selection,
                engine_id,
            } => (selection, engine_id),
            _ => {
                return Err(transition_rejected(
                    "new owner binding reservation is missing",
                ));
            }
        };
        clear_candidate_owner_reservation(
            config_root,
            transition_id,
            token,
            selection,
            engine_id,
            CandidateDiscardAuthorization::BeforeCoreApplication,
        )
        .await
    }
}
