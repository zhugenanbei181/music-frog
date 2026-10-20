//! Process-local owner registrations paired with the cross-process owner lock.
//!
//! The registry is process-local: it records which configuration root this
//! process owns, together with the fence token of that ownership. Every
//! mutation is fenced by the token and by the exact registered state, so a
//! stale lease can never mutate a registration that moved on.

use crate::instance_lock::ThreadedInstanceLockGuard;
use crate::owner_switch::owner::ActiveOwnerToken;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_ports::error::PortError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// The process-local registration state of one configuration root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOwnerRegistrationState {
    /// The owner is bound and running.
    Bound {
        /// Stable identity of the bound owner.
        identity: CoreIdentity,
        /// Exact generation the owner is bound to.
        generation: CoreGeneration,
    },
    /// A candidate owner admission is reserved.
    Binding {
        /// The token that identifies the running transition.
        transition_id: ActiveOwnerToken,
    },
    /// The owner is being replaced.
    Switching {
        /// The token that identifies the running transition.
        transition_id: ActiveOwnerToken,
        /// Stable identity of the previous owner.
        identity: CoreIdentity,
        /// Exact generation the previous owner is bound to.
        generation: CoreGeneration,
    },
}

/// The outcome of recovering a local switch registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSwitchRegistrationRecovery {
    /// The registration was already switching for this transition.
    AlreadySwitching,
    /// The registration was bound and has been promoted to switching.
    PromotedBound,
    /// This owner does not hold the registration.
    NotHeldByThisOwner,
}

struct LocalOwnerRegistration {
    token: ActiveOwnerToken,
    state: LocalOwnerRegistrationState,
    _guard: ThreadedInstanceLockGuard,
}

static REGISTRATIONS: OnceLock<Mutex<HashMap<PathBuf, LocalOwnerRegistration>>> = OnceLock::new();

/// The exclusive guard over the process-local owner registrations.
pub struct LocalOwnerRegistryGuard<'a> {
    registrations: MutexGuard<'a, HashMap<PathBuf, LocalOwnerRegistration>>,
}

/// Locks the process-local owner registry.
///
/// Fails with a `Failed` port error when the registry lock is poisoned.
pub fn lock_local_owner_registry() -> Result<LocalOwnerRegistryGuard<'static>, PortError> {
    let registrations = REGISTRATIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let registrations = registrations
        .lock()
        .map_err(|_| PortError::Failed("active owner registration lock is poisoned".into()))?;
    Ok(LocalOwnerRegistryGuard { registrations })
}

