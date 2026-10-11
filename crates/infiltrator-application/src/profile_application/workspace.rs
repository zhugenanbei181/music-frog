//! Coherent document and sidecar reads and source-bound workspace commits.
use super::{ProfileApplication, valid_name};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{
    ProfileId, ProfileSidecarRevision, ProfileWorkspaceIdentity,
};
use infiltrator_contract::subscription_quota::QuotaProfileIdentity;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

impl ProfileApplication {
    pub(crate) fn engine_id(&self) -> EngineId {
        self.store.engine_id()
    }

    pub(crate) fn validate_expected_engine(
        &self,
        expected: &ProfileWorkspaceIdentity,
    ) -> Result<(), Failure> {
        let store_engine = self.store.engine_id();
        if expected.document.engine_id != store_engine {
            return Err(engine_mismatch(&expected.document.engine_id, &store_engine));
        }
        Ok(())
    }

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
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, Failure> {
        self.validate_expected_engine(expected)?;
        let store_engine = self.store.engine_id();
        let previous = self
            .store
            .load_workspace(&expected.document.profile_id)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&previous, &expected.document.profile_id, &store_engine)?;
        if previous.identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The profile workspace changed before the commit began",
                true,
            ));
        }
        let committed = if let Some(runtime) = runtime {
            let runtime_engine = runtime.engine_id().ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "Managed runtime has no authoritative engine identity",
                    false,
                )
            })?;
            if runtime_engine != expected.document.engine_id {
                return Err(engine_mismatch(
                    &expected.document.engine_id,
                    &runtime_engine,
                ));
            }
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
        validate_workspace(&committed, &expected.document.profile_id, &store_engine)?;
        if !same_document_namespace(expected, &committed.identity()) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed engine, profile, source, format, or schema",
                false,
            ));
        }
        if committed.document.raw_bytes != update.document_bytes
            || committed.sidecar_bytes != update.sidecar_bytes
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt does not match the submitted documents",
                false,
            ));
        }
        if previous.document.raw_bytes != update.document_bytes
            && committed.document.identity.revision == previous.document.identity.revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed document bytes without changing its revision",
                false,
            ));
        }
        if previous.sidecar_bytes != update.sidecar_bytes
            && committed.sidecar_revision == previous.sidecar_revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed sidecar bytes without changing its revision",
                false,
            ));
        }
        Ok(committed)
    }

    /// Commit an edit that was opened from the active-profile pointer. The
    /// store checks the active pointer and document revision together, so a
    /// concurrent profile switch cannot redirect or silently stale the write.
    pub async fn commit_active_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, Failure> {
        self.validate_expected_engine(expected)?;
        let store_engine = self.store.engine_id();
        let previous = self
            .store
            .load_workspace(&expected.document.profile_id)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&previous, &expected.document.profile_id, &store_engine)?;
        if previous.identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The profile workspace changed before the commit began",
                true,
            ));
        }
        let committed = self
            .store
            .compare_and_save_active_workspace(expected, update)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&committed, &expected.document.profile_id, &store_engine)?;
        if !same_document_namespace(expected, &committed.identity()) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed engine, profile, source, format, or schema",
                false,
            ));
        }
        if committed.document.raw_bytes != update.document_bytes
            || committed.sidecar_bytes != update.sidecar_bytes
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt does not match the submitted documents",
                false,
            ));
        }
        if previous.document.raw_bytes != update.document_bytes
            && committed.document.identity.revision == previous.document.identity.revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed document bytes without changing its revision",
                false,
            ));
        }
        if previous.sidecar_bytes != update.sidecar_bytes
            && committed.sidecar_revision == previous.sidecar_revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Workspace receipt changed sidecar bytes without changing its revision",
                false,
            ));
        }
        Ok(committed)
    }

    pub async fn load_workspace(&self, name: &str) -> Result<ProfileWorkspace, Failure> {
        let name = valid_name(name)?;
        let profile = ProfileId::new(name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        self.load_workspace_id(&profile).await
    }

    /// Read and validate the active profile's exact typed workspace identity
    /// under the profile-store owner boundary.
    pub async fn load_active_workspace(&self) -> Result<ProfileWorkspace, Failure> {
        let engine = self.store.engine_id();
        let workspace = self
            .store
            .load_active_workspace()
            .await
            .map_err(Failure::from)?;
        validate_workspace(&workspace, &workspace.document.identity.profile_id, &engine)?;
        Ok(workspace)
    }

    pub async fn load_workspace_id(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, Failure> {
        let engine = self.store.engine_id();
        let workspace = self
            .store
            .load_workspace(profile)
            .await
            .map_err(Failure::from)?;
        validate_workspace(&workspace, profile, &engine)?;
        Ok(workspace)
    }
}

fn validate_workspace(
    workspace: &ProfileWorkspace,
    profile: &ProfileId,
    engine: &EngineId,
) -> Result<(), Failure> {
    let workspace_identity = workspace.identity();
    let sidecar_present = matches!(
        workspace_identity.sidecar_revision,
        ProfileSidecarRevision::Present(_)
    );
    if workspace.document.identity.profile_id != *profile
        || workspace.document.identity.engine_id != *engine
        || workspace.sidecar_bytes.is_some() != sidecar_present
    {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "Workspace identity does not match its engine, profile, or sidecar bytes",
            false,
        ));
    }
    Ok(())
}

fn engine_mismatch(expected: &EngineId, actual: &EngineId) -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        format!(
            "profile engine `{}` does not match active engine `{}`",
            expected.as_str(),
            actual.as_str()
        ),
        false,
    )
}

fn same_document_namespace(
    expected: &ProfileWorkspaceIdentity,
    actual: &ProfileWorkspaceIdentity,
) -> bool {
    let expected = &expected.document;
    let actual = &actual.document;
    expected.engine_id == actual.engine_id
        && expected.profile_id == actual.profile_id
        && expected.source == actual.source
        && expected.format_id == actual.format_id
        && expected.schema_version == actual.schema_version
}
