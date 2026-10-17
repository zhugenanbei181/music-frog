//! Mihomo option-sidecar storage. Schema transforms live in
//! [`crate::profile_options_codec`].
use crate::manager::lock_reset_fenced_profile_mutation_for_dir;
use crate::manager::paths::scope_config_dir;
use crate::profile_options_codec;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profile_options::{ProfileOptions, options_path};
use infiltrator_domain::profiles::validate_profile_name;
use infiltrator_profile_store::document_store::{
    read_optional_bytes, remove_optional, write_document_bytes,
};
use infiltrator_profile_store::file_lock::lock_profile_writes_for_dir;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use std::path::{Path, PathBuf};

fn scoped_dir(config_root_or_scoped: &Path) -> Result<PathBuf, PortError> {
    scope_config_dir(config_root_or_scoped, &crate::engine_id())
        .map_err(|error| PortError::Io(error.to_string()))
}

pub async fn load_options(config_dir: &Path, profile: &str) -> Result<ProfileOptions, PortError> {
    let bytes = read_options_bytes(config_dir, profile).await?;
    profile_options_codec::decode(bytes.as_deref())
}

/// Read exact opaque sidecar bytes for the Mihomo options codec.
pub(crate) async fn read_options_bytes(
    config_dir: &Path,
    profile: &str,
) -> Result<Option<Vec<u8>>, PortError> {
    let profile = validated_profile_id(profile)?;
    let config_dir = scoped_dir(config_dir)?;
    let _guard = lock_profile_writes_for_dir(&config_dir).await?;
    read_optional_bytes(&options_path(&config_dir, profile.as_str())).await
}

pub async fn save_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> Result<(), PortError> {
    let bytes = profile_options_codec::encode(options)?;
    write_options_bytes(config_dir, profile, bytes.as_deref()).await
}

/// Publish or remove exact opaque sidecar bytes for the Mihomo options codec.
pub(crate) async fn write_options_bytes(
    config_dir: &Path,
    profile: &str,
    bytes: Option<&[u8]>,
) -> Result<(), PortError> {
    let profile = validated_profile_id(profile)?;
    let config_dir = scoped_dir(config_dir)?;
    let _guard = lock_reset_fenced_profile_mutation_for_dir(&config_dir).await?;
    let path = options_path(&config_dir, profile.as_str());
    match bytes {
        None => remove_optional(&path).await,
        Some(bytes) => write_document_bytes(&path, bytes).await,
    }
}

pub async fn delete_options(config_dir: &Path, profile: &str) -> Result<(), PortError> {
    let profile = validated_profile_id(profile)?;
    let config_dir = scoped_dir(config_dir)?;
    let _guard = lock_reset_fenced_profile_mutation_for_dir(&config_dir).await?;
    remove_optional(&options_path(&config_dir, profile.as_str())).await
}

fn validated_profile_id(profile: &str) -> Result<ProfileId, PortError> {
    validate_profile_name(profile).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::InvalidInput,
            error.to_string(),
            false,
        ))
    })?;
    ProfileId::new(profile.to_owned()).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::InvalidInput,
            error.to_string(),
            false,
        ))
    })
}
