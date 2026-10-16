//! DUAL-12-08: desktop host adapter for the tracer's one-click reverse-apply.
//!
//! Loads the current profile's rule list and commits a rewritten list through
//! the same CORE-004 apply transaction the editor save path uses (atomic write,
//! reload-or-restart, readiness, rollback). It shares the runtime's apply
//! guard so a tracer override cannot interleave with another config apply.

use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_trace_run::RuleTraceOwnerBinding;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
};
use infiltrator_ports::profile_document_codec::ProfileRuleWorkspaceUpdate;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use mihomo_config::manager::ConfigManager;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

/// Host persistence capability behind the shared rule override command.
pub struct DesktopRuleOverridePort {
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    profiles: ProfileApplication,
    session: Weak<CoreApplication>,
    configuration_transaction: Arc<dyn ProfileConfigurationTransactionPort>,
    apply_guard: Arc<Mutex<()>>,
}

impl DesktopRuleOverridePort {
    pub fn new(
        config: Arc<ConfigManager<DefaultCredentialStore>>,
        profiles: ProfileApplication,
        session: Arc<CoreApplication>,
        configuration_transaction: Arc<dyn ProfileConfigurationTransactionPort>,
        apply_guard: Arc<Mutex<()>>,
    ) -> Self {
        Self {
            config,
            profiles,
            session: Arc::downgrade(&session),
            configuration_transaction,
            apply_guard,
        }
    }
    async fn commit_updated(
        &self,
        expected: &ProfileWorkspaceIdentity,
        profile: &str,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<(), PortError> {
        let _session = self.session.upgrade().ok_or_else(|| {
            PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The product session has closed",
                false,
            ))
        })?;
        let expected_reference =
            ProfileStore::read_active_profile_reference(self.config.engine_profile_store()).await?;
        let ActiveProfileReferenceState::Selected(
            expected_reference,
        ) = expected_reference
        else {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "rule override requires a selected active profile",
                false,
            )));
        };
        if expected_reference.engine_id != expected.document.engine_id
            || expected_reference.profile_id != expected.document.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "rule override workspace no longer belongs to the selected profile",
                true,
            )));
        }
        let authorization = _session
            .active_profile_reference_write_authorization()
            .map_err(PortError::Rejected)?;
        let outcome = self
            .configuration_transaction
            .transact(ProfileConfigurationTransactionRequest {
                expected_reference,
                expected: expected.clone(),
                update: update.clone(),
                authorization,
                strategy: ApplyStrategy::PreferReload,
            })
            .await?;
        match outcome {
            ProfileConfigurationTransactionOutcome::Committed {
                effect: ProfileConfigurationCommitEffect::AppliedToRunningCore,
                ..
            }
            | ProfileConfigurationTransactionOutcome::Committed {
                effect: ProfileConfigurationCommitEffect::StoredWhileStopped,
                ..
            }
            | ProfileConfigurationTransactionOutcome::Committed {
                effect: ProfileConfigurationCommitEffect::StoredWhileFailed,
                ..
            } => Ok(()),
            ProfileConfigurationTransactionOutcome::RolledBack { cause, .. } => {
                Err(PortError::Rejected(Failure::new(
                    cause.code,
                    format!("profile {profile}: {}", cause.message),
                    cause.retryable,
                )))
            }
            ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } => {
                Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "profile {profile}: {}; rollback failed: {}",
                        cause.message, rollback.message
                    ),
                    true,
                )))
            }
        }
    }
}

#[async_trait]
impl RuleOverridePort for DesktopRuleOverridePort {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        let workspace =
            ProfileStore::load_active_workspace(self.config.engine_profile_store()).await?;
        self.profiles
            .read_profile_rule_workspace(&workspace)
            .map_err(PortError::Rejected)
    }

    async fn load_rule_workspace_for_trace(
        &self,
        owner: &RuleTraceOwnerBinding,
    ) -> Result<RuleWorkspace, PortError> {
        if owner.workspace.document.engine_id != owner.scope.identity.engine {
            return Err(PortError::Rejected(Failure::unsupported(
                "rule trace profile belongs to another engine",
            )));
        }
        let session = self.session.upgrade().ok_or_else(|| {
            PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The product session has closed",
                false,
            ))
        })?;
        if session.snapshot().scope().as_ref() != Some(&owner.scope) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Core scope changed before rule simulation began",
                true,
            )));
        }
        let workspace =
            ProfileStore::load_active_workspace(self.config.engine_profile_store()).await?;
        if workspace.identity() != owner.workspace {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Profile workspace changed before rule simulation began",
                true,
            )));
        }
        self.profiles
            .read_profile_rule_workspace(&workspace)
            .map_err(PortError::Rejected)
    }

    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        let guard = self.apply_guard.lock().await;
        let workspace =
            ProfileStore::load_active_workspace(self.config.engine_profile_store()).await?;
        let identity = workspace.identity();
        let profile = identity.document.profile_id.as_str().to_owned();
        let observed = self
            .profiles
            .read_profile_rule_workspace(&workspace)
            .map_err(PortError::Rejected)?;
        if workspace.identity() != expected.profile_identity
            || observed.source != expected.source
            || observed.rules != expected.rules
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The profile or rules changed; run the simulation again before applying",
                true,
            )));
        }
        let mutation = self
            .profiles
            .apply_profile_rule_workspace(
                &identity,
                &workspace.document.raw_bytes,
                ProfileRuleWorkspaceUpdate::ReplaceRules(entries.to_vec()),
            )
            .map_err(PortError::Rejected)?;
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: mutation.document_bytes,
            sidecar_bytes: workspace.sidecar_bytes,
        };
        drop(guard);
        self.commit_updated(&identity, &profile, &update).await
    }
    async fn compare_and_apply_named_rule(
        &self,
        expected: &RuleWorkspace,
        location: &RuleLocation,
        expected_rule: &str,
        replacement: &str,
    ) -> Result<(), PortError> {
        let guard = self.apply_guard.lock().await;
        let workspace =
            ProfileStore::load_active_workspace(self.config.engine_profile_store()).await?;
        let identity = workspace.identity();
        let profile = identity.document.profile_id.as_str().to_owned();
        let observed = self
            .profiles
            .read_profile_rule_workspace(&workspace)
            .map_err(PortError::Rejected)?;
        if workspace.identity() != expected.profile_identity || observed.source != expected.source {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The source document changed before applying",
                true,
            )));
        }
        let mutation = self
            .profiles
            .apply_profile_rule_workspace(
                &identity,
                &workspace.document.raw_bytes,
                ProfileRuleWorkspaceUpdate::ReplaceNamedRule {
                    location: location.clone(),
                    expected_rule: expected_rule.to_owned(),
                    replacement: replacement.to_owned(),
                },
            )
            .map_err(PortError::Rejected)?;
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: mutation.document_bytes,
            sidecar_bytes: workspace.sidecar_bytes,
        };
        drop(guard);
        self.commit_updated(&identity, &profile, &update).await
    }
}
