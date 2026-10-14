//! Secure, engine-scoped Age credential editing with owner-fenced readback.

use crate::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use crate::core_identity::{CoreScope, EngineId};
use crate::profile_document_identity::ProfileId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error;
use std::fmt::{self, Debug, Display, Formatter};
use std::ptr;
use std::sync::atomic::{Ordering, compiler_fence};

const MAX_AGE_PRIVATE_KEY_BYTES: usize = 4096;
const MAX_AGE_ENCRYPTED_CONTENT_BYTES: usize = 20 * 1024 * 1024;
const AGE_ENCRYPTED_FILE_HEADER: &str = "-----BEGIN AGE ENCRYPTED FILE-----";

/// Age private key input. It is intentionally neither serializable nor
/// displayable; `Debug` is always redacted and its owned bytes are scrubbed on
/// drop. Transport adapters must map it explicitly to a secure request.
pub struct AgePrivateKey(String);

impl AgePrivateKey {
    pub const MAX_BYTES: usize = MAX_AGE_PRIVATE_KEY_BYTES;

    pub fn new(mut value: String) -> Result<Self, AgePrivateKeyError> {
        let error = if value.is_empty() || value.trim().is_empty() {
            Some(AgePrivateKeyError::Empty)
        } else if value.len() > MAX_AGE_PRIVATE_KEY_BYTES {
            Some(AgePrivateKeyError::TooLong)
        } else if value.trim() != value || value.chars().any(char::is_control) {
            Some(AgePrivateKeyError::InvalidWhitespace)
        } else {
            None
        };
        match error {
            Some(error) => {
                scrub_string(&mut value);
                Err(error)
            }
            None => Ok(Self(value)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct a private key from a one-shot byte payload and erase the
    /// source bytes on both valid and invalid UTF-8 paths.
    pub fn from_bytes(value: Vec<u8>) -> Result<Self, AgePrivateKeyError> {
        if value.len() > MAX_AGE_PRIVATE_KEY_BYTES {
            let mut bytes = value;
            scrub_bytes(&mut bytes);
            return Err(AgePrivateKeyError::TooLong);
        }
        let text = match String::from_utf8(value) {
            Ok(text) => text,
            Err(error) => {
                let mut bytes = error.into_bytes();
                scrub_bytes(&mut bytes);
                return Err(AgePrivateKeyError::InvalidEncoding);
            }
        };
        Self::new(text)
    }

    pub fn matches(&self, other: &Self) -> bool {
        let left = self.0.as_bytes();
        let right = other.0.as_bytes();
        let mut difference = left.len() ^ right.len();
        for index in 0..left.len().max(right.len()) {
            difference |= usize::from(
                left.get(index).copied().unwrap_or_default()
                    ^ right.get(index).copied().unwrap_or_default(),
            );
        }
        difference == 0
    }
}

/// Raw one-shot bytes for the Android owner IPC path. The UI process may hold
/// this while submitting a credential, but it is never serialized, formatted,
/// or converted to `AgePrivateKey` before arriving in the owner process.
pub struct ProfileAgeCredentialSecret(Vec<u8>);

impl ProfileAgeCredentialSecret {
    pub fn new(mut value: Vec<u8>) -> Result<Self, AgePrivateKeyError> {
        if value.len() > MAX_AGE_PRIVATE_KEY_BYTES {
            scrub_bytes(&mut value);
            return Err(AgePrivateKeyError::TooLong);
        }
        Ok(Self(value))
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Debug for ProfileAgeCredentialSecret {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProfileAgeCredentialSecret([REDACTED])")
    }
}

impl Drop for ProfileAgeCredentialSecret {
    fn drop(&mut self) {
        scrub_bytes(&mut self.0);
    }
}

impl Debug for AgePrivateKey {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("AgePrivateKey([REDACTED])")
    }
}

impl PartialEq for AgePrivateKey {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other)
    }
}

impl Eq for AgePrivateKey {}

impl Drop for AgePrivateKey {
    fn drop(&mut self) {
        scrub_string(&mut self.0);
    }
}

/// A bounded, armored Age document accepted only by the Core subscription
/// decoder. It is not serializable or printable.
pub struct ProfileAgeEncryptedContent(String);

impl ProfileAgeEncryptedContent {
    pub fn is_armored_age_document(value: &str) -> bool {
        value.trim_start().starts_with(AGE_ENCRYPTED_FILE_HEADER)
    }

