use super::*;
use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::owner::transition_rejected;
use crate::owner_switch::registry::lock_local_owner_registry;
use crate::owner_switch::registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches, remove_local_owner_registration,
};
use crate::owner_switch::stage::switch_snapshot;
use lurker_core_contract::engine_switch::{EngineSwitchSnapshot, EngineSwitchStage};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_switch::{EngineOwnerBindingLease, EngineOwnerLease};
use lurker_core_ports::error::PortError;
use std::marker::PhantomData;
use std::path::PathBuf;

pub struct OwnerBindingLease<C: OwnerSwitchCodec> {
    pub(crate) config_root: PathBuf,
    pub(crate) transition_id: ActiveOwnerToken,
    pub(crate) token: ActiveOwnerToken,
    pub(crate) stage: EngineSwitchStage<C::Selection>,
    pub(crate) codec: PhantomData<C>,
}

impl<C: OwnerSwitchCodec> OwnerBindingLease<C> {
    pub fn new(
        config_root: impl Into<PathBuf>,
        transition_id: ActiveOwnerToken,
        token: ActiveOwnerToken,
        stage: EngineSwitchStage<C::Selection>,
    ) -> Self {
        Self {
            config_root: config_root.into(),
            transition_id,
            token,
            stage,
            codec: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
enum CandidateDiscardAuthorization {
    BeforeCoreApplication,
    Stopped(CoreGeneration),
}

/// Release the candidate admission. `BeforeCoreApplication` is the explicit
/// pre-application abort and leaves the journal unowned. `Stopped` is
/// [`EngineOwnerBindingLease::discard_unstarted_candidate`]: it restores the
/// old owner's exact terminal proof and rewinds the owner state to the durable
/// `ReferenceChanged` stage (neutral `SelectionChanged`), so the caller can
/// retry the reservation exactly as the port documents. It never fabricates an
/// owner or a terminal.
async fn clear_candidate_owner_reservation<C: OwnerSwitchCodec>(
    config_root: PathBuf,
    transition_id: ActiveOwnerToken,
    token: ActiveOwnerToken,
    selection: C::Selection,
    engine_id: EngineId,
    authorization: CandidateDiscardAuthorization,
) -> Result<EngineSwitchSnapshot<C::Selection>, PortError> {
    match authorization {
        CandidateDiscardAuthorization::BeforeCoreApplication => {}
        CandidateDiscardAuthorization::Stopped(terminal_generation) => {
            if terminal_generation != CoreGeneration::new(0) {
                return Err(transition_rejected(
                    "candidate can be discarded only after a never-started terminal readback",
                ));
            }
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
    let (
        previous_reference,
        old_owner_identity,
        old_owner_generation,
        old_owner_token,
        old_terminal_generation,
    ) = match C::owner(&record) {
        ActiveCoreOwnerState::Switching {
            transition_id: persisted_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage:
                ActiveOwnerSwitchStage::BindingNew {
                    engine_id: persisted_engine,
                    token: persisted_token,
                    terminal_generation,
                },
        } if persisted_id == &transition_id
            && persisted_engine == &engine_id
            && persisted_token == &token =>
        {
            (
                previous_reference.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                *terminal_generation,
            )
        }
        _ => return Err(transition_rejected("new owner reservation is stale")),
    };
    if C::selection(&record) != &selection
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
    let mut proof = sync_read_active_core_owner_proof::<C>(&config_root)?;
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
            CandidateDiscardAuthorization::Stopped(terminal_generation),
        ) if proof_identity.engine == engine_id
            && *proof_generation == terminal_generation
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
    if let Some(probe) = OwnerSwitchFile::<C>::new(config_root.clone()).try_acquire_owner_lock()? {
        drop(probe);
        return Err(transition_rejected(
            "candidate owner kernel admission was lost",
        ));
    }
    // Restore the exact old terminal proof before rewinding the root stage.
    // A crash between these writes therefore leaves a BindingNew marker that
    // can still request candidate terminal evidence; it never leaves an
    // unowned root with an unproven Reserved/Bound owner proof.
    proof.state = ActiveCoreOwnerProof::Terminal {
        identity: old_owner_identity.clone(),
        bound_generation: old_owner_generation,
        terminal_generation: old_terminal_generation,
        token: old_owner_token.clone(),
    };
    sync_publish_active_core_owner_proof::<C>(&config_root, &proof)?;
    match authorization {
        CandidateDiscardAuthorization::BeforeCoreApplication => {
            C::set_owner(&mut record, ActiveCoreOwnerState::Unowned);
        }
        CandidateDiscardAuthorization::Stopped(_) => {
            C::set_owner(
                &mut record,
                ActiveCoreOwnerState::Switching {
                    transition_id,
                    previous_reference,
                    owner_identity: old_owner_identity,
                    owner_generation: old_owner_generation,
                    owner_token: old_owner_token,
                    stage: ActiveOwnerSwitchStage::ReferenceChanged {
                        terminal_generation: old_terminal_generation,
                    },
                },
            );
        }
    }
    sync_publish_active_profile_record::<C>(&config_root, &record)?;
    remove_local_owner_registration(&config_root, &token)?;
    switch_snapshot::<C>(&record)
}

/// The engine-neutral owner fence returned by a successful binding. The
/// product rehydrates its own owner lease separately.
pub struct OwnerFenceLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
}

impl OwnerFenceLease {
    pub fn new(identity: CoreIdentity, generation: CoreGeneration) -> Self {
        Self {
            identity,
            generation,
        }
    }
}

impl EngineOwnerLease for OwnerFenceLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }
}

#[async_trait::async_trait]
impl<C: OwnerSwitchCodec> EngineOwnerBindingLease<C::Selection> for OwnerBindingLease<C> {
    fn stage(&self) -> &EngineSwitchStage<C::Selection> {
        &self.stage
    }

    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerLease>, PortError> {
        let Self {
            config_root,
            transition_id,
            token,
            stage,
            ..
        } = *self;
        let (selection, engine_id) = match stage {
            EngineSwitchStage::BindingNew {
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
        let _root_guard = OwnerSwitchFile::<C>::new(config_root.clone())
            .acquire_root_lock()
            .await?
            .1;
        let _owner_state_guard = OwnerSwitchFile::<C>::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record::<C>(&config_root)?;
        let stage_token = match C::owner(&record) {
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
        if C::selection(&record) != &selection || stage_token != token {
            return Err(transition_rejected(
                "active reference changed after the new owner was reserved",
            ));
        }
        if !local_owner_registration_matches(
            &config_root,
            &token,
            &LocalOwnerRegistrationState::Binding {
                transition_id: transition_id.clone(),
            },
        )? {
            return Err(transition_rejected(
                "new owner kernel admission is no longer held by this process",
            ));
        }
        let mut owner_proof = sync_read_active_core_owner_proof::<C>(&config_root)?;
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
            OwnerSwitchFile::<C>::new(config_root.clone()).try_acquire_owner_lock()?
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
        sync_publish_active_core_owner_proof::<C>(&config_root, &owner_proof)?;
        C::set_owner(
            &mut record,
            ActiveCoreOwnerState::Bound {
                identity: identity.clone(),
                generation,
                token: token.clone(),
            },
        );
        sync_publish_active_profile_record::<C>(&config_root, &record)?;
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
        Ok(Box::new(OwnerFenceLease::new(identity, generation)))
    }

    async fn discard_unstarted_candidate(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<C::Selection>, PortError> {
        let Self {
            config_root,
            transition_id,
            token,
            stage,
            ..
        } = *self;
        let (selection, engine_id) = match stage {
            EngineSwitchStage::BindingNew {
                selection,
                engine_id,
            } => (selection, engine_id),
            _ => {
                return Err(transition_rejected(
                    "new owner binding reservation is missing",
                ));
            }
        };
        clear_candidate_owner_reservation::<C>(
            config_root,
            transition_id,
            token,
            selection,
            engine_id,
            CandidateDiscardAuthorization::Stopped(terminal_generation),
        )
        .await
    }

    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<C::Selection>, PortError> {
        let Self {
            config_root,
            transition_id,
            token,
            stage,
            ..
        } = *self;
        let (selection, engine_id) = match stage {
            EngineSwitchStage::BindingNew {
                selection,
                engine_id,
            } => (selection, engine_id),
            _ => {
                return Err(transition_rejected(
                    "new owner binding reservation is missing",
                ));
            }
        };
        clear_candidate_owner_reservation::<C>(
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
