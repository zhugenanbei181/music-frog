//! Selection-agnostic profile-reset intent parsing and mutation fences.
//!
//! The reset intent is generic over the product selection payload; the
//! transaction that rewrites a product's default profile document stays in the
//! product crate. These predicates and the root-to-profile fence are shared by
//! every engine store.

use crate::document_store::read_optional_bytes;
use crate::file_lock::{ProfileWriteGuard, lock_profile_writes_for_dir};
use crate::instance_lock::ThreadedInstanceLockGuard;
use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::file::OwnerSwitchFile;
use crate::workspace::sha256_hex;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::from_slice;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;

pub const ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION: u32 = 1;
pub const PROFILE_RESET_PENDING_MUTATION_MESSAGE: &str =
    "a profile reset is awaiting durable cleanup; retry that reset before other profile mutations";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveProfileResetIntentRecord<S> {
    pub schema_version: u32,
    pub expected_reference: S,
    pub target_reference: S,
    pub default_document_sha256: String,
    pub default_document: Vec<u8>,
}

impl<S> ActiveProfileResetIntentRecord<S> {
    pub fn new(expected_reference: S, target_reference: S, default_content: &[u8]) -> Self {
        Self {
            schema_version: ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION,
            expected_reference,
            target_reference,
            default_document_sha256: sha256_hex(default_content),
            default_document: default_content.to_vec(),
        }
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.schema_version != ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                Capability::Profiles,
                format!(
                    "active profile reset intent schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        if self.default_document_sha256.len() != 64
            || !self
                .default_document_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(PortError::Io(
                "active profile reset intent has an invalid default digest".into(),
            ));
        }
        if self.default_document_sha256 != sha256_hex(&self.default_document) {
            return Err(PortError::Io(
                "active profile reset intent default bytes do not match their digest".into(),
            ));
        }
        Ok(())
    }

