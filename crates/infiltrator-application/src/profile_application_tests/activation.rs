//! Revision-fenced active-profile activation behavior.

use super::*;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryLease, ActiveProfileWorkspaceRecoveryResolution,
    ActiveProfileWorkspaceRestoreOutcome,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreGeneration;
use lurker_core_contract::identity::CoreIdentity;
use lurker_core_contract::identity::CoreInstanceId;
use lurker_core_contract::identity::EngineId;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

enum ApplyBehavior {
    Succeed,
    Fail,
    SwitchThenFail(String),
}

struct ActivationRuntime {
    engine: EngineId,
    store: Arc<FakeStore>,
    behaviors: Mutex<VecDeque<ApplyBehavior>>,
    calls: AtomicUsize,
    references: Mutex<Vec<ActiveProfileReference>>,
    lifecycle: Mutex<CoreLifecycle>,
}

impl ActivationRuntime {
    fn new(store: Arc<FakeStore>, behaviors: impl IntoIterator<Item = ApplyBehavior>) -> Self {
        let engine = EngineId::new("mihomo").expect("test engine ID");
        store.bind_owner_for_test(
            CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("activation-test-core").expect("test owner instance"),
                None,
            ),
            CoreGeneration::new(1),
        );
        Self {
            engine,
            store,
            behaviors: Mutex::new(behaviors.into_iter().collect()),
            calls: AtomicUsize::new(0),
            references: Mutex::new(Vec::new()),
            lifecycle: Mutex::new(CoreLifecycle::Running),
        }
    }

    fn set_lifecycle(&self, lifecycle: CoreLifecycle) {
        *self.lifecycle.lock().expect("runtime lifecycle") = lifecycle;
    }
}

#[async_trait]
impl ActiveProfileReferenceApplier for ActivationRuntime {
    fn engine_id(&self) -> Option<EngineId> {
        Some(self.engine.clone())
    }

    fn core_lifecycle(&self) -> CoreLifecycle {
        self.lifecycle.lock().expect("runtime lifecycle").clone()
    }

    fn write_authorization(&self) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        Ok(ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: CoreIdentity::new(
                self.engine.clone(),
                CoreInstanceId::new("activation-test-core").expect("test owner instance"),
                None,
            ),
            generation: CoreGeneration::new(1),
        })
    }

    async fn apply_active_profile_reference(
        &self,
        expected: &ActiveProfileReference,
        _: ApplyStrategy,
    ) -> Result<u64, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.references
            .lock()
            .expect("runtime references")
            .push(expected.clone());
        if self.store.active_reference() != ActiveProfileReferenceState::Selected(expected.clone())
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "runtime observed a different active profile reference",
                true,
            )));
        }
        match self
            .behaviors
            .lock()
            .expect("runtime behavior")
            .pop_front()
            .unwrap_or(ApplyBehavior::Succeed)
        {
            ApplyBehavior::Succeed => Ok(2),
            ApplyBehavior::Fail => Err(PortError::Failed("target apply failed".to_owned())),
            ApplyBehavior::SwitchThenFail(profile) => {
                self.store.set_external_current(&profile)?;
                Err(PortError::Failed(
                    "target apply failed after a newer selection".to_owned(),
                ))
            }
        }
    }

    async fn apply_active_profile_workspace(
        &self,
        expected_reference: &ActiveProfileReference,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        authorization: ActiveProfileReferenceWriteAuthorization,
        strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        let lease = self
            .store
            .acquire_active_workspace_write_lease(
                expected_reference.clone(),
                expected.clone(),
                authorization,
            )
            .await?;
        let published = resolve_fake_publish(lease.publish(update.clone()).await).await?;
        match ActiveProfileReferenceApplier::apply_active_profile_reference(
            self,
            expected_reference,
            strategy,
        )
        .await
        {
            Ok(_) => Ok(published.commit()),
            Err(apply_error) => {
                recover_fake_workspace_after_apply_failure(published, apply_error).await
            }
        }
    }
}

async fn resolve_fake_publish<'a>(
    outcome: ActiveProfileWorkspacePublishOutcome<'a>,
) -> Result<Box<dyn ActiveProfileWorkspacePublishedLease + 'a>, PortError> {
    match outcome {
        ActiveProfileWorkspacePublishOutcome::Published(published) => Ok(published),
        ActiveProfileWorkspacePublishOutcome::Unpublished(error) => Err(error),
        ActiveProfileWorkspacePublishOutcome::Superseded { .. } => Err(PortError::Rejected(
            Failure::new(ErrorCode::NotReady, "fake publication was superseded", true),
        )),
        ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery) => {
            resolve_fake_workspace_recovery(recovery).await
        }
    }
}

async fn resolve_fake_workspace_recovery<'a>(
    mut recovery: Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>,
) -> Result<Box<dyn ActiveProfileWorkspacePublishedLease + 'a>, PortError> {
    loop {
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                return Ok(published);
            }
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                let _ = restored.finish();
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake publication recovery restored the previous workspace",
                    true,
                )));
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => return Err(error),
            ActiveProfileWorkspaceRecoveryResolution::Superseded { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake publication recovery was superseded",
                    true,
                )));
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                recovery = next;
            }
        }
    }
}

async fn recover_fake_workspace_after_apply_failure<'a>(
    mut published: Box<dyn ActiveProfileWorkspacePublishedLease + 'a>,
    apply_error: PortError,
) -> Result<ProfileWorkspace, PortError> {
    'restore: loop {
        match published.restore_if_current().await? {
            ActiveProfileWorkspaceRestoreOutcome::Restored(restored) => {
                let _ = restored.finish();
                return Err(apply_error);
            }
            ActiveProfileWorkspaceRestoreOutcome::Superseded { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake apply failed after a later workspace edit",
                    true,
                )));
            }
            ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(mut recovery) => loop {
                match recovery.resolve().await {
                    ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                        let _ = restored.finish();
                        return Err(apply_error);
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Published(next) => {
                        published = next;
                        continue 'restore;
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                        return Err(error);
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Superseded { .. } => {
                        return Err(PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            "fake apply recovery was superseded",
                            true,
                        )));
                    }
                    ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                        recovery = next;
                    }
                }
            },
        }
    }
}

fn store_with_profiles(current: &str, profiles: &[&str]) -> Arc<FakeStore> {
    let store = Arc::new(FakeStore::with_profile(current, "mode: rule\n", true));
    let mut saved = store.profiles.lock().expect("profiles");
    for profile in profiles {
        saved
            .entry((*profile).to_owned())
            .or_insert_with(|| ("mode: rule\n".to_owned(), ProfileMetadata::default()));
    }
    drop(saved);
    store
}

fn target(profile: &str) -> ActiveProfileReferenceTarget {
    ActiveProfileReferenceTarget::selected(
        EngineId::new("mihomo").expect("test engine ID"),
        ProfileId::new(profile).expect("test profile ID"),
    )
}

fn selected_profile(state: &ActiveProfileReferenceState) -> (&str, u64) {
    match state {
        ActiveProfileReferenceState::Selected(reference) => {
            (reference.profile_id.as_str(), reference.revision.value())
        }
        ActiveProfileReferenceState::Unselected { .. } => panic!("expected selected reference"),
    }
}

#[path = "activation/profile_content.rs"]
mod profile_content;
#[path = "activation/profile_selection.rs"]
mod profile_selection;
#[path = "activation/recovery.rs"]
mod recovery;
#[path = "activation/subscription.rs"]
mod subscription;