    pub fn new(mut value: String) -> Result<Self, ProfileAgeEncryptedContentError> {
        let error = if value.len() > MAX_AGE_ENCRYPTED_CONTENT_BYTES {
            Some(ProfileAgeEncryptedContentError::TooLong)
        } else if !Self::is_armored_age_document(&value) {
            Some(ProfileAgeEncryptedContentError::NotArmoredAge)
        } else {
            None
        };
        match error {
            Some(error) => {
                scrub_string(&mut value);
                Err(error)
            }
            None => Ok(Self(value)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Debug for ProfileAgeEncryptedContent {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProfileAgeEncryptedContent([REDACTED])")
    }
}

impl Drop for ProfileAgeEncryptedContent {
    fn drop(&mut self) {
        scrub_string(&mut self.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileAgeEncryptedContentError {
    TooLong,
    NotArmoredAge,
}

impl Display for ProfileAgeEncryptedContentError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLong => "Age-encrypted profile content exceeds the supported size",
            Self::NotArmoredAge => "profile content is not an armored Age document",
        })
    }
}

impl error::Error for ProfileAgeEncryptedContentError {}

/// Decrypted profile text for immediate validation and owner commit. The
/// value cannot be formatted, serialized, or cloned and is scrubbed on drop.
pub struct ProfileAgeDecryptedContent(String);

impl ProfileAgeDecryptedContent {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Debug for ProfileAgeDecryptedContent {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProfileAgeDecryptedContent([REDACTED])")
    }
}

impl Drop for ProfileAgeDecryptedContent {
    fn drop(&mut self) {
        scrub_string(&mut self.0);
    }
}

#[derive(Debug)]
pub enum ProfileAgeContentDecryption {
    CredentialNotConfigured,
    InvalidCredentialOrContent,
    Decrypted {
        content: ProfileAgeDecryptedContent,
        credential_revision: ProfileAgeCredentialRevision,
    },
}

impl<'de> Deserialize<'de> for AgePrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgePrivateKeyError {
    Empty,
    TooLong,
    InvalidWhitespace,
    InvalidEncoding,
}

impl Display for AgePrivateKeyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "Age private key must not be empty",
            Self::TooLong => "Age private key exceeds the supported size",
            Self::InvalidWhitespace => "Age private key must be a single trimmed line",
            Self::InvalidEncoding => "Age private key must use valid UTF-8 encoding",
        })
    }
}

impl error::Error for AgePrivateKeyError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileAgeCredentialRevision(u64);

