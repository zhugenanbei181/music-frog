//! Durable owner-switch lease transitions shared by all engine stores.

use crate::active_owner_lease::ActiveCoreOwnerLease;
use crate::active_owner_registry::{
    LocalOwnerRegistrationState, LocalSwitchRegistrationRecovery,
    local_owner_registration_matches as local_registration_matches, local_owner_registration_state,
    lock_local_owner_registry, remove_local_owner_registration,
};
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveCoreOwnerProofRecord,
    ActiveCoreOwnerState, ActiveOwnerSwitchStage, ActiveOwnerToken, ActiveProfileReferenceRecord,
    LegacySelectionMigration, transition_rejected,
};
use crate::active_reference_owner_coordinator::{
    ActiveProfileOwnerCoordinator, validate_terminal_for_owner,
};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerBindingLease, ActiveProfileOwnerLease, ActiveProfileOwnerStoppedSwitchLease,
    ActiveProfileOwnerSwitchLease, ActiveProfileOwnerSwitchRecovery, ActiveProfileOwnerSwitchState,
    ActiveProfileReferenceChangedLease,
};
use infiltrator_ports::error::PortError;
use std::path::{Path, PathBuf};

fn sync_read_active_profile_record(
    config_root: &Path,
) -> Result<ActiveProfileReferenceRecord, PortError> {
    Ok(ActiveProfileReferenceFile::new(config_root)
        .read_active_profile_record_sync()?
        .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration))
}

fn sync_read_active_core_owner_proof(
    config_root: &Path,
) -> Result<ActiveCoreOwnerProofRecord, PortError> {
    ActiveProfileReferenceFile::new(config_root).read_owner_proof_sync()
}

fn sync_publish_active_core_owner_proof(
    config_root: &Path,
    record: &ActiveCoreOwnerProofRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root).publish_owner_proof_sync_reconciled(record)
}

fn sync_publish_active_profile_record(
    config_root: &Path,
    record: &ActiveProfileReferenceRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root)
        .publish_active_profile_record_sync_reconciled(record)
}

pub fn switching_state(
    record: &ActiveProfileReferenceRecord,
    transition_id: &ActiveOwnerToken,
) -> Result<
    (
        ActiveProfileReferenceState,
        CoreIdentity,
        CoreGeneration,
        ActiveOwnerToken,
        ActiveOwnerSwitchStage,
    ),
    PortError,
> {
    match &record.owner {
        ActiveCoreOwnerState::Switching {
            transition_id: persisted_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
        } if persisted_id == transition_id => Ok((
            previous_reference.clone(),
            owner_identity.clone(),
            *owner_generation,
            owner_token.clone(),
            stage.clone(),
        )),
        _ => Err(transition_rejected(
            "active Core owner switch reservation is stale or no longer present",
        )),
    }
}

pub fn terminal_proof_generation(
    proof: &ActiveCoreOwnerProofRecord,
    identity: &CoreIdentity,
    bound_generation: CoreGeneration,
    token: &ActiveOwnerToken,
) -> Result<CoreGeneration, PortError> {
    match &proof.state {
        ActiveCoreOwnerProof::Terminal {
            identity: proof_identity,
            bound_generation: proof_bound_generation,
            terminal_generation,
            token: proof_token,
        } if proof_identity.same_instance(identity)
            && *proof_bound_generation == bound_generation
            && proof_token == token
            && *terminal_generation >= bound_generation =>
        {
            Ok(*terminal_generation)
        }
        _ => Err(transition_rejected(
            "persisted owner terminal proof does not match the switch reservation",
        )),
    }
}

pub fn recover_local_switch_registration(
    config_root: &Path,
    token: &ActiveOwnerToken,
    transition_id: &ActiveOwnerToken,
    identity: &CoreIdentity,
    generation: CoreGeneration,
) -> Result<LocalSwitchRegistrationRecovery, PortError> {
    let mut registrations = lock_local_owner_registry()?;
    Ok(registrations.recover_switch(config_root, token, transition_id, identity, generation))
}

