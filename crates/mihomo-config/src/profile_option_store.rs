//! Mihomo option-sidecar storage. Schema transforms live in
//! [`crate::profile_options_codec`].
use crate::manager::paths::{scope_config_dir, validate_profile_name};
use crate::profile_write_boundary::shared_boundary;
use crate::sidecar_store::{read_optional_bytes, remove_optional, write_document_bytes};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::profile_options::{ProfileOptions, options_path};
use infiltrator_ports::error::PortError;
use std::fmt::Display;
use std::path::{Path, PathBuf};

fn scoped_dir(config_root_or_scoped: &Path) -> Result<PathBuf, PortError> {
    scope_config_dir(config_root_or_scoped, &crate::engine_id())
        .map_err(|error| PortError::Io(error.to_string()))
}

pub async fn load_options(config_dir: &Path, profile: &str) -> Result<ProfileOptions, PortError> {
    let bytes = read_options_bytes(config_dir, profile).await?;
    crate::profile_options_codec::decode(bytes.as_deref())
}

/// Read exact opaque sidecar bytes for the Mihomo options codec.
pub(crate) async fn read_options_bytes(
    config_dir: &Path,
    profile: &str,
) -> Result<Option<Vec<u8>>, PortError> {
    validate_profile_name(profile).map_err(storage_error)?;
    let config_dir = scoped_dir(config_dir)?;
    let boundary = shared_boundary(&config_dir).map_err(storage_error)?;
    let _guard = boundary.lock().await;
    read_optional_bytes(&options_path(&config_dir, profile)).await
}

pub async fn save_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> Result<(), PortError> {
    let bytes = crate::profile_options_codec::encode(options)?;
    write_options_bytes(config_dir, profile, bytes.as_deref()).await
}

/// Publish or remove exact opaque sidecar bytes for the Mihomo options codec.
pub(crate) async fn write_options_bytes(
    config_dir: &Path,
    profile: &str,
    bytes: Option<&[u8]>,
) -> Result<(), PortError> {
    validate_profile_name(profile).map_err(storage_error)?;
    let config_dir = scoped_dir(config_dir)?;
    let boundary = shared_boundary(&config_dir).map_err(storage_error)?;
    let _guard = boundary.lock().await;
    let path = options_path(&config_dir, profile);
    match bytes {
        None => remove_optional(&path).await,
        Some(bytes) => write_document_bytes(&path, bytes).await,
    }
}

pub async fn delete_options(config_dir: &Path, profile: &str) -> Result<(), PortError> {
    validate_profile_name(profile).map_err(storage_error)?;
    let config_dir = scoped_dir(config_dir)?;
    let boundary = shared_boundary(&config_dir).map_err(storage_error)?;
    let _guard = boundary.lock().await;
    remove_optional(&options_path(&config_dir, profile)).await
}

fn storage_error(error: impl Display) -> PortError {
    PortError::Io(error.to_string())
}
