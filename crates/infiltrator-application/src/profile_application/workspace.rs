//! Coherent document and sidecar reads and source-bound workspace commits.
use super::{ProfileApplication, valid_name};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::subscription_quota::QuotaProfileIdentity;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::{hash_document_bytes, identify_profile_source};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

impl ProfileApplication {
    pub async fn active_quota_identity(&self) -> Result<QuotaProfileIdentity, Failure> {
        let profile = self.current_profile().await?;
        if profile.is_empty() {
            return Ok(QuotaProfileIdentity {
                profile,
                provider_hash: None,
            });
        }
        let metadata = self.load_metadata(&profile).await?;
        let provider_hash = metadata
            .subscription_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(hash_document_bytes);
        Ok(QuotaProfileIdentity {
            profile,
            provider_hash,
        })
    }
    pub async fn commit_workspace<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, Failure> {
        let committed = if let Some(runtime) = runtime {
            runtime
                .apply_profile_workspace(expected, update, ApplyStrategy::PreferReload)
                .await
                .map_err(Failure::from)
        } else {
            self.store
                .compare_and_save_workspace(expected, update)
                .await
                .map_err(Failure::from)
        }?;
        validate_workspace(&committed, &expected.profile)?;
        if committed.content != update.content || committed.options != update.options {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt does not match the submitted documents",
                false,
            ));
        }
        Ok(committed)
    }

    /// DUAL-07-08: load a profile's option sidecar (filter + mixin).
    pub async fn load_options(&self, name: &str) -> Result<ProfileOptions, Failure> {
        let name = valid_name(name)?;
        self.store.load_options(&name).await.map_err(Failure::from)
    }

    pub async fn load_workspace(&self, name: &str) -> Result<ProfileWorkspace, Failure> {
        let name = valid_name(name)?;
        let workspace = self
            .store
            .load_workspace(&name)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&workspace, &name)?;
        Ok(workspace)
    }

    /// DUAL-07-08: persist a profile's option sidecar.
    pub async fn save_options(&self, name: &str, options: &ProfileOptions) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .save_options(&name, options)
            .await
            .map_err(Failure::from)
    }
}

fn validate_workspace(workspace: &ProfileWorkspace, profile: &str) -> Result<(), Failure> {
    let decoded: ProfileOptions = workspace
        .options_document
        .as_deref()
        .map(serde_yaml_ng::from_str)
        .transpose()
        .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), false))?
        .unwrap_or_default();
    if workspace.source.profile != profile
        || workspace.source
            != identify_profile_source(
                profile.to_owned(),
                &workspace.content,
                workspace.options_document.as_deref(),
            )
        || decoded != workspace.options
    {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "Workspace documents do not match their reported source",
            false,
        ));
    }
    Ok(())
}