pub struct ActiveOwnerSwitchLease {
    config_root: PathBuf,
    transition_id: ActiveOwnerToken,
    expected_owner: CoreIdentity,
    expected_generation: CoreGeneration,
    state: ActiveProfileOwnerSwitchState,
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
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
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
        if local_registration_matches(
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

pub struct ActiveOwnerStoppedSwitchLease {
    config_root: PathBuf,
    transition_id: ActiveOwnerToken,
    state: ActiveProfileOwnerSwitchState,
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
        if !local_registration_matches(
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

pub struct ReferenceChangedLease {
    config_root: PathBuf,
    transition_id: ActiveOwnerToken,
    state: ActiveProfileOwnerSwitchState,
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

pub struct OwnerBindingLease {
    config_root: PathBuf,
    transition_id: ActiveOwnerToken,
    token: ActiveOwnerToken,
    state: ActiveProfileOwnerSwitchState,
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
    Stopped(&'a infiltrator_contract::snapshot::CoreLifecycleSnapshot),
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
                || terminal.lifecycle != infiltrator_contract::snapshot::CoreLifecycle::Stopped
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
        || !local_registration_matches(
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
        if local_registration_matches(
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
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
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

impl ActiveProfileOwnerCoordinator {
    /// Begin a switch while the caller holds the root fence and has checked
    /// the product reset fence.
    pub async fn begin_owner_switch_while_root_locked(
        &self,
        root_guard: ThreadedInstanceLockGuard,
        expected_revision: infiltrator_contract::active_profile_reference::ActiveProfileReferenceRevision,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerSwitchLease>, PortError> {
        let _root_guard = root_guard;
        let config_root = self.config_root().to_path_buf();
        let mut record = sync_read_active_profile_record(&config_root)?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        if record.selection.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        let (owner_identity, owner_generation, owner_token) = match &record.owner {
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                token,
            } if identity.same_instance(&expected_owner)
                && *generation == expected_binding_generation =>
            {
                (identity.clone(), *generation, token.clone())
            }
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                ..
            } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!(
                        "active Core owner {}/{} at generation {} does not match switch request",
                        identity.engine.as_str(),
                        identity.instance.as_str(),
                        generation.value()
                    ),
                    true,
                )));
            }
            ActiveCoreOwnerState::Unowned => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "owner switching requires a currently bound Core owner",
                    true,
                )));
            }
            ActiveCoreOwnerState::Switching { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "an active Core owner switch is already in progress",
                    true,
                )));
            }
        };
        if ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "persisted owner binding has no live kernel owner lock",
                true,
            )));
        }
        let proof = sync_read_active_core_owner_proof(&config_root)?;
        if !matches!(
            proof.state,
            ActiveCoreOwnerProof::Bound {
                ref identity,
                generation,
                ref token,
            } if identity.same_instance(&owner_identity)
                && generation == owner_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "persisted owner proof does not match the active owner binding",
            ));
        }
        if !local_registration_matches(
            &config_root,
            &owner_token,
            &LocalOwnerRegistrationState::Bound {
                identity: owner_identity.clone(),
                generation: owner_generation,
            },
        )? {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "only the process holding the old Core owner can reserve its switch",
                true,
            )));
        }
        let transition_id = ActiveOwnerToken::generate()?;
        let previous_reference = record.selection.clone();
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            owner_token: owner_token.clone(),
            stage: ActiveOwnerSwitchStage::ClosingOld,
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        lock_local_owner_registry()?.begin_switching(
            &config_root,
            &owner_token,
            transition_id.clone(),
            owner_identity.clone(),
            owner_generation,
        )?;
        let state = ActiveProfileOwnerSwitchState::ClosingOld {
            previous_reference,
            owner_identity,
            owner_generation,
        };
        Ok(Box::new(ActiveOwnerSwitchLease::new(
            config_root,
            transition_id,
            expected_owner,
            expected_binding_generation,
            state,
        )))
    }
}