impl ProfileAgeCredentialRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileAgeCredentialState {
    NotConfigured,
    Configured,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileAgeCredentialObservation {
    pub engine_id: EngineId,
    pub profile_id: ProfileId,
    pub revision: ProfileAgeCredentialRevision,
    pub state: ProfileAgeCredentialState,
}

impl ProfileAgeCredentialObservation {
    pub fn new(
        engine_id: EngineId,
        profile_id: ProfileId,
        revision: ProfileAgeCredentialRevision,
        state: ProfileAgeCredentialState,
    ) -> Self {
        Self {
            engine_id,
            profile_id,
            revision,
            state,
        }
    }
}

#[derive(Debug)]
pub enum ProfileAgeCredentialTarget {
    Keep,
    Set(AgePrivateKey),
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileAgeCredentialReadRequest {
    pub profile_id: ProfileId,
    pub expected_selection: ActiveProfileReferenceState,
    pub authorization: ActiveProfileReferenceWriteAuthorization,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProfileAgeCredentialReadOutcome {
    Observed {
        current: ProfileAgeCredentialObservation,
        /// Exact active-reference state fenced by the owner for this read.
        /// A later Set/Clear must submit this state unchanged.
        selection: ActiveProfileReferenceState,
    },
    StaleSelection {
        expected: ActiveProfileReferenceState,
        observed: ActiveProfileReferenceState,
    },
}

#[derive(Debug)]
pub struct ProfileAgeCredentialWriteRequest {
    pub expected: ProfileAgeCredentialObservation,
    pub expected_selection: ActiveProfileReferenceState,
    pub authorization: ActiveProfileReferenceWriteAuthorization,
    pub target: ProfileAgeCredentialTarget,
}

/// Cross-process owner request. The credential value is intentionally kept
/// outside this serializable metadata; Android transports it as a one-shot
/// sensitive payload and reconstructs [`AgePrivateKey`] only in the owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "request", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ProfileAgeCredentialOwnerMetadata {
    Read {
        scope: CoreScope,
        profile_id: ProfileId,
    },
    Set {
        scope: CoreScope,
        expected: ProfileAgeCredentialObservation,
        expected_selection: ActiveProfileReferenceState,
    },
    Clear {
        scope: CoreScope,
        expected: ProfileAgeCredentialObservation,
        expected_selection: ActiveProfileReferenceState,
    },
}

/// In-process typed request used by frontends and the Android adapter. Unlike
/// the metadata wire enum, `Set` carries the non-serializable zeroizing key.
#[derive(Debug)]
pub enum ProfileAgeCredentialOwnerRequest {
    Read {
        scope: CoreScope,
        profile_id: ProfileId,
    },
    Set {
        scope: CoreScope,
        expected: ProfileAgeCredentialObservation,
        expected_selection: ActiveProfileReferenceState,
        private_key: AgePrivateKey,
    },
    Clear {
        scope: CoreScope,
        expected: ProfileAgeCredentialObservation,
        expected_selection: ActiveProfileReferenceState,
    },
}

/// Typed request for a platform bridge that carries private bytes separately
/// from the serializable scope and selection metadata.
#[derive(Debug)]
pub struct ProfileAgeCredentialOwnerSubmission {
    pub metadata: ProfileAgeCredentialOwnerMetadata,
    pub secret: ProfileAgeCredentialSecret,
}

impl ProfileAgeCredentialOwnerRequest {
    pub fn metadata(&self) -> ProfileAgeCredentialOwnerMetadata {
        match self {
            Self::Read { scope, profile_id } => ProfileAgeCredentialOwnerMetadata::Read {
                scope: scope.clone(),
                profile_id: profile_id.clone(),
            },
            Self::Set {
                scope,
                expected,
                expected_selection,
                ..
            } => ProfileAgeCredentialOwnerMetadata::Set {
                scope: scope.clone(),
                expected: expected.clone(),
                expected_selection: expected_selection.clone(),
            },
            Self::Clear {
                scope,
                expected,
                expected_selection,
            } => ProfileAgeCredentialOwnerMetadata::Clear {
                scope: scope.clone(),
                expected: expected.clone(),
                expected_selection: expected_selection.clone(),
            },
        }
    }
}

/// Non-secret owner reply for a read or mutation request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", content = "readback", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ProfileAgeCredentialOwnerReadback {
    Read(ProfileAgeCredentialReadOutcome),
    Mutation(ProfileAgeCredentialMutation),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProfileAgeCredentialMutation {
    Unchanged {
        current: ProfileAgeCredentialObservation,
    },
    Updated {
        previous: ProfileAgeCredentialObservation,
        current: ProfileAgeCredentialObservation,
    },
    Cleared {
        previous: ProfileAgeCredentialObservation,
        current: ProfileAgeCredentialObservation,
    },
    StaleCredential {
        expected: ProfileAgeCredentialObservation,
        observed: ProfileAgeCredentialObservation,
    },
    StaleSelection {
        expected: ActiveProfileReferenceState,
        observed: ActiveProfileReferenceState,
    },
}

fn scrub_string(value: &mut String) {
    let bytes = unsafe { value.as_bytes_mut() };
    for byte in bytes {
        unsafe { ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

fn scrub_bytes(value: &mut [u8]) {
    for byte in value {
        // SAFETY: the pointer is valid for the duration of this write, and the
        // volatile store prevents the compiler from eliding secret erasure.
        unsafe { ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::{
        AgePrivateKey, ProfileAgeCredentialObservation, ProfileAgeCredentialOwnerMetadata,
        ProfileAgeCredentialOwnerReadback, ProfileAgeCredentialOwnerRequest,
        ProfileAgeCredentialReadOutcome, ProfileAgeCredentialRevision, ProfileAgeCredentialSecret,
        ProfileAgeCredentialState, ProfileAgeDecryptedContent, ProfileAgeEncryptedContent,
    };
    use crate::active_profile_reference::{
        ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    };
    use crate::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId};
    use crate::profile_document_identity::ProfileId;

    #[test]
    fn age_private_key_debug_is_redacted_and_not_serializable() {
        let secret = AgePrivateKey::new("AGE-SECRET-KEY-example".to_owned()).unwrap();
        assert_eq!(format!("{secret:?}"), "AgePrivateKey([REDACTED])");
        assert!(!format!("{secret:?}").contains("AGE-SECRET-KEY-example"));
    }

    #[test]
    fn one_shot_owner_secret_is_redacted_and_size_bounded() {
        let secret = ProfileAgeCredentialSecret::new(b"AGE-SECRET-KEY-example".to_vec()).unwrap();
        assert_eq!(
            format!("{secret:?}"),
            "ProfileAgeCredentialSecret([REDACTED])"
        );
        assert!(!format!("{secret:?}").contains("AGE-SECRET-KEY-example"));
        assert!(matches!(
            ProfileAgeCredentialSecret::new(vec![b'a'; AgePrivateKey::MAX_BYTES + 1]),
            Err(super::AgePrivateKeyError::TooLong)
        ));
    }

    #[test]
    fn private_key_one_shot_bytes_are_validated_before_use() {
        assert_eq!(
            AgePrivateKey::from_bytes(vec![0xff]),
            Err(super::AgePrivateKeyError::InvalidEncoding)
        );
        assert_eq!(
            AgePrivateKey::from_bytes(Vec::new()),
            Err(super::AgePrivateKeyError::Empty)
        );
        assert_eq!(
            AgePrivateKey::from_bytes(vec![b'a'; AgePrivateKey::MAX_BYTES + 1]),
            Err(super::AgePrivateKeyError::TooLong)
        );
        assert!(AgePrivateKey::from_bytes(b"AGE-SECRET-KEY".to_vec()).is_ok());
    }

    #[test]
    fn credential_observation_has_a_stable_nonsecret_wire_shape() {
        let observation = ProfileAgeCredentialObservation::new(
            EngineId::mihomo(),
            ProfileId::new("primary").unwrap(),
            super::ProfileAgeCredentialRevision::new(4),
            super::ProfileAgeCredentialState::Configured,
        );
        let encoded = serde_json::to_string(&observation).unwrap();
        assert!(!encoded.contains("AGE-SECRET"));
        assert!(encoded.contains("configured"));
    }

    #[test]
    fn owner_request_metadata_is_scope_fenced_and_never_serializes_the_private_key() {
        let engine = EngineId::mihomo();
        let profile_id = ProfileId::new("primary").unwrap();
        let selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            engine.clone(),
            profile_id.clone(),
            ActiveProfileReferenceRevision::new(8),
        ));
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("android-vpn").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(12),
            session_token: None,
        };
        let expected = ProfileAgeCredentialObservation::new(
            engine,
            profile_id,
            ProfileAgeCredentialRevision::new(4),
            ProfileAgeCredentialState::NotConfigured,
        );
        let request = ProfileAgeCredentialOwnerRequest::Set {
            scope: scope.clone(),
            expected: expected.clone(),
            expected_selection: selection.clone(),
            private_key: AgePrivateKey::new("AGE-SECRET-KEY-never-wire".to_owned()).unwrap(),
        };

        let encoded = serde_json::to_string(&request.metadata()).unwrap();

        assert!(!encoded.contains("AGE-SECRET-KEY-never-wire"));
        assert!(matches!(
            serde_json::from_str::<ProfileAgeCredentialOwnerMetadata>(&encoded).unwrap(),
            ProfileAgeCredentialOwnerMetadata::Set {
                scope: observed_scope,
                expected: observed,
                expected_selection,
            } if observed_scope == scope && observed == expected && expected_selection == selection
        ));
        assert!(!format!("{request:?}").contains("AGE-SECRET-KEY-never-wire"));
    }

    #[test]
    fn owner_read_metadata_omits_selection_and_readback_returns_its_fenced_state() {
        let engine = EngineId::mihomo();
        let profile_id = ProfileId::new("primary").unwrap();
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("android-vpn").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(12),
            session_token: None,
        };
        let selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            engine.clone(),
            profile_id.clone(),
            ActiveProfileReferenceRevision::new(8),
        ));
        let metadata = ProfileAgeCredentialOwnerMetadata::Read {
            scope: scope.clone(),
            profile_id: profile_id.clone(),
        };
        let encoded_metadata = serde_json::to_string(&metadata).unwrap();
        assert!(!encoded_metadata.contains("expected_selection"));
        assert_eq!(
            serde_json::from_str::<ProfileAgeCredentialOwnerMetadata>(&encoded_metadata).unwrap(),
            metadata,
        );

