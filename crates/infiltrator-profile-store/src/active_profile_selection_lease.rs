//! Selection fence promotion into the unique live Core owner lease.

use crate::active_owner_lease::ActiveCoreOwnerLease;
use crate::active_owner_registry::lock_local_owner_registry;
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveCoreOwnerState, ActiveOwnerToken,
    ActiveProfileReferenceRecord, transition_rejected,
};
use crate::store::EngineProfileStore;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerLease, ActiveProfileReferenceLease,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;

impl<S: SecureStore> EngineProfileStore<S> {
    /// Project the selected profile for this engine while the caller retains
    /// the product-wide selection fence.
    pub fn get_current_under_selection_lease(
        &self,
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<String, PortError> {
        match selection.state() {
            ActiveProfileReferenceState::Unselected { .. } => Err(PortError::unsupported(
                Capability::Profiles,
                "no product-wide active profile is selected",
            )),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id() =>
            {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Selected(reference) => Err(PortError::unsupported(
                Capability::Profiles,
                format!(
                    "the active profile belongs to engine {} instead of {}",
                    reference.engine_id.as_str(),
                    self.engine_id().as_str()
                ),
            )),
        }
    }
}

pub struct ActiveProfileSelectionLease {
    state: ActiveProfileReferenceState,
    config_root: PathBuf,
    _guard: ThreadedInstanceLockGuard,
}

impl ActiveProfileSelectionLease {
    pub fn new(
        state: ActiveProfileReferenceState,
        config_root: impl Into<PathBuf>,
        guard: ThreadedInstanceLockGuard,
    ) -> Self {
        Self {
            state,
            config_root: config_root.into(),
            _guard: guard,
        }
    }
}

#[async_trait::async_trait]
impl ActiveProfileReferenceLease for ActiveProfileSelectionLease {
    fn state(&self) -> &ActiveProfileReferenceState {
        &self.state
    }

    async fn into_owner_binding(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        let Self {
            state,
            config_root,
            _guard: root_guard,
        } = *self;
        let owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        tokio::task::spawn_blocking(move || {
            let selected_engine = match &state {
                ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
                ActiveProfileReferenceState::Unselected { .. } => {
                    return Err(PortError::Failed(
                        "an unselected profile cannot bind a Core owner".into(),
                    ));
                }
            };
            if selected_engine != identity.engine {
                return Err(PortError::Failed(format!(
                    "active profile engine {} does not match owner engine {}",
                    selected_engine.as_str(),
                    identity.engine.as_str()
                )));
            }
            let file = ActiveProfileReferenceFile::new(config_root.clone());
            let mut record = file
                .read_active_profile_record_sync()?
                .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration);
            if record.selection != state {
                return Err(PortError::Rejected(
                    infiltrator_contract::error::Failure::new(
                        infiltrator_contract::error::ErrorCode::NotReady,
                        "active profile changed before owner binding",
                        true,
                    ),
                ));
            }
            let _owner_state_guard = owner_state_guard;
            let owner_guard = file.try_acquire_owner_lock()?.ok_or_else(|| {
                PortError::Rejected(infiltrator_contract::error::Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "another process holds the active Core owner fence",
                    true,
                ))
            })?;
            let mut registrations = lock_local_owner_registry()?;
            if registrations.contains(&config_root) {
                return Err(PortError::Rejected(
                    infiltrator_contract::error::Failure::new(
                        infiltrator_contract::error::ErrorCode::NotReady,
                        "this process already has an active Core owner registration",
                        true,
                    ),
                ));
            }
            let mut owner_proof = file.read_owner_proof_sync()?;
            match &record.owner {
                ActiveCoreOwnerState::Unowned => {}
                ActiveCoreOwnerState::Bound { .. } => {
                    return Err(transition_rejected(
                        "a previous Core owner still needs explicit terminal normalization",
                    ));
                }
                ActiveCoreOwnerState::Switching { .. } => {
                    return Err(transition_rejected(
                        "an active Core owner switch is in progress",
                    ));
                }
            }
            match &owner_proof.state {
                ActiveCoreOwnerProof::Unowned | ActiveCoreOwnerProof::Terminal { .. } => {}
                ActiveCoreOwnerProof::Bound { .. } | ActiveCoreOwnerProof::Reserved { .. } => {
                    return Err(transition_rejected(
                        "an orphaned owner proof requires explicit terminal recovery",
                    ));
                }
            }
            let token = ActiveOwnerToken::generate()?;
            owner_proof.state = ActiveCoreOwnerProof::Bound {
                identity: identity.clone(),
                generation,
                token: token.clone(),
            };
            owner_proof.launch = ActiveCoreOwnerLaunchProof::Unstarted;
            record.owner = ActiveCoreOwnerState::Bound {
                identity: identity.clone(),
                generation,
                token: token.clone(),
            };
            file.publish_owner_proof_sync_reconciled(&owner_proof)?;
            file.publish_active_profile_record_sync_reconciled(&record)?;
            registrations.register_bound(
                config_root.clone(),
                token.clone(),
                identity.clone(),
                generation,
                owner_guard,
            )?;
            drop(root_guard);
            Ok(Box::new(ActiveCoreOwnerLease::new(
                identity,
                generation,
                token,
                config_root,
            )) as Box<dyn ActiveProfileOwnerLease>)
        })
        .await
        .map_err(|error| PortError::Failed(error.to_string()))?
    }
}