impl ActiveProfileOwnerCoordinator {
    pub async fn recover_owner_switch(
        &self,
    ) -> Result<ActiveProfileOwnerSwitchRecovery, PortError> {
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (
            transition_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
        ) = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id,
                previous_reference,
                owner_identity,
                owner_generation,
                owner_token,
                stage,
            } => (
                transition_id.clone(),
                previous_reference.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                stage.clone(),
            ),
            ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
                return Ok(ActiveProfileOwnerSwitchRecovery::NoPendingSwitch);
            }
        };
        match stage {
            ActiveOwnerSwitchStage::ClosingOld => {
                match ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?
                {
                    None => {
                        let local_recovery = recover_local_switch_registration(
                            &config_root,
                            &owner_token,
                            &transition_id,
                            &owner_identity,
                            owner_generation,
                        )?;
                        if !matches!(
                            local_recovery,
                            LocalSwitchRegistrationRecovery::NotHeldByThisOwner
                        ) {
                            let state = ActiveProfileOwnerSwitchState::ClosingOld {
                                previous_reference,
                                owner_identity: owner_identity.clone(),
                                owner_generation,
                            };
                            return Ok(ActiveProfileOwnerSwitchRecovery::ResumeClosingOld(
                                Box::new(ActiveOwnerSwitchLease::new(
                                    config_root,
                                    transition_id,
                                    owner_identity,
                                    owner_generation,
                                    state,
                                )),
                            ));
                        }
                        Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                            identity: owner_identity,
                            generation: owner_generation,
                        })
                    }
                    Some(owner_probe) => {
                        drop(owner_probe);
                        let proof = sync_read_active_core_owner_proof(&config_root)?;
                        match terminal_proof_generation(
                            &proof,
                            &owner_identity,
                            owner_generation,
                            &owner_token,
                        ) {
                            Ok(terminal_generation) => {
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
                                let state = ActiveProfileOwnerSwitchState::OldStopped {
                                    previous_reference,
                                    owner_identity,
                                    owner_generation,
                                    terminal_generation,
                                };
                                Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                                    Box::new(ActiveOwnerStoppedSwitchLease::new(
                                        config_root,
                                        transition_id,
                                        state,
                                    )),
                                ))
                            }
                            Err(_) => Ok(
                                ActiveProfileOwnerSwitchRecovery::OwnerTerminalEvidenceRequired {
                                    identity: owner_identity,
                                    bound_generation: owner_generation,
                                },
                            ),
                        }
                    }
                }
            }
            ActiveOwnerSwitchStage::OldStopped {
                terminal_generation,
            } => {
                let proof = sync_read_active_core_owner_proof(&config_root)?;
                let proven_generation = terminal_proof_generation(
                    &proof,
                    &owner_identity,
                    owner_generation,
                    &owner_token,
                )?;
                if proven_generation != terminal_generation {
                    return Err(transition_rejected(
                        "persisted old-owner terminal generation differs from its proof",
                    ));
                }
                if ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?
                    .is_none()
                {
                    return Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                        identity: owner_identity,
                        generation: owner_generation,
                    });
                }
                let state = ActiveProfileOwnerSwitchState::OldStopped {
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                };
                Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                    Box::new(ActiveOwnerStoppedSwitchLease::new(
                        config_root,
                        transition_id,
                        state,
                    )),
                ))
            }
            ActiveOwnerSwitchStage::ReferenceChanged {
                terminal_generation,
            } => {
                let owner_probe = ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?;
                let Some(owner_probe) = owner_probe else {
                    return Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                        identity: owner_identity,
                        generation: owner_generation,
                    });
                };
                drop(owner_probe);
                let mut proof = sync_read_active_core_owner_proof(&config_root)?;
                let proven_generation = match terminal_proof_generation(
                    &proof,
                    &owner_identity,
                    owner_generation,
                    &owner_token,
                ) {
                    Ok(proven_generation) => proven_generation,
                    Err(_) => {
                        // `reserve_new_owner` persists its kernel admission
                        // proof before advancing the root record to
                        // BindingNew. If a process dies between those writes,
                        // this ReferenceChanged stage proves provider
                        // dispatch never received a lease. Restore the old
                        // terminal proof so the durable CAS stage can resume.
                        let selected_engine = match &record.selection {
                            ActiveProfileReferenceState::Selected(reference) => {
                                &reference.engine_id
                            }
                            ActiveProfileReferenceState::Unselected { .. } => {
                                return Err(transition_rejected(
                                    "reference-changed owner switch has no selected engine",
                                ));
                            }
                        };
                        if !matches!(
                            &proof.state,
                            ActiveCoreOwnerProof::Reserved {
                                transition_id: proof_transition,
                                engine_id: proof_engine,
                                ..
                            } if proof_transition == &transition_id
                                && proof_engine == selected_engine
                        ) {
                            return Err(transition_rejected(
                                "persisted owner terminal generation differs from its proof",
                            ));
                        }
                        proof.state = ActiveCoreOwnerProof::Terminal {
                            identity: owner_identity.clone(),
                            bound_generation: owner_generation,
                            terminal_generation,
                            token: owner_token.clone(),
                        };
                        sync_publish_active_core_owner_proof(&config_root, &proof)?;
                        terminal_generation
                    }
                };
                if proven_generation != terminal_generation {
                    return Err(transition_rejected(
                        "persisted owner terminal generation differs from its proof",
                    ));
                }
                let state = ActiveProfileOwnerSwitchState::ReferenceChanged {
                    previous_reference,
                    selection: record.selection,
                    owner_identity,
                    terminal_generation,
                };
                Ok(ActiveProfileOwnerSwitchRecovery::ResumeReferenceChanged(
                    Box::new(ReferenceChangedLease::new(
                        config_root,
                        transition_id,
                        state,
                    )),
                ))
            }
            ActiveOwnerSwitchStage::BindingNew {
                engine_id, token, ..
            } => match ActiveProfileReferenceFile::new(config_root.clone())
                .try_acquire_owner_lock()?
            {
                None => {
                    let local_state = LocalOwnerRegistrationState::Binding {
                        transition_id: transition_id.clone(),
                    };
                    if local_registration_matches(&config_root, &token, &local_state)? {
                        let state = ActiveProfileOwnerSwitchState::BindingNew {
                            selection: record.selection,
                            engine_id,
                        };
                        Ok(ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(
                            Box::new(OwnerBindingLease::new(
                                config_root,
                                transition_id,
                                token,
                                state,
                            )),
                        ))
                    } else {
                        Ok(ActiveProfileOwnerSwitchRecovery::CandidateStillLive { engine_id })
                    }
                }
                Some(owner_probe) => {
                    drop(owner_probe);
                    Ok(
                        ActiveProfileOwnerSwitchRecovery::CandidateTerminalEvidenceRequired {
                            selection: record.selection,
                            engine_id,
                        },
                    )
                }
            },
        }
    }

    pub async fn recover_old_owner_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (transition_id, previous_reference, owner_identity, owner_generation, owner_token) =
            match &record.owner {
                ActiveCoreOwnerState::Switching {
                    transition_id,
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    owner_token,
                    stage: ActiveOwnerSwitchStage::ClosingOld,
                } => (
                    transition_id.clone(),
                    previous_reference.clone(),
                    owner_identity.clone(),
                    *owner_generation,
                    owner_token.clone(),
                ),
                _ => {
                    return Err(transition_rejected(
                        "old-owner terminal recovery requires a ClosingOld reservation",
                    ));
                }
            };
        if record.selection != previous_reference {
            return Err(transition_rejected(
                "active reference changed before old-owner terminal recovery",
            ));
        }
        let terminal_generation =
            validate_terminal_for_owner(terminal, &owner_identity, owner_generation)?;
        if local_owner_registration_state(&config_root, &owner_token)?.is_some() {
            return Err(transition_rejected(
                "old owner still has a local registration; recover its live switch lease",
            ));
        }
        let owner_probe = ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .ok_or_else(|| {
                transition_rejected("old owner still holds the cross-process owner fence")
            })?;
        drop(owner_probe);

        let mut proof = sync_read_active_core_owner_proof(&config_root)?;
        match proof.state.clone() {
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            } if proof_identity.same_instance(&owner_identity)
                && proof_generation == owner_generation
                && proof_token == owner_token =>
            {
                proof.state = ActiveCoreOwnerProof::Terminal {
                    identity: owner_identity.clone(),
                    bound_generation: owner_generation,
                    terminal_generation,
                    token: owner_token.clone(),
                };
                sync_publish_active_core_owner_proof(&config_root, &proof)?;
            }
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            } if proof_identity.same_instance(&owner_identity)
                && proof_generation == owner_generation
                && proof_terminal_generation == terminal_generation
                && proof_token == owner_token => {}
            _ => {
                return Err(transition_rejected(
                    "old-owner terminal snapshot does not match the persisted owner proof",
                ));
            }
        }
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
        let state = ActiveProfileOwnerSwitchState::OldStopped {
            previous_reference,
            owner_identity,
            owner_generation,
            terminal_generation,
        };
        Ok(Box::new(ActiveOwnerStoppedSwitchLease::new(
            config_root,
            transition_id,
            state,
        )))
    }

    pub async fn recover_candidate_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (
            transition_id,
            engine_id,
            token,
            old_owner_identity,
            old_owner_generation,
            old_owner_token,
            old_terminal_generation,
        ) = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id,
                owner_identity,
                owner_generation,
                owner_token,
                stage:
                    ActiveOwnerSwitchStage::BindingNew {
                        engine_id,
                        token,
                        terminal_generation,
                    },
                ..
            } => (
                transition_id.clone(),
                engine_id.clone(),
                token.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                *terminal_generation,
            ),
            _ => {
                return Err(transition_rejected(
                    "no crashed candidate reservation is pending",
                ));
            }
        };
        let identity = terminal
            .identity
            .as_ref()
            .ok_or_else(|| transition_rejected("candidate terminal has no Core identity"))?;
        if identity.engine != engine_id
            || terminal.lifecycle != infiltrator_contract::snapshot::CoreLifecycle::Stopped
            || terminal.generation != 0
            || terminal.session_token.is_some()
        {
            return Err(transition_rejected(
                "candidate recovery requires a never-started Stopped readback without a session",
            ));
        }
        if local_owner_registration_state(&config_root, &token)?.is_some() {
            return Err(transition_rejected(
                "candidate owner is still registered in this process; use its binding lease",
            ));
        }
        if ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .is_none()
        {
            return Err(transition_rejected(
                "candidate process still holds the active owner fence",
            ));
        }
        let mut proof = sync_read_active_core_owner_proof(&config_root)?;
        match &proof.state {
            ActiveCoreOwnerProof::Reserved {
                transition_id: proof_transition,
                engine_id: proof_engine,
                token: proof_token,
            } if proof_transition == &transition_id
                && proof_engine == &engine_id
                && proof_token == &token => {}
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            } if proof_identity.same_instance(identity)
                && *proof_generation == CoreGeneration::new(terminal.generation)
                && *proof_generation == CoreGeneration::new(0)
                && proof_token == &token => {}
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_bound_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            } if proof_identity.same_instance(&old_owner_identity)
                && *proof_bound_generation == old_owner_generation
                && *proof_terminal_generation == old_terminal_generation
                && proof_token == &old_owner_token => {}
            _ => return Err(transition_rejected("candidate owner proof is stale")),
        }
        proof.state = ActiveCoreOwnerProof::Terminal {
            identity: old_owner_identity,
            bound_generation: old_owner_generation,
            terminal_generation: old_terminal_generation,
            token: old_owner_token,
        };
        sync_publish_active_core_owner_proof(&config_root, &proof)?;
        record.owner = ActiveCoreOwnerState::Unowned;
        sync_publish_active_profile_record(&config_root, &record)?;
        Ok(record.selection)
    }
}
