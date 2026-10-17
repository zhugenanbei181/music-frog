//! Live Core owner session proof and exact terminal release.

use crate::active_owner_registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches, local_owner_registration_state,
    remove_local_owner_registration,
};
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveOwnerToken, transition_rejected,
};
use crate::active_reference_owner_coordinator::validate_terminal_for_owner;
use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_ports::engine_switch::EngineOwnerLease;
use lurker_core_ports::error::PortError;
use lurker_profile_store::instance_lock::ThreadedInstanceLockGuard;
use std::path::{Path, PathBuf};
use tokio::task::spawn_blocking;

pub struct ActiveCoreOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    token: ActiveOwnerToken,
    config_root: PathBuf,
}

impl ActiveCoreOwnerLease {
    pub fn new(
        identity: CoreIdentity,
        generation: CoreGeneration,
        token: ActiveOwnerToken,
        config_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            identity,
            generation,
            token,
            config_root: config_root.into(),
        }
    }
}

#[async_trait::async_trait]
impl ActiveProfileOwnerLease for ActiveCoreOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    async fn record_start_context(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<(), PortError> {
        let identity = self.identity.clone();
        let generation = self.generation;
        let token = self.token.clone();
        let context = context.clone();
        let config_root = self.config_root.clone();
        let owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        spawn_blocking(move || {
            let _owner_state_guard = owner_state_guard;
            let (context_identity, context_generation, session_token) = match &context {
                CoreProcessStartContext::Managed {
                    identity,
                    generation,
                    session_token,
                } => (identity, *generation, *session_token),
                CoreProcessStartContext::Standalone { .. } => {
                    return Err(transition_rejected(
                        "active Core owner proof only accepts managed start contexts",
                    ));
                }
            };
            if !context_identity.same_instance(&identity)
                || context_generation <= generation
                || !session_token.is_valid()
            {
                return Err(transition_rejected(
                    "managed start context does not advance the bound Core owner session",
                ));
            }
            if !local_owner_registration_matches(
                &config_root,
                &token,
                &LocalOwnerRegistrationState::Bound {
                    identity: identity.clone(),
                    generation,
                },
            )? {
                return Err(transition_rejected(
                    "managed start context owner is no longer bound in this process",
                ));
            }
            let file = ActiveProfileReferenceFile::new(config_root.clone());
            let mut proof = file.read_owner_proof_sync()?;
            match &proof.state {
                ActiveCoreOwnerProof::Bound {
                    identity: proof_identity,
                    generation: proof_generation,
                    token: proof_token,
                } if proof_identity.same_instance(&identity)
                    && *proof_generation == generation
                    && *proof_token == token => {}
                _ => {
                    return Err(transition_rejected(
                        "managed start context does not match the persisted Core owner proof",
                    ));
                }
            }
            match &proof.launch {
                ActiveCoreOwnerLaunchProof::Unstarted => {}
                ActiveCoreOwnerLaunchProof::Managed(previous) if previous == &context => {
                    return Ok(());
                }
                ActiveCoreOwnerLaunchProof::Managed(previous) => {
                    let previous_generation = match previous {
                        CoreProcessStartContext::Managed { generation, .. } => *generation,
                        CoreProcessStartContext::Standalone { .. } => {
                            return Err(transition_rejected(
                                "persisted owner launch proof contains a standalone context",
                            ));
                        }
                    };
                    if context_generation <= previous_generation {
                        return Err(transition_rejected(
                            "managed start context does not advance the persisted session",
                        ));
                    }
                }
                ActiveCoreOwnerLaunchProof::LegacyUnverified => {
                    return Err(transition_rejected(
                        "legacy Core owner proof has no exact session and cannot launch",
                    ));
                }
            }
            proof.launch = ActiveCoreOwnerLaunchProof::Managed(context);
            file.publish_owner_proof_sync_reconciled(&proof)
        })
        .await
        .map_err(|error| PortError::Failed(error.to_string()))?
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        let identity = self.identity.clone();
        let generation = self.generation;
        let token = self.token.clone();
        let config_root = self.config_root.clone();
        let terminal = terminal.clone();
        let owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        spawn_blocking(move || {
            release_owner_after_terminal(
                &identity,
                generation,
                &token,
                &config_root,
                &terminal,
                owner_state_guard,
            )
        })
        .await
        .map_err(|error| PortError::Failed(error.to_string()))?
    }
}

