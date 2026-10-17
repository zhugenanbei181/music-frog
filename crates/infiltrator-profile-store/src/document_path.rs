//! Product facade over the neutral engine-scoped document path resolution.

use crate::store::EngineProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn profile_document_path(&self, profile: &ProfileId) -> Result<PathBuf, PortError> {
        self.core().profile_document_path(profile).await
    }

    pub async fn existing_profile_document_path(
        &self,
        profile: &ProfileId,
    ) -> Result<PathBuf, PortError> {
        self.core().existing_profile_document_path(profile).await
    }
}
