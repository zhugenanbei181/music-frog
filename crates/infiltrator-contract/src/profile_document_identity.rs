//! Engine- and source-bound identity for opaque profile documents.

use lurker_core_contract::identity::EngineId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str;

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

/// Stable application identity for a profile, not a filesystem path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileDocumentIdentityError> {
        let value = value.into();
        if !valid_profile_identifier(&value) {
            return Err(ProfileDocumentIdentityError::InvalidProfileId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfileId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Opaque stable key for the source record; it must not contain a URL or path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileSourceId(String);

impl ProfileSourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileDocumentIdentityError> {
        let value = value.into();
        if !valid_source_identifier(&value) {
            return Err(ProfileDocumentIdentityError::InvalidSourceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfileSourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Origin category for a stored profile document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDocumentSourceKind {
    Local,
    Subscription,
    Imported,
    Synchronized,
}

/// Stable origin identity, distinct from the profile ID and its CAS revision.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileDocumentSourceIdentity {
    pub kind: ProfileDocumentSourceKind,
    pub source_id: ProfileSourceId,
}

impl ProfileDocumentSourceIdentity {
    pub fn new(kind: ProfileDocumentSourceKind, source_id: ProfileSourceId) -> Self {
        Self { kind, source_id }
    }
}

/// Opaque compare-and-swap token supplied by the persistence adapter.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileDocumentRevision(String);

impl ProfileDocumentRevision {
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileDocumentIdentityError> {
        let value = value.into();
        if !valid_opaque_identifier(&value, 256) {
            return Err(ProfileDocumentIdentityError::InvalidRevision);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfileDocumentRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

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

/// CAS identity for one profile document within one engine and source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocumentIdentity {
    pub profile_id: ProfileId,
    pub engine_id: EngineId,
    pub source: ProfileDocumentSourceIdentity,
    pub revision: ProfileDocumentRevision,
    pub format_id: ProfileFormatId,
    pub schema_version: ProfileSchemaVersion,
}

impl ProfileDocumentIdentity {
    pub fn new(
        profile_id: ProfileId,
        engine_id: EngineId,
        source: ProfileDocumentSourceIdentity,
        revision: ProfileDocumentRevision,
        format_id: ProfileFormatId,
        schema_version: ProfileSchemaVersion,
    ) -> Self {
        Self {
            profile_id,
            engine_id,
            source,
            revision,
            format_id,
            schema_version,
        }
    }
}

/// Revision of the options sidecar paired with a profile document.
///
/// `Absent` and `Present` with an empty-byte revision are distinct states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSidecarRevision {
    Absent,
    Present(ProfileDocumentRevision),
}

/// Compare-and-swap identity for a profile document and its options sidecar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileWorkspaceIdentity {
    pub document: ProfileDocumentIdentity,
    pub sidecar_revision: ProfileSidecarRevision,
}

impl ProfileWorkspaceIdentity {
    pub fn new(
        document: ProfileDocumentIdentity,
        sidecar_revision: ProfileSidecarRevision,
    ) -> Self {
        Self {
            document,
            sidecar_revision,
        }
    }
}

/// Native configuration bytes with an explicit engine, source, format and schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocument {
    pub identity: ProfileDocumentIdentity,
    pub raw_bytes: Vec<u8>,
}

impl ProfileDocument {
    pub fn new(identity: ProfileDocumentIdentity, raw_bytes: Vec<u8>) -> Self {
        Self {
            identity,
            raw_bytes,
        }
    }

    pub fn utf8_text(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.raw_bytes)
    }
}

fn valid_opaque_identifier(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn valid_profile_identifier(value: &str) -> bool {
    valid_opaque_identifier(value, 256)
        && value == value.trim()
        && value != "."
        && value != ".."
        && !value.chars().any(|character| {
            matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        })
}

fn valid_source_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{
        ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
        ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
        ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
    };
    use lurker_core_contract::identity::EngineId;

    fn identity(
        engine_id: &str,
        profile_id: &str,
        source_kind: ProfileDocumentSourceKind,
        source_id: &str,
        revision: &str,
        format_id: &str,
        schema_version: u32,
    ) -> ProfileDocumentIdentity {
        ProfileDocumentIdentity::new(
            ProfileId::new(profile_id).expect("valid profile ID"),
            EngineId::new(engine_id).expect("valid engine ID"),
            ProfileDocumentSourceIdentity::new(
                source_kind,
                ProfileSourceId::new(source_id).expect("valid source ID"),
            ),
            ProfileDocumentRevision::new(revision).expect("valid document revision"),
            ProfileFormatId::new(format_id).expect("valid format ID"),
            ProfileSchemaVersion::new(schema_version).expect("positive schema version"),
        )
    }

    #[test]
    fn identity_scopes_document_revision_by_engine_profile_and_source() {
        let local = identity(
            "mihomo",
            "main",
            ProfileDocumentSourceKind::Local,
            "local-main",
            "rev-1",
            "mihomo-yaml",
            1,
        );
        let other_engine = identity(
            "sing-box",
            "main",
            ProfileDocumentSourceKind::Local,
            "local-main",
            "rev-1",
            "mihomo-yaml",
            1,
        );
        let other_profile = identity(
            "mihomo",
            "backup",
            ProfileDocumentSourceKind::Local,
            "local-main",
            "rev-1",
            "mihomo-yaml",
            1,
        );
        let subscription = identity(
            "mihomo",
            "main",
            ProfileDocumentSourceKind::Subscription,
            "subscription-7",
            "rev-1",
            "mihomo-yaml",
            1,
        );

        assert_ne!(local, other_engine);
        assert_ne!(local, other_profile);
        assert_ne!(local, subscription);
        assert_ne!(
            local,
            identity(
                "mihomo",
                "main",
                ProfileDocumentSourceKind::Local,
                "local-main",
                "rev-1",
                "mihomo-yaml",
                2,
            )
        );
        assert_ne!(
            local,
            identity(
                "mihomo",
                "main",
                ProfileDocumentSourceKind::Local,
                "local-main",
                "rev-1",
                "mihomo-yaml-v2",
                1,
            )
        );
    }

    #[test]
    fn workspace_identity_distinguishes_absent_and_present_empty_sidecar() {
        let document = identity(
            "mihomo",
            "main",
            ProfileDocumentSourceKind::Local,
            "local-main",
            "rev-1",
            "mihomo-yaml",
            1,
        );
        let absent =
            ProfileWorkspaceIdentity::new(document.clone(), ProfileSidecarRevision::Absent);
        let present_empty = ProfileWorkspaceIdentity::new(
            document,
            ProfileSidecarRevision::Present(
                ProfileDocumentRevision::new("empty-sidecar-revision").expect("valid revision"),
            ),
        );

        assert_ne!(absent, present_empty);
    }

    #[test]
    fn native_document_preserves_exact_non_utf8_bytes_through_serde() {
        let bytes = vec![0x00, 0xFF, 0x80, b'{', b'"', b'k', b'"', b':', b'1', b'}'];
        let document = ProfileDocument::new(
            identity(
                "sing-box",
                "imported",
                ProfileDocumentSourceKind::Imported,
                "import-42",
                "sha256:abc123",
                "sing-box-json",
                2,
            ),
            bytes.clone(),
        );

        let encoded = serde_json::to_vec(&document).expect("serialize document");
        let decoded: ProfileDocument =
            serde_json::from_slice(&encoded).expect("deserialize document");

        assert_eq!(decoded, document);
        assert_eq!(decoded.raw_bytes, bytes);
        assert_eq!(decoded.identity.engine_id.as_str(), "sing-box");
        assert_eq!(
            decoded.identity.source.kind,
            ProfileDocumentSourceKind::Imported
        );
    }

    #[test]
    fn document_identity_and_format_newtypes_reject_invalid_values() {
        assert!(ProfileId::new("  ").is_err());
        assert!(ProfileId::new("profile\nname").is_err());
        assert!(ProfileSourceId::new("").is_err());
        assert!(ProfileSourceId::new("https://example.test/profile").is_err());
        assert!(ProfileDocumentRevision::new("\0").is_err());
        assert!(ProfileFormatId::new("Mihomo-YAML").is_err());
        assert!(ProfileSchemaVersion::new(0).is_err());
    }

    #[test]
    fn serde_cannot_bypass_identifier_and_schema_validation() {
        let invalid_profile = serde_json::from_value::<ProfileId>(serde_json::json!(""));
        let invalid_format = serde_json::from_value::<ProfileFormatId>(serde_json::json!("YAML"));
        let invalid_schema = serde_json::from_value::<ProfileSchemaVersion>(serde_json::json!(0));

        assert!(invalid_profile.is_err());
        assert!(invalid_format.is_err());
        assert!(invalid_schema.is_err());
    }

    #[test]
    fn profile_identity_cannot_be_used_as_a_path_fragment() {
        for value in [
            "../outside",
            "a/b",
            "a\\b",
            "C:\\profiles\\main",
            "main:backup",
            ".",
            "..",
            " main ",
        ] {
            assert!(
                ProfileId::new(value).is_err(),
                "accepted unsafe ID {value:?}"
            );
        }
        let error = ProfileId::new("../outside").expect_err("path fragments are rejected");
        assert!(error.to_string().contains("path characters"));
        assert_eq!(ProfileId::new("main").unwrap().as_str(), "main");
    }
}
