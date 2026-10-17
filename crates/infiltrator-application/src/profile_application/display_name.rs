use super::{ProfileApplication, valid_name};
use infiltrator_contract::profile_display_name::{
    ProfileDisplayNameMutation, ProfileDisplayNameObservation, ProfileDisplayNameTarget,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use lurker_core_contract::error::{ErrorCode, Failure};

impl ProfileApplication {
    /// Read the presentation label independently from the stable profile ID.
    pub async fn profile_display_name(
        &self,
        name: &str,
    ) -> Result<ProfileDisplayNameObservation, Failure> {
        let name = valid_name(name)?;
        let profile = ProfileId::new(name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        self.store
            .profile_display_name(&profile)
            .await
            .map_err(Failure::from)
    }

    /// Update or reset a display label without renaming files or changing the
    /// active profile reference. The store returns the exact persisted readback.
    pub async fn set_profile_display_name(
        &self,
        name: &str,
        target: ProfileDisplayNameTarget,
    ) -> Result<ProfileDisplayNameMutation, Failure> {
        let name = valid_name(name)?;
        let profile = ProfileId::new(name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        self.store
            .set_profile_display_name(&profile, target)
            .await
            .map_err(Failure::from)
    }
}
