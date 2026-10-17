//! Product facade over the neutral coherent-workspace kernel.
//!
//! The kernel validates and publishes exact document/sidecar bytes under caller
//! held fences. This module supplies the product-owned profile source facts
//! (write protection from the engine settings document and the subscription URL
//! stored through the product credential store) and keeps the historical
//! [`EngineProfileStore`] surface unchanged.

use crate::store::EngineProfileStore;
use crate::workspace::{product_workspace, workspace_record};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_ports::error::PortError;
use lurker_profile_store::workspace::ProfileSourceFacts;
use lurker_profile_store::workspace_store::{
    ProfileWorkspaceEdit, ProfileWorkspacePublicationTarget,
};

impl<S: SecureStore> EngineProfileStore<S> {
    /// Resolve the product-selected workspace under a retained root selection
    /// lease, then acquire the engine profile lock in root-to-profile order.
    pub async fn read_active_workspace_under_selection_lease(
        &self,
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<ProfileWorkspace, PortError> {
        let facts = ProductProfileSourceFacts { store: self };
        Ok(product_workspace(
            self.core()
                .read_active_workspace_under_selection(selection.state(), &facts)
                .await?,
        ))
    }

    /// Read one coherent document and sidecar while the caller holds the
    /// cross-process profile-write boundary.
    pub async fn read_profile_workspace_locked(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        let facts = ProductProfileSourceFacts { store: self };
        Ok(product_workspace(
            self.core()
                .read_profile_workspace_locked(profile, &facts)
                .await?,
        ))
    }

    /// Read a coherent workspace under the shared profile-write lock.
    pub async fn read_profile_workspace(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        let facts = ProductProfileSourceFacts { store: self };
        Ok(product_workspace(
            self.core().read_profile_workspace(profile, &facts).await?,
        ))
    }

    /// Validate recovery bytes before callers acquire filesystem locks or
    /// perform path access.
    pub fn validate_profile_workspace_recovery(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<(), PortError> {
        self.core()
            .validate_profile_workspace_recovery(expected, &workspace_record(previous))
    }

    /// Publish exact document and sidecar bytes after the caller has acquired
    /// the selection, owner-state, and profile-write fences.
    pub async fn publish_profile_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        target: ProfileWorkspacePublicationTarget<'_>,
    ) -> Result<ProfileWorkspace, PortError> {
        let facts = ProductProfileSourceFacts { store: self };
        Ok(product_workspace(
            self.core()
                .publish_profile_workspace_locked(
                    &facts,
                    expected,
                    workspace_edit(update.purpose),
                    &update.document_bytes,
                    update.sidecar_bytes.as_deref(),
                    target,
                )
                .await?,
        ))
    }

    /// Restore exact previous bytes while the caller still holds all write
    /// fences. This retains the same source and revision compare rules as a
    /// normal workspace publication.
    pub async fn restore_profile_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
        target: ProfileWorkspacePublicationTarget<'_>,
    ) -> Result<ProfileWorkspace, PortError> {
        let facts = ProductProfileSourceFacts { store: self };
        Ok(product_workspace(
            self.core()
                .restore_profile_workspace_locked(
                    &facts,
                    expected,
                    &workspace_record(previous),
                    target,
                )
                .await?,
        ))
    }

    /// Product write-protection rule: a profile with a recorded subscription
    /// URL is a remote subscription and needs an explicit unlock to edit
    /// directly.
    async fn workspace_write_protection(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWriteProtection, PortError> {
        let settings = self.read_settings_value().await?;
        let Some(profiles) = settings.get("profiles") else {
            return Ok(ProfileWriteProtection::Editable);
        };
        let profiles = profiles.as_table().ok_or_else(invalid_metadata)?;
        let Some(metadata) = profiles.get(profile.as_str()) else {
            return Ok(ProfileWriteProtection::Editable);
        };
        let metadata = metadata.as_table().ok_or_else(invalid_metadata)?;
        for field in ["subscription_url", "subscription_url_key"] {
            if let Some(value) = metadata.get(field) {
                let value = value.as_str().ok_or_else(invalid_metadata)?;
                if !value.trim().is_empty() {
                    return Ok(ProfileWriteProtection::RemoteSubscription);
                }
            }
        }
        Ok(ProfileWriteProtection::Editable)
    }
}

fn workspace_edit(purpose: ProfileWorkspacePurpose) -> ProfileWorkspaceEdit {
    match purpose {
        ProfileWorkspacePurpose::Derived => ProfileWorkspaceEdit::Derived,
        ProfileWorkspacePurpose::DirectEdit { authorization } => {
            ProfileWorkspaceEdit::DirectEdit { authorization }
        }
    }
}

fn invalid_metadata() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Storage,
        "Invalid persisted profile ownership metadata",
        false,
    ))
}

/// Product facts that classify a profile's document source.
struct ProductProfileSourceFacts<'a, S: SecureStore> {
    store: &'a EngineProfileStore<S>,
}

#[async_trait::async_trait]
impl<S: SecureStore> ProfileSourceFacts for ProductProfileSourceFacts<'_, S> {
    async fn write_protection(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWriteProtection, PortError> {
        self.store.workspace_write_protection(profile).await
    }

    async fn subscription_url(&self, profile: &ProfileId) -> Result<Option<String>, PortError> {
        Ok(self
            .store
            .get_profile_metadata(profile.as_str())
            .await?
            .subscription_url)
    }
}

#[cfg(test)]
#[path = "workspace_store_tests.rs"]
mod tests;
