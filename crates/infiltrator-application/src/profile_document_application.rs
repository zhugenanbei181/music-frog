//! DUAL-09-03/14: profile document use-cases for the editor surfaces.
//!
//! Both editors edit the stored profile document through this application:
//! [`ProfileDocumentApplication::load`] publishes raw document text and the
//! configured engine/format/schema codec's inspection result.
//! [`ProfileDocumentApplication::save`] validates through that scoped codec,
//! then commits the exact caller-observed workspace through the protected,
//! source-bound transaction. Format-specific snippets and live preflight are
//! delegated to the document codec adapter.

use crate::profile_application::{ProfileApplication, ProfileRuntimeAccess, valid_name};
use crate::profile_editor_observations::EditorFacet;
use infiltrator_contract::profile_document::{DocumentInspection, ProfileDocumentSnapshot};
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileId, ProfileWorkspaceIdentity,
};
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentSnippetOutcome,
};

#[derive(Clone)]
pub struct ProfileDocumentApplication {
    profiles: ProfileApplication,
}

impl ProfileDocumentApplication {
    pub fn new(profiles: ProfileApplication) -> Self {
        Self { profiles }
    }

    /// Run a local, engine-scoped preflight over the current editor buffer.
    /// The result contains only format-neutral diagnostics and performs no
    /// store or runtime I/O.
    pub fn inspect_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
    ) -> DocumentInspection {
        self.profiles
            .inspect_document_buffer(identity, content.as_bytes())
    }

    /// Insert a codec-owned editor snippet for a particular document identity.
    /// Foreign engine/format/schema documents fail closed before codec work.
    pub fn insert_snippet_for_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        self.profiles.insert_document_snippet(
            identity,
            content,
            snippet_id,
            ProfileDocumentCaret { line, column },
        )
    }

    /// Load the stored document and publish the scoped codec result.
    pub async fn load(&self, profile: Option<&str>) -> Result<ProfileDocumentSnapshot, Failure> {
        let name = match profile {
            Some(name) if !name.trim().is_empty() => name.to_string(),
            _ => self.profiles.current_profile().await?,
        };
        let name = valid_name(&name)?;
        let profile_id = ProfileId::new(name.clone())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let read = self
            .profiles
            .begin_editor_read(&name, EditorFacet::Document);
        self.profiles.mark_editor_loading(&read);
        let workspace = match self.profiles.load_workspace(&profile_id).await {
            Ok(workspace) => workspace,
            Err(failure) => {
                self.profiles.fail_editor_read(&read, failure.clone());
                return Err(failure);
            }
        };
        let document = match document_snapshot(&self.profiles, workspace) {
            Ok(document) => document,
            Err(failure) => {
                self.profiles.fail_editor_read(&read, failure.clone());
                return Err(failure);
            }
        };
        if !self.profiles.observe_document(&read, document.clone()) {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Profile document read was superseded",
                false,
            ));
        }
        Ok(document)
    }

    /// Commit an editor buffer as `profile`'s stored document.
    ///
    /// The configured codec validates before any store or runtime access.
    /// `authorization` carries the editor's explicit unlock; the application
    /// re-checks the stored subscription metadata.
    pub async fn save<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected: &ProfileWorkspaceIdentity,
        content: &str,
        authorization: ProfileWriteAuthorization,
    ) -> Result<ProfileDocumentSnapshot, Failure> {
        self.profiles
            .validate_document_write(expected, content.as_bytes())?;
        let name = expected.document.profile_id.as_str();
        let read = self.profiles.begin_editor_read(name, EditorFacet::Document);
        let workspace = self
            .profiles
            .load_workspace(&expected.document.profile_id)
            .await?;
        if workspace.identity() != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The observed profile document or options changed; inspect before saving",
                true,
            ));
        }
        let committed = self
            .profiles
            .commit_workspace(
                runtime,
                expected,
                &ProfileWorkspaceUpdate {
                    purpose: ProfileWorkspacePurpose::DirectEdit { authorization },
                    document_bytes: content.as_bytes().to_vec(),
                    sidecar_bytes: workspace.sidecar_bytes,
                },
            )
            .await?;
        let document = document_snapshot(&self.profiles, committed)?;
        self.profiles.observe_document(&read, document.clone());
        Ok(document)
    }
}

pub(crate) fn document_snapshot(
    profiles: &ProfileApplication,
    workspace: ProfileWorkspace,
) -> Result<ProfileDocumentSnapshot, Failure> {
    let content = workspace
        .document
        .utf8_text()
        .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), false))?
        .to_owned();
    let inspection = profiles.inspect_document(&workspace.document);
    let document =
        ProfileDocumentSnapshot::new(workspace.identity(), content, workspace.write_protection)
            .with_inspection(inspection);
    Ok(document)
}
