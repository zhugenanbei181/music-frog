use super::super::backend_conformance::profile_support::{
    ConformanceLifecycleSession, ConformanceProfileStore,
};
use async_trait::async_trait;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
    ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspaceRecoveryResolution, ProfileStore,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

pub(super) struct FakeNativeProfile {
    pub(super) scope: ProfileConfigurationTransactionScope,
    pub(super) valid_profile: Vec<u8>,
    pub(super) native_invalid_profile: Vec<u8>,
    pub(super) store: Arc<ConformanceProfileStore>,
    pub(super) transaction_owner: Arc<ConformanceLifecycleSession>,
}

#[async_trait]
impl ProfileConfigurationTransactionPort for FakeNativeProfile {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        if !self.scope.supports(&request.expected) {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "fake transaction rejects a foreign scope",
            ));
        }
        let previous = self.store.load_active_workspace().await?;
        if previous.identity() != request.expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake transaction rejects a stale workspace revision",
                false,
            )));
        }
        if request.update.document_bytes == self.native_invalid_profile {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "fake native check rejects semantic-invalid document",
                false,
            )));
        }
        if request.update.document_bytes != self.valid_profile {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "fake native check rejects an unregistered document",
                false,
            )));
        }
        let observed_lifecycle = self.transaction_owner.lifecycle();
        if observed_lifecycle == CoreLifecycle::Running
            && self.scope.document.engine_id.as_str() == "sing-box"
        {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "fake sing-box transaction does not apply while Core is running",
            ));
        }
        let workspace_update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: request.update.document_bytes,
            sidecar_bytes: request.update.sidecar_bytes,
        };
        let lease = self
            .store
            .acquire_active_workspace_write_lease(
                request.expected_reference,
                request.expected,
                request.authorization,
            )
            .await?;
        let committed = match lease.publish(workspace_update).await {
            ActiveProfileWorkspacePublishOutcome::Published(published) => published.commit(),
            ActiveProfileWorkspacePublishOutcome::Unpublished(error) => return Err(error),
            ActiveProfileWorkspacePublishOutcome::Superseded { observed } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!("fake transaction observed stale workspace {observed:?}"),
                    false,
                )));
            }
            ActiveProfileWorkspacePublishOutcome::RecoveryRequired(mut recovery) => loop {
                match recovery.resolve().await {
                    ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                        break published.commit();
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                        restored.finish();
                        return Err(PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            "fake transaction publication was restored during recovery",
                            false,
                        )));
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                        return Err(error);
                    }
                    ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                        return Err(PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            format!(
                                "fake transaction recovery observed stale workspace {observed:?}"
                            ),
                            false,
                        )));
                    }
                    ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                        recovery = next;
                    }
                }
            },
        };
        let effect = match observed_lifecycle {
            CoreLifecycle::Stopped => ProfileConfigurationCommitEffect::StoredWhileStopped,
            CoreLifecycle::Ready | CoreLifecycle::Running => {
                ProfileConfigurationCommitEffect::AppliedToRunningCore
            }
            CoreLifecycle::Failed => ProfileConfigurationCommitEffect::StoredWhileFailed,
            CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake transaction rejects a Core transition",
                    false,
                )));
            }
        };
        Ok(ProfileConfigurationTransactionOutcome::Committed {
            workspace: committed,
            effect,
        })
    }
}
