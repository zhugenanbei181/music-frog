//! The single option-sidecar I/O owner used by ports and legacy host adapters.
use crate::manager::paths::validate_profile_name;
use crate::profile_write_boundary::shared_boundary;
use crate::sidecar_store::{read_optional, remove_optional, write_document};
use infiltrator_domain::profile_options::{ProfileOptions, options_path};
use infiltrator_ports::error::PortError;
use std::fmt::Display;
use std::path::Path;

pub async fn load_options(config_dir: &Path, profile: &str) -> Result<ProfileOptions, PortError> {
    validate_profile_name(profile).map_err(storage_error)?;
    let boundary = shared_boundary(config_dir).map_err(storage_error)?;
    let _guard = boundary.lock().await;
    let Some(text) = read_optional(&options_path(config_dir, profile)).await? else {
        return Ok(Default::default());
    };
    serde_yaml_ng::from_str(&text).map_err(storage_error)
}

pub async fn save_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> Result<(), PortError> {
    validate_profile_name(profile).map_err(storage_error)?;
    let boundary = shared_boundary(config_dir).map_err(storage_error)?;
    let _guard = boundary.lock().await;
    let path = options_path(config_dir, profile);
    if options.is_empty() {
        return remove_optional(&path).await;
    }
    let text = serde_yaml_ng::to_string(options).map_err(storage_error)?;
    write_document(&path, &text).await
}

pub async fn delete_options(config_dir: &Path, profile: &str) -> Result<(), PortError> {
    validate_profile_name(profile).map_err(storage_error)?;
    let boundary = shared_boundary(config_dir).map_err(storage_error)?;
    let _guard = boundary.lock().await;
    remove_optional(&options_path(config_dir, profile)).await
}

fn storage_error(error: impl Display) -> PortError {
    PortError::Io(error.to_string())
}