    pub fn matches_transition(&self, expected: &S, target: &S) -> bool
    where
        S: PartialEq,
    {
        &self.expected_reference == expected && &self.target_reference == target
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyActiveProfileResetIntentRecord<S> {
    pub expected_reference: S,
    pub target_reference: S,
    pub default_document_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedActiveProfileResetIntentRecord<S> {
    LegacyV1(LegacyActiveProfileResetIntentRecord<S>),
    CurrentV2(ActiveProfileResetIntentRecord<S>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveProfileResetIntentEnvelope<S> {
    schema_version: u32,
    expected_reference: S,
    target_reference: S,
    default_document_sha256: String,
    default_document: Option<Vec<u8>>,
}

impl<S> ParsedActiveProfileResetIntentRecord<S> {
    pub fn expected_reference(&self) -> &S {
        match self {
            Self::LegacyV1(record) => &record.expected_reference,
            Self::CurrentV2(record) => &record.expected_reference,
        }
    }

    pub fn target_reference(&self) -> &S {
        match self {
            Self::LegacyV1(record) => &record.target_reference,
            Self::CurrentV2(record) => &record.target_reference,
        }
    }

    pub fn current_record(&self) -> Option<&ActiveProfileResetIntentRecord<S>> {
        match self {
            Self::LegacyV1(_) => None,
            Self::CurrentV2(record) => Some(record),
        }
    }
}

pub enum ActiveProfileResetMutationAccess {
    OrdinaryMutation,
    RetryEngine(EngineId),
}

/// Holds the product-wide reset fence before the engine-scoped profile lock.
pub struct ResetFencedProfileMutationGuard {
    _root: ThreadedInstanceLockGuard,
    _profile: ProfileWriteGuard,
}

pub fn parse_reset_intent_record<S>(
    bytes: &[u8],
) -> Result<ParsedActiveProfileResetIntentRecord<S>, PortError>
where
    S: DeserializeOwned,
{
    let record: ActiveProfileResetIntentEnvelope<S> = from_slice(bytes)
        .map_err(|error| PortError::Io(format!("invalid profile reset intent record: {error}")))?;
    if record.default_document_sha256.len() != 64
        || !record
            .default_document_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(PortError::Io(
            "active profile reset intent has an invalid default digest".into(),
        ));
    }
    match (record.schema_version, record.default_document) {
        (LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION, None) => Ok(
            ParsedActiveProfileResetIntentRecord::LegacyV1(LegacyActiveProfileResetIntentRecord {
                expected_reference: record.expected_reference,
                target_reference: record.target_reference,
                default_document_sha256: record.default_document_sha256,
            }),
        ),
        (ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION, Some(default_document)) => {
            let current = ActiveProfileResetIntentRecord {
                schema_version: record.schema_version,
                expected_reference: record.expected_reference,
                target_reference: record.target_reference,
                default_document_sha256: record.default_document_sha256,
                default_document,
            };
            current.validate()?;
            Ok(ParsedActiveProfileResetIntentRecord::CurrentV2(current))
        }
        (schema_version, _) => Err(PortError::unsupported(
            Capability::Profiles,
            format!("active profile reset intent schema {schema_version} is unsupported"),
        )),
    }
}

pub async fn ensure_reset_not_pending_for_dir<C: OwnerSwitchCodec, S>(
    config_dir: &Path,
) -> Result<(), PortError>
where
    S: DeserializeOwned + PartialEq,
{
    if reset_pending_for_dir::<C, S>(config_dir).await? {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            PROFILE_RESET_PENDING_MUTATION_MESSAGE,
            true,
        )));
    }
    Ok(())
}

pub async fn reset_pending_for_dir<C: OwnerSwitchCodec, S>(
    config_dir: &Path,
) -> Result<bool, PortError>
where
    S: DeserializeOwned + PartialEq,
{
    let layout = C::layout();
    let intent_path = config_dir.join(layout.reset_intent_file_name.as_str());
    let Some(intent_bytes) = read_optional_bytes(&intent_path).await? else {
        return Ok(false);
    };
    let intent = parse_reset_intent_record::<S>(&intent_bytes)?;
    let complete_path = config_dir.join(layout.reset_complete_file_name.as_str());
    let complete = read_optional_bytes(&complete_path).await?;
    if complete
        .as_deref()
        .map(parse_reset_intent_record::<S>)
        .transpose()?
        .as_ref()
        == Some(&intent)
    {
        return Ok(false);
    }
    Ok(true)
}

pub async fn ensure_no_pending_reset_in_engine_namespaces<
    C: OwnerSwitchCodec,
    S: DeserializeOwned + PartialEq,
>(
    config_root: &Path,
    access: ActiveProfileResetMutationAccess,
) -> Result<(), PortError> {
    let layout = C::layout();
    let engines_root = config_root.join(layout.engine_dir_name.as_str());
    let mut entries = match fs::read_dir(&engines_root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(PortError::Io(error.to_string())),
    };
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| PortError::Io(error.to_string()))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        if file_type.is_symlink() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "engine namespace scan encountered a symlink and cannot prove reset state",
                true,
            )));
        }
        if !file_type.is_dir() {
            continue;
        }
        let engine_name = entry.file_name();
        let skip_local_retry_engine = match &access {
            ActiveProfileResetMutationAccess::OrdinaryMutation => false,
            ActiveProfileResetMutationAccess::RetryEngine(engine_id) => {
                engine_name == engine_id.as_str()
            }
        };
        if skip_local_retry_engine {
            continue;
        }
        ensure_reset_not_pending_for_dir::<C, S>(&entry.path()).await?;
    }
    Ok(())
}

/// Acquire the shared root reference lock, inspect every engine reset intent,
/// then acquire this engine's profile-write boundary in root-to-profile order.
pub async fn acquire_reset_fenced_profile_mutation_for_dir<
    C: OwnerSwitchCodec,
    S: DeserializeOwned + PartialEq,
>(
    config_dir: &Path,
) -> Result<ResetFencedProfileMutationGuard, PortError> {
    let layout = C::layout();
    let engines_root = config_dir
        .parent()
        .ok_or_else(|| PortError::Io("engine config directory has no parent".into()))?;
    if engines_root.file_name().and_then(|name| name.to_str())
        != Some(layout.engine_dir_name.as_str())
    {
        return Err(PortError::Io(
            "engine config directory is outside the shared engines namespace".into(),
        ));
    }
    let config_root = engines_root
        .parent()
        .ok_or_else(|| PortError::Io("engine config namespace has no shared config root".into()))?;
    let (config_root, root) = OwnerSwitchFile::<C>::new(config_root.to_path_buf())
        .acquire_root_lock()
        .await?;
    ensure_no_pending_reset_in_engine_namespaces::<C, S>(
        &config_root,
        ActiveProfileResetMutationAccess::OrdinaryMutation,
    )
    .await?;
    let profile = lock_profile_writes_for_dir(layout, config_dir).await?;
    Ok(ResetFencedProfileMutationGuard {
        _root: root,
        _profile: profile,
    })
}