impl LocalOwnerRegistryGuard<'_> {
    /// Whether this process holds a registration for `config_root`.
    pub fn contains(&self, config_root: &Path) -> bool {
        self.registrations.contains_key(config_root)
    }

    /// Whether the registration matches the exact token and state.
    pub fn matches(
        &self,
        config_root: &Path,
        token: &ActiveOwnerToken,
        state: &LocalOwnerRegistrationState,
    ) -> bool {
        self.registrations
            .get(config_root)
            .is_some_and(|registration| {
                registration.token == *token && registration.state == *state
            })
    }

    /// Returns the state of the registration, when the token matches.
    pub fn state(
        &self,
        config_root: &Path,
        token: &ActiveOwnerToken,
    ) -> Option<LocalOwnerRegistrationState> {
        self.registrations
            .get(config_root)
            .filter(|registration| registration.token == *token)
            .map(|registration| registration.state.clone())
    }

    fn insert_registration(
        &mut self,
        config_root: PathBuf,
        token: ActiveOwnerToken,
        state: LocalOwnerRegistrationState,
        guard: ThreadedInstanceLockGuard,
    ) -> Result<(), PortError> {
        if self.registrations.contains_key(&config_root) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "this process already has an active Core owner registration",
                true,
            )));
        }
        self.registrations.insert(
            config_root,
            LocalOwnerRegistration {
                token,
                state,
                _guard: guard,
            },
        );
        Ok(())
    }

    /// Registers a bound owner for `config_root`.
    ///
    /// Fails with a retryable `NotReady` failure when this process already
    /// holds a registration for the same configuration root.
    pub fn register_bound(
        &mut self,
        config_root: PathBuf,
        token: ActiveOwnerToken,
        identity: CoreIdentity,
        generation: CoreGeneration,
        guard: ThreadedInstanceLockGuard,
    ) -> Result<(), PortError> {
        self.insert_registration(
            config_root,
            token,
            LocalOwnerRegistrationState::Bound {
                identity,
                generation,
            },
            guard,
        )
    }

    /// Registers a reserved candidate owner admission for `config_root`.
    pub fn register_binding(
        &mut self,
        config_root: PathBuf,
        token: ActiveOwnerToken,
        transition_id: ActiveOwnerToken,
        guard: ThreadedInstanceLockGuard,
    ) -> Result<(), PortError> {
        self.insert_registration(
            config_root,
            token,
            LocalOwnerRegistrationState::Binding { transition_id },
            guard,
        )
    }

    /// Moves a bound registration into the switching state.
    ///
    /// Fails with a retryable `NotReady` failure when the registration is
    /// stale or does not carry the exact bound identity and generation.
    pub fn begin_switching(
        &mut self,
        config_root: &Path,
        token: &ActiveOwnerToken,
        transition_id: ActiveOwnerToken,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<(), PortError> {
        let registration = self
            .registrations
            .get_mut(config_root)
            .filter(|registration| registration.token == *token)
            .ok_or_else(|| stale_registration("active Core owner registration is stale"))?;
        match &registration.state {
            LocalOwnerRegistrationState::Bound {
                identity: registered_identity,
                generation: registered_generation,
            } if registered_identity.same_instance(&identity)
                && *registered_generation == generation =>
            {
                registration.state = LocalOwnerRegistrationState::Switching {
                    transition_id,
                    identity,
                    generation,
                };
                Ok(())
            }
            _ => Err(stale_registration(
                "active Core owner cannot enter switching from its current registration",
            )),
        }
    }

    /// Cancels a switch that has not closed the previous owner yet.
    ///
    /// The registration returns to the bound state of the previous owner.
    pub fn cancel_switch_before_close(
        &mut self,
        config_root: &Path,
        token: &ActiveOwnerToken,
        transition_id: &ActiveOwnerToken,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<(), PortError> {
        let registration = self
            .registrations
            .get_mut(config_root)
            .filter(|registration| registration.token == *token)
            .ok_or_else(|| stale_registration("active Core owner registration is stale"))?;
        match &registration.state {
            LocalOwnerRegistrationState::Switching {
                transition_id: registered_transition,
                identity: registered_identity,
                generation: registered_generation,
            } if registered_transition == transition_id
                && registered_identity.same_instance(&identity)
                && *registered_generation == generation =>
            {
                registration.state = LocalOwnerRegistrationState::Bound {
                    identity,
                    generation,
                };
                Ok(())
            }
            _ => Err(stale_registration(
                "active Core owner cannot cancel a different switch registration",
            )),
        }
    }

    /// Finalizes a reserved candidate admission into a bound owner.
    pub fn finalize_binding(
        &mut self,
        config_root: &Path,
        token: &ActiveOwnerToken,
        transition_id: &ActiveOwnerToken,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<(), PortError> {
        let registration = self
            .registrations
            .get_mut(config_root)
            .filter(|registration| registration.token == *token)
            .ok_or_else(|| stale_registration("active Core owner registration is stale"))?;
        match &registration.state {
            LocalOwnerRegistrationState::Binding {
                transition_id: registered_transition,
            } if registered_transition == transition_id => {
                registration.state = LocalOwnerRegistrationState::Bound {
                    identity,
                    generation,
                };
                Ok(())
            }
            _ => Err(stale_registration(
                "active Core owner cannot finalize a different binding registration",
            )),
        }
    }

    /// Removes the registration of `config_root`.
    ///
    /// Fails with a retryable `NotReady` failure when the token does not match
    /// the registration, so a stale lease cannot release a live owner.
    pub fn remove(
        &mut self,
        config_root: &Path,
        token: &ActiveOwnerToken,
    ) -> Result<(), PortError> {
        match self.registrations.get(config_root) {
            Some(registration) if registration.token == *token => {
                self.registrations.remove(config_root);
                Ok(())
            }
            Some(_) | None => Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active Core owner lease is stale or no longer registered",
                true,
            ))),
        }
    }

    /// Recovers a local registration into the switching state.
    ///
    /// Returns whether the registration was already switching, was promoted
    /// from the bound state, or is not held by this owner at all.
    pub fn recover_switch(
        &mut self,
        config_root: &Path,
        token: &ActiveOwnerToken,
        transition_id: &ActiveOwnerToken,
        identity: &CoreIdentity,
        generation: CoreGeneration,
    ) -> LocalSwitchRegistrationRecovery {
        let Some(registration) = self
            .registrations
            .get_mut(config_root)
            .filter(|registration| registration.token == *token)
        else {
            return LocalSwitchRegistrationRecovery::NotHeldByThisOwner;
        };
        match &registration.state {
            LocalOwnerRegistrationState::Bound {
                identity: registered_identity,
                generation: registered_generation,
            } if registered_identity.same_instance(identity)
                && *registered_generation == generation =>
            {
                registration.state = LocalOwnerRegistrationState::Switching {
                    transition_id: transition_id.clone(),
                    identity: identity.clone(),
                    generation,
                };
                LocalSwitchRegistrationRecovery::PromotedBound
            }
            LocalOwnerRegistrationState::Switching {
                transition_id: registered_transition,
                identity: registered_identity,
                generation: registered_generation,
            } if registered_transition == transition_id
                && registered_identity.same_instance(identity)
                && *registered_generation == generation =>
            {
                LocalSwitchRegistrationRecovery::AlreadySwitching
            }
            _ => LocalSwitchRegistrationRecovery::NotHeldByThisOwner,
        }
    }
}

/// Builds the typed stale-registration failure of the local registry.
fn stale_registration(message: &str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message, true))
}

/// Whether the local registration of `config_root` matches token and state.
pub fn local_owner_registration_matches(
    config_root: &Path,
    token: &ActiveOwnerToken,
    state: &LocalOwnerRegistrationState,
) -> Result<bool, PortError> {
    Ok(lock_local_owner_registry()?.matches(config_root, token, state))
}

/// Returns the local registration state of `config_root`, when the token matches.
pub fn local_owner_registration_state(
    config_root: &Path,
    token: &ActiveOwnerToken,
) -> Result<Option<LocalOwnerRegistrationState>, PortError> {
    Ok(lock_local_owner_registry()?.state(config_root, token))
}

/// Removes the local registration of `config_root`.
pub fn remove_local_owner_registration(
    config_root: &Path,
    token: &ActiveOwnerToken,
) -> Result<(), PortError> {
    lock_local_owner_registry()?.remove(config_root, token)
}
