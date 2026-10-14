//! Owner-authorized Age credential editing, separate from profile snapshots.

use super::ProfileApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_age_credential::{
    ProfileAgeContentDecryption, ProfileAgeCredentialMutation, ProfileAgeCredentialReadOutcome,
    ProfileAgeCredentialReadRequest, ProfileAgeCredentialWriteRequest, ProfileAgeDecryptedContent,
    ProfileAgeEncryptedContent,
};
use infiltrator_contract::profile_document_identity::ProfileId;

pub(super) enum PreparedSubscriptionContent<'a> {
    Plaintext(&'a str),
    AgeDecrypted(ProfileAgeDecryptedContent),
}

impl PreparedSubscriptionContent<'_> {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Plaintext(content) => content,
            Self::AgeDecrypted(content) => content.as_str(),
        }
    }
}

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

    pub(super) async fn prepare_subscription_content<'a>(
        &self,
        profile_id: &ProfileId,
        content: &'a str,
    ) -> Result<PreparedSubscriptionContent<'a>, Failure> {
        if !ProfileAgeEncryptedContent::is_armored_age_document(content) {
            return Ok(PreparedSubscriptionContent::Plaintext(content));
        }
        let encrypted = ProfileAgeEncryptedContent::new(content.to_owned())
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        match self
            .store
            .decrypt_age_subscription_content(profile_id, encrypted)
            .await
            .map_err(Failure::from)?
        {
            ProfileAgeContentDecryption::Decrypted { content, .. } => {
                Ok(PreparedSubscriptionContent::AgeDecrypted(content))
            }
            ProfileAgeContentDecryption::CredentialNotConfigured => Err(Failure::new(
                ErrorCode::Configuration,
                "Age-encrypted subscription requires a configured Age private key",
                false,
            )),
            ProfileAgeContentDecryption::InvalidCredentialOrContent => Err(Failure::new(
                ErrorCode::Configuration,
                "Age private key could not decrypt this subscription content",
                false,
            )),
        }
    }
}
