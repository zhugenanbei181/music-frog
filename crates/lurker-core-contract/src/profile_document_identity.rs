//! Engine-neutral profile-document identity fragments.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Validation failure for any profile-document identity newtype.
///
/// Product-owned identifiers (profile ID, source ID, revision, workspace
/// identity) reuse this neutral error vocabulary so adapters can report
/// identity failures without depending on a product crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileDocumentIdentityError {
    InvalidProfileId,
    InvalidSourceId,
    InvalidRevision,
    InvalidFormatId,
    InvalidSchemaVersion,
}

impl Display for ProfileDocumentIdentityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfileId => formatter
                .write_str(
                    "profile ID must be non-empty, bounded, contain no controls, and contain no path characters",
                ),
            Self::InvalidSourceId => formatter
                .write_str("profile source ID must be non-empty, bounded, and contain no controls"),
            Self::InvalidRevision => formatter.write_str(
                "profile document revision must be non-empty, bounded, and contain no controls",
            ),
            Self::InvalidFormatId => formatter
                .write_str("profile format ID must use lowercase ASCII identifier characters"),
            Self::InvalidSchemaVersion => {
                formatter.write_str("profile schema version must be greater than zero")
            }
        }
    }
}

impl Error for ProfileDocumentIdentityError {}

/// Adapter-defined native document format, such as `mihomo-yaml` or `sing-box-json`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileFormatId(String);

impl ProfileFormatId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileDocumentIdentityError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.".contains(&byte)
            })
        {
            return Err(ProfileDocumentIdentityError::InvalidFormatId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfileFormatId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Positive schema revision within one adapter-defined format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileSchemaVersion(u32);

impl ProfileSchemaVersion {
    pub const fn new(value: u32) -> Result<Self, ProfileDocumentIdentityError> {
        if value == 0 {
            return Err(ProfileDocumentIdentityError::InvalidSchemaVersion);
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ProfileSchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileDocumentIdentityError, ProfileFormatId, ProfileSchemaVersion};

    #[test]
    fn format_and_schema_newtypes_reject_invalid_values() {
        assert!(ProfileFormatId::new("Mihomo-YAML").is_err());
        assert!(ProfileFormatId::new("").is_err());
        assert!(ProfileSchemaVersion::new(0).is_err());
        assert_eq!(
            ProfileFormatId::new("mihomo-yaml")
                .expect("valid format ID")
                .as_str(),
            "mihomo-yaml"
        );
        assert_eq!(
            ProfileSchemaVersion::new(2)
                .expect("positive schema version")
                .value(),
            2
        );
    }

    #[test]
    fn serde_cannot_bypass_format_and_schema_validation() {
        let invalid_format = serde_json::from_value::<ProfileFormatId>(serde_json::json!("YAML"));
        let invalid_schema = serde_json::from_value::<ProfileSchemaVersion>(serde_json::json!(0));

        assert!(invalid_format.is_err());
        assert!(invalid_schema.is_err());
    }

    #[test]
    fn identity_error_messages_preserve_adapter_diagnostics() {
        assert!(
            ProfileDocumentIdentityError::InvalidFormatId
                .to_string()
                .contains("format ID")
        );
        assert!(
            ProfileDocumentIdentityError::InvalidSchemaVersion
                .to_string()
                .contains("schema version")
        );
    }
}
