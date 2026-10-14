//! Owner-authorized Age credential editing, separate from profile snapshots.

use super::ProfileApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_age_credential::{
    ProfileAgeCredentialMutation, ProfileAgeCredentialReadOutcome, ProfileAgeCredentialReadRequest,
    ProfileAgeCredentialWriteRequest,
};
use infiltrator_contract::profile_document_identity::ProfileId;

impl ProfileApplication {
    /// Read Age-key presence and revision through the exact selected-reference
    /// snapshot and Core-owner authorization. The secret is never returned.
    pub async fn profile_age_credential(
        &self,
        profile_id: ProfileId,
        expected_selection: ActiveProfileReferenceState,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ProfileAgeCredentialReadOutcome, Failure> {
        self.store
            .profile_age_credential(ProfileAgeCredentialReadRequest {
                profile_id,
                expected_selection,
                authorization,
            })
            .await
            .map_err(Failure::from)
    }

    /// Apply a typed Age-key edit and return only the secure owner's
    /// non-secret readback. Storage errors are sanitized by the owner adapter.
    pub async fn mutate_profile_age_credential(
        &self,
        request: ProfileAgeCredentialWriteRequest,
    ) -> Result<ProfileAgeCredentialMutation, Failure> {
        if request.expected.engine_id != self.store.engine_id() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Age credential request belongs to another engine",
                false,
            ));
        }
        self.store
            .mutate_profile_age_credential(request)
            .await
            .map_err(Failure::from)
    }
}
