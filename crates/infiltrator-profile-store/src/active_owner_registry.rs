//! Process-local owner registrations paired with the cross-process owner lock.

use crate::active_reference_owner::ActiveOwnerToken;
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::error::PortError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOwnerRegistrationState {
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
    Binding {
        transition_id: ActiveOwnerToken,
    },
    Switching {
        transition_id: ActiveOwnerToken,
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSwitchRegistrationRecovery {
    AlreadySwitching,
    PromotedBound,
    NotHeldByThisOwner,
}

struct LocalOwnerRegistration {
    token: ActiveOwnerToken,
    state: LocalOwnerRegistrationState,
    _guard: ThreadedInstanceLockGuard,
}

static REGISTRATIONS: OnceLock<Mutex<HashMap<PathBuf, LocalOwnerRegistration>>> = OnceLock::new();

pub struct LocalOwnerRegistryGuard<'a> {
    registrations: MutexGuard<'a, HashMap<PathBuf, LocalOwnerRegistration>>,
}

pub fn lock_local_owner_registry() -> Result<LocalOwnerRegistryGuard<'static>, PortError> {
    let registrations = REGISTRATIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let registrations = registrations
        .lock()
        .map_err(|_| PortError::Failed("active owner registration lock is poisoned".into()))?;
    Ok(LocalOwnerRegistryGuard { registrations })
}

impl LocalOwnerRegistryGuard<'_> {
    pub fn contains(&self, config_root: &Path) -> bool {
        self.registrations.contains_key(config_root)
    }

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

    pub fn insert(
        &mut self,
        config_root: PathBuf,
        token: ActiveOwnerToken,
        state: LocalOwnerRegistrationState,
        guard: ThreadedInstanceLockGuard,
    ) -> Result<(), PortError> {
        if self.registrations.contains_key(&config_root) {
            return Err(PortError::Rejected(
                infiltrator_contract::error::Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "this process already has an active Core owner registration",
                    true,
                ),
            ));
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

    pub fn set_state(
        &mut self,
        config_root: &Path,
        token: &ActiveOwnerToken,
        state: LocalOwnerRegistrationState,
    ) -> Result<(), PortError> {
        let registration = self
            .registrations
            .get_mut(config_root)
            .filter(|registration| registration.token == *token)
            .ok_or_else(|| {
                PortError::Rejected(infiltrator_contract::error::Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "active Core owner registration is stale",
                    true,
                ))
            })?;
        registration.state = state;
        Ok(())
    }

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
            Some(_) | None => Err(PortError::Rejected(
                infiltrator_contract::error::Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "active Core owner lease is stale or no longer registered",
                    true,
                ),
            )),
        }
    }

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

pub fn local_owner_registration_matches(
    config_root: &Path,
    token: &ActiveOwnerToken,
    state: &LocalOwnerRegistrationState,
) -> Result<bool, PortError> {
    Ok(lock_local_owner_registry()?.matches(config_root, token, state))
}

pub fn local_owner_registration_state(
    config_root: &Path,
    token: &ActiveOwnerToken,
) -> Result<Option<LocalOwnerRegistrationState>, PortError> {
    Ok(lock_local_owner_registry()?.state(config_root, token))
}

pub fn remove_local_owner_registration(
    config_root: &Path,
    token: &ActiveOwnerToken,
) -> Result<(), PortError> {
    lock_local_owner_registry()?.remove(config_root, token)
}
