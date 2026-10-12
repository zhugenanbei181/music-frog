//! Engine-scoped profile namespace schema and durable settings I/O.

use crate::document_store::write_document_bytes_durable;
use crate::store::EngineProfileStore;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::future::Future;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::fs;

pub const PROFILE_NAMESPACE_SCHEMA_KEY: &str = "profile_namespace_schema";
const ENGINE_SCOPED_SCHEMA_V1: &str = "engine_scoped_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileNamespaceSchema {
    LegacyShared,
    EngineScopedV1,
}

impl ProfileNamespaceSchema {
    pub fn parse(value: Option<&toml::Value>) -> Result<Self, PortError> {
        match value {
            None => Ok(Self::LegacyShared),
            Some(toml::Value::String(value)) if value == ENGINE_SCOPED_SCHEMA_V1 => {
                Ok(Self::EngineScopedV1)
            }
            Some(toml::Value::String(other)) => Err(namespace_configuration_error(format!(
                "unsupported profile namespace schema `{other}`"
            ))),
            Some(_) => Err(namespace_configuration_error(
                "profile namespace schema marker must be a string",
            )),
        }
    }

    pub fn write_engine_scoped_v1(state: &mut toml::Value) -> Result<(), PortError> {
        let table = state
            .as_table_mut()
            .ok_or_else(|| namespace_configuration_error("profile state must be a TOML table"))?;
        table.insert(
            PROFILE_NAMESPACE_SCHEMA_KEY.into(),
            toml::Value::String(ENGINE_SCOPED_SCHEMA_V1.into()),
        );
        Ok(())
    }
}

/// Initialize only the generic engine-scoped schema. The caller must hold
/// the product reset fence and the profile write lock.
pub async fn initialize_engine_scoped_namespace_unlocked(
    config_dir: &Path,
    settings_file: &Path,
) -> Result<(), PortError> {
    fs::create_dir_all(config_dir).await.map_err(map_io_error)?;
    let mut state = read_profile_state(settings_file).await?;
    if matches!(
        ProfileNamespaceSchema::parse(state.get(PROFILE_NAMESPACE_SCHEMA_KEY))?,
        ProfileNamespaceSchema::EngineScopedV1
    ) {
        return Ok(());
    }
    ProfileNamespaceSchema::write_engine_scoped_v1(&mut state)?;
    write_profile_state_atomically(settings_file, &state).await
}

pub async fn read_profile_state(path: &Path) -> Result<toml::Value, PortError> {
    recover_profile_state_backup(path).await?;
    let text = match fs::read_to_string(path).await {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(toml::Value::Table(toml::map::Map::new()));
        }
        Err(error) => return Err(map_io_error(error)),
    };
    toml::from_str(&text).map_err(|error| namespace_configuration_error(error.to_string()))
}

pub async fn write_profile_state_atomically(
    path: &Path,
    state: &toml::Value,
) -> Result<(), PortError> {
    recover_profile_state_backup(path).await?;
    let content = toml::to_string_pretty(state)
        .map_err(|error| namespace_configuration_error(error.to_string()))?;
    write_document_bytes_durable(path, content.as_bytes()).await
}

pub async fn recover_profile_state_backup(path: &Path) -> Result<(), PortError> {
    let backup = profile_state_backup_path(path);
    match (
        fs::try_exists(path).await.map_err(map_io_error)?,
        fs::try_exists(&backup).await.map_err(map_io_error)?,
    ) {
        (false, true) => fs::rename(backup, path).await.map_err(map_io_error),
        (true, true) => fs::remove_file(backup).await.map_err(map_io_error),
        (_, false) => Ok(()),
    }
}

pub fn profile_state_backup_path(path: &Path) -> PathBuf {
    path.with_extension("toml.namespace-backup")
}

impl<S: SecureStore> EngineProfileStore<S> {
    /// Initialize the generic engine namespace once, under a caller-provided
    /// product mutation fence.
    pub async fn initialize_engine_scoped_namespace_with<G, F, Fut>(
        &self,
        acquire_fence: F,
    ) -> Result<(), PortError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<G, PortError>>,
    {
        self.initialize_namespace_with(|| async {
            let _fence = acquire_fence().await?;
            initialize_engine_scoped_namespace_unlocked(self.config_dir(), self.settings_file())
                .await
        })
        .await
    }

    pub async fn reset_engine_scoped_namespace_marker_unlocked(&self) -> Result<(), PortError> {
        let mut state = toml::Value::Table(toml::map::Map::new());
        ProfileNamespaceSchema::write_engine_scoped_v1(&mut state)?;
        write_profile_state_atomically(self.settings_file(), &state).await
    }
}

fn namespace_configuration_error(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, message, false))
}

fn map_io_error(error: std::io::Error) -> PortError {
    if error.kind() == ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}