fn release_owner_after_terminal(
    identity: &CoreIdentity,
    generation: CoreGeneration,
    token: &ActiveOwnerToken,
    config_root: &Path,
    terminal: &CoreLifecycleSnapshot,
    _owner_state_guard: ThreadedInstanceLockGuard,
) -> Result<(), PortError> {
    let terminal_generation = validate_terminal_for_owner(terminal, identity, generation)?;
    // Terminal release takes only owner-state. Store operations use root then
    // owner-state; close never waits for the root lock.
    let file = ActiveProfileReferenceFile::new(config_root);
    let mut proof = file.read_owner_proof_sync()?;
    let local_state = local_owner_registration_state(config_root, token)?;
    match proof.state.clone() {
        ActiveCoreOwnerProof::Bound {
            identity: proof_identity,
            generation: proof_generation,
            token: proof_token,
        } if proof_identity.same_instance(identity)
            && proof_generation == generation
            && proof_token == *token =>
        {
            let local_state_matches = match local_state {
                Some(LocalOwnerRegistrationState::Bound {
                    identity: registered_identity,
                    generation: registered_generation,
                }) => {
                    registered_identity.same_instance(identity)
                        && registered_generation == generation
                }
                Some(LocalOwnerRegistrationState::Switching {
                    identity: registered_identity,
                    generation: registered_generation,
                    ..
                }) => {
                    registered_identity.same_instance(identity)
                        && registered_generation == generation
                }
                Some(LocalOwnerRegistrationState::Binding { .. }) | None => false,
            };
            if !local_state_matches {
                return Err(transition_rejected(
                    "active Core owner kernel lease is not held by this process",
                ));
            }
            proof.state = ActiveCoreOwnerProof::Terminal {
                identity: identity.clone(),
                bound_generation: generation,
                terminal_generation,
                token: token.clone(),
            };
            file.publish_owner_proof_sync_reconciled(&proof)?;
            remove_local_owner_registration(config_root, token)
        }
        ActiveCoreOwnerProof::Terminal {
            identity: proof_identity,
            bound_generation: proof_generation,
            terminal_generation: proof_terminal_generation,
            token: proof_token,
        } if proof_identity.same_instance(identity)
            && proof_generation == generation
            && proof_terminal_generation == terminal_generation
            && proof_token == *token =>
        {
            match local_state {
                Some(LocalOwnerRegistrationState::Bound {
                    identity: registered_identity,
                    generation: registered_generation,
                })
                | Some(LocalOwnerRegistrationState::Switching {
                    identity: registered_identity,
                    generation: registered_generation,
                    ..
                }) if registered_identity.same_instance(identity)
                    && registered_generation == generation =>
                {
                    remove_local_owner_registration(config_root, token)
                }
                Some(LocalOwnerRegistrationState::Binding { .. }) => Err(transition_rejected(
                    "active Core owner registration is not bound to the terminal identity",
                )),
                None => Ok(()),
                _ => Err(transition_rejected(
                    "active Core owner registration differs from the terminal proof",
                )),
            }
        }
        _ => Err(transition_rejected(
            "active Core owner lease is stale or no longer bound",
        )),
    }
}

/// The neutral binding lease view of a live owner. Product release semantics
/// stay on `ActiveProfileOwnerLease`; this only exposes the exact fence the
/// neutral journal needs.
impl EngineOwnerLease for ActiveCoreOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }
}
