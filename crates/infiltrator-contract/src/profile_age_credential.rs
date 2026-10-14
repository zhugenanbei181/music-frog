//! Secure, engine-scoped Age credential editing with owner-fenced readback.

use crate::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use crate::core_identity::EngineId;
use crate::profile_document_identity::ProfileId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error;
use std::fmt::{self, Debug, Display, Formatter};
use std::ptr;
use std::sync::atomic::{Ordering, compiler_fence};

const MAX_AGE_PRIVATE_KEY_BYTES: usize = 4096;

/// Age private key input. It is intentionally neither serializable nor
/// displayable; `Debug` is always redacted and its owned bytes are scrubbed on
/// drop. Transport adapters must map it explicitly to a secure request.
pub struct AgePrivateKey(String);

impl AgePrivateKey {
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
}

impl Display for AgePrivateKeyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "Age private key must not be empty",
            Self::TooLong => "Age private key exceeds the supported size",
            Self::InvalidWhitespace => "Age private key must be a single trimmed line",
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

#[cfg(test)]
mod tests {
    use super::{AgePrivateKey, ProfileAgeCredentialObservation};
    use crate::core_identity::EngineId;
    use crate::profile_document_identity::ProfileId;

    #[test]
    fn age_private_key_debug_is_redacted_and_not_serializable() {
        let secret = AgePrivateKey::new("AGE-SECRET-KEY-example".to_owned()).unwrap();
        assert_eq!(format!("{secret:?}"), "AgePrivateKey([REDACTED])");
        assert!(!format!("{secret:?}").contains("AGE-SECRET-KEY-example"));
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
}