        let readback =
            ProfileAgeCredentialOwnerReadback::Read(ProfileAgeCredentialReadOutcome::Observed {
                current: ProfileAgeCredentialObservation::new(
                    engine,
                    profile_id,
                    ProfileAgeCredentialRevision::new(4),
                    ProfileAgeCredentialState::Configured,
                ),
                selection,
            });
        let encoded_readback = serde_json::to_string(&readback).unwrap();
        assert_eq!(
            serde_json::from_str::<ProfileAgeCredentialOwnerReadback>(&encoded_readback).unwrap(),
            readback,
        );
    }

    #[test]
    fn encrypted_and_decrypted_profile_content_debug_is_redacted() {
        let encrypted_document = "-----BEGIN AGE ENCRYPTED FILE-----\nopaque-ciphertext\n-----END AGE ENCRYPTED FILE-----";
        assert!(ProfileAgeEncryptedContent::is_armored_age_document(
            encrypted_document
        ));
        assert!(!ProfileAgeEncryptedContent::is_armored_age_document(
            "proxies: []\n"
        ));
        let armored = ProfileAgeEncryptedContent::new(encrypted_document.to_owned()).unwrap();
        let decrypted = ProfileAgeDecryptedContent::new("proxies: [private]".to_owned());

        assert!(!format!("{armored:?}").contains("opaque-ciphertext"));
        assert!(!format!("{decrypted:?}").contains("private"));
    }
}
