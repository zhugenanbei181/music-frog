//! Source-bound application workflows for engine-owned profile option schemas.
//!
//! The selected document codec owns sidecar decoding, Mixin/filter schema
//! transforms, and format validation. This service resolves a coherent
//! workspace, delegates the pure transform to that scoped adapter, then
//! commits the returned opaque document and sidecar bytes through the shared
//! compare-and-save transaction.
use crate::profile_application::{ProfileApplication, ProfileRuntimeAccess, valid_name};
use crate::profile_document_application::document_snapshot;
use crate::profile_editor_observations::EditorFacet;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::ProfileWorkspaceReceipt;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_mixin_studio::ProfileMixinStudioObservation;
use infiltrator_contract::profile_mixin_studio::ProfileMixinToggleId;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::subscription_filter_result::SubscriptionFilterApplied;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};

/// Resolve the sidecar profile: an explicit name, or the active profile.
async fn resolve_profile(
    profiles: &ProfileApplication,
    profile: Option<&str>,
) -> Result<String, Failure> {
    match profile {
        Some(name) if !name.trim().is_empty() => valid_name(name),
        _ => profiles.current_profile().await,
    }
}

#[derive(Clone)]
pub struct ProfileOptionsApplication {
    profiles: ProfileApplication,
}

impl ProfileOptionsApplication {
    pub fn new(profiles: ProfileApplication) -> Self {
        Self { profiles }
    }

    /// Validate a live Mixin draft with the engine-scoped codec used by the
    /// sidecar save path. This is a pure operation: it parses and composes in
    /// memory, and performs no store or runtime I/O.
    pub fn inspect_mixin_draft(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.profiles
            .apply_profile_mixin(identity, document_content.as_bytes(), None, mixin_yaml)
            .map(|_| ())
    }

    pub fn inspect_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.profiles
            .inspect_profile_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    pub fn set_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.profiles
            .set_profile_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    pub fn inspect_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioObservation, Failure> {
        self.profiles.inspect_profile_mixin_studio(
            identity,
            document_content.as_bytes(),
            mixin_yaml,
        )
    }

    /// Load the sidecar, publish it for the surface projection and return it.
    /// `profile = None` selects the active profile.
    pub async fn load(&self, profile: Option<&str>) -> Result<ProfileOptionsSnapshot, Failure> {
        let name = resolve_profile(&self.profiles, profile).await?;
        let profile_id = ProfileId::new(name.clone())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let read = self.profiles.begin_editor_read(&name, EditorFacet::Options);
        self.profiles.mark_editor_loading(&read);
        let result = self
            .profiles
            .load_workspace(&profile_id)
            .await
            .and_then(|workspace| project_options(&self.profiles, workspace));
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(failure) => {
                self.profiles.fail_editor_read(&read, failure.clone());
                return Err(failure);
            }
        };
        if !self.profiles.observe_options(&read, snapshot.clone()) {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Profile options read was superseded",
                false,
            ));
        }
        Ok(snapshot)
    }

    /// Commit an edited Mixin overlay for `profile`.
    pub async fn save_mixin<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected: &ProfileWorkspaceIdentity,
        mixin_yaml: &str,
    ) -> Result<ProfileWorkspaceReceipt, Failure> {
        self.profiles
            .validate_profile_options_scope(&expected.document)?;
        let profile = expected.document.profile_id.as_str();
        let read = self
            .profiles
            .begin_editor_read(profile, EditorFacet::Options);
        let document_read = self
            .profiles
            .begin_editor_read(profile, EditorFacet::Document);
        let workspace = self
            .profiles
            .load_workspace(&expected.document.profile_id)
            .await?;
        if workspace.identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The observed Mixin document or options changed; inspect before saving",
                true,
            ));
        }
        let mutation = self.profiles.apply_profile_mixin(
            &workspace.document.identity,
            &workspace.document.raw_bytes,
            workspace.sidecar_bytes.as_deref(),
            mixin_yaml,
        )?;
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: mutation.document_bytes,
            sidecar_bytes: mutation.sidecar_bytes,
        };
        let committed = self
            .profiles
            .commit_workspace(runtime, expected, &update)
            .await?;
        let document = document_snapshot(&self.profiles, committed.clone())?;
        let options = ProfileOptionsSnapshot::new(
            committed.identity(),
            mutation.projection.mixin_yaml,
            mutation.projection.filter,
        );
        self.profiles
            .observe_document(&document_read, document.clone());
        self.profiles.observe_options(&read, options.clone());
        Ok(ProfileWorkspaceReceipt {
            previous: expected.clone(),
            submitted_mixin_yaml: mixin_yaml.into(),
            document,
            options,
        })
    }

    /// Apply a neutral filter draft through the selected engine codec and
    /// commit its opaque document and sidecar mutation atomically.
    pub async fn save_filter<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected: &ProfileWorkspaceIdentity,
        draft: &SubscriptionFilterDraft,
    ) -> Result<SubscriptionFilterApplied, Failure> {
        self.profiles
            .validate_profile_options_scope(&expected.document)?;
        let read = self
            .profiles
            .begin_editor_read(expected.document.profile_id.as_str(), EditorFacet::Options);
        let workspace = self
            .profiles
            .load_workspace(&expected.document.profile_id)
            .await?;
        if workspace.identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The filter source changed; inspect the current profile and options",
                true,
            ));
        }
        let mutation = self.profiles.apply_profile_filter(
            &workspace.document.identity,
            &workspace.document.raw_bytes,
            workspace.sidecar_bytes.as_deref(),
            draft,
        )?;
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: mutation.document_bytes,
            sidecar_bytes: mutation.sidecar_bytes,
        };
        let committed = self
            .profiles
            .commit_workspace(runtime, expected, &update)
            .await?;
        let report = SubscriptionFilterApplied {
            workspace_identity: committed.identity(),
            report: mutation.report,
        };
        // Publish the stored draft so a surface that just applied it (and any
        // other surface reading the same projection) sees the persisted form.
        self.profiles.observe_options(
            &read,
            ProfileOptionsSnapshot::new(
                report.workspace_identity.clone(),
                mutation.projection.mixin_yaml,
                mutation.projection.filter,
            ),
        );
        Ok(report)
    }
}

fn project_options(
    profiles: &ProfileApplication,
    workspace: ProfileWorkspace,
) -> Result<ProfileOptionsSnapshot, Failure> {
    let projection = profiles.inspect_profile_options(
        &workspace.document.identity,
        workspace.sidecar_bytes.as_deref(),
    )?;
    Ok(ProfileOptionsSnapshot::new(
        workspace.identity(),
        projection.mixin_yaml,
        projection.filter,
    ))
}
