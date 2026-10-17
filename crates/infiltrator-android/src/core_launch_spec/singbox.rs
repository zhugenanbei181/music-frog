use crate::runtime::CoreLaunchSpecSource;
use async_trait::async_trait;
use infiltrator_contract::android_control::core_launch::{
    AndroidCoreBinaryTarget, CoreLaunchArguments, CoreLaunchBinary,
    CoreLaunchConfigurationIdentity, CoreLaunchPath, CoreLaunchSpec,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileDocument;
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_ports::core_binary_selection::CoreBinarySelection;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelectionPolicy, CoreBinarySelectionPort, CoreBinarySelectionRequest,
    CoreBinarySelectionSource,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::engine_id;
use singbox_process::command::{NativeConfigCheck, SingBoxCommandPort};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

pub(crate) struct AndroidSingBoxProfileLaunchSpecProvider<S: SecureStore> {
    profile_store: Arc<ConfigManager<S>>,
    profile_document: ProfileDocument,
    binary_selection: Arc<dyn CoreBinarySelectionPort>,
    working_directory: PathBuf,
    expected_target: EngineBinaryTarget,
    command: Arc<dyn SingBoxCommandPort>,
}

impl<S: SecureStore> AndroidSingBoxProfileLaunchSpecProvider<S> {
    pub(crate) fn new(
        profile_store: Arc<ConfigManager<S>>,
        profile_document: ProfileDocument,
        binary_selection: Arc<dyn CoreBinarySelectionPort>,
        working_directory: PathBuf,
        expected_target: EngineBinaryTarget,
        command: Arc<dyn SingBoxCommandPort>,
    ) -> Self {
        Self {
            profile_store,
            profile_document,
            binary_selection,
            working_directory,
            expected_target,
            command,
        }
    }
}

#[async_trait]
impl<S: SecureStore> CoreLaunchSpecSource for AndroidSingBoxProfileLaunchSpecProvider<S> {
    async fn resolve(&self) -> Result<CoreLaunchSpec, PortError> {
        let engine_id = engine_id();
        validate_provider_engine(
            self.profile_store.engine_id(),
            self.binary_selection.engine_id(),
            &engine_id,
        )?;
        validate_document_identity(&self.profile_document, &engine_id)?;

        let selection = self
            .binary_selection
            .select(CoreBinarySelectionRequest {
                engine_id: engine_id.clone(),
                policy: CoreBinarySelectionPolicy::ManagedOnly,
            })
            .await?;
        validate_binary_selection(
            &selection,
            &engine_id,
            self.expected_target,
            &self.working_directory,
        )?;
        validate_binary_storage_path(&selection.binary_path, &self.working_directory).await?;

        let workspace = ProfileStore::load_workspace(
            self.profile_store.engine_profile_store(),
            &self.profile_document.identity.profile_id,
        )
        .await?;
        if workspace.document != self.profile_document {
            return Err(stale_profile_document());
        }
        let configuration_path = self
            .profile_store
            .existing_profile_document_path(self.profile_document.identity.profile_id.as_str())
            .await
            .map_err(config_failure)?;
        let configuration_bytes = fs::read(&configuration_path)
            .await
            .map_err(|error| PortError::Io(format!("read sing-box profile: {error}")))?;
        if configuration_bytes != self.profile_document.raw_bytes {
            return Err(stale_profile_document());
        }
        SingBoxControllerConfiguration::from_profile_json(&configuration_bytes)
            .map_err(config_failure)?;

        match self
            .command
            .check_config(&selection.binary_path, &configuration_path)
            .await?
        {
            NativeConfigCheck::Accepted => {}
            NativeConfigCheck::Rejected => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::Configuration,
                    "sing-box rejected the selected profile during native validation",
                    false,
                )));
            }
        }

        let final_workspace = ProfileStore::load_workspace(
            self.profile_store.engine_profile_store(),
            &self.profile_document.identity.profile_id,
        )
        .await?;
        let final_bytes = fs::read(&configuration_path)
            .await
            .map_err(|error| PortError::Io(format!("re-read sing-box profile: {error}")))?;
        if final_workspace.document != self.profile_document
            || final_bytes != self.profile_document.raw_bytes
        {
            return Err(stale_profile_document());
        }

        let target = AndroidCoreBinaryTarget::from_target_triple(
            self.expected_target
                .target_triple()
                .ok_or_else(|| unsupported_android_target(self.expected_target))?,
        )
        .ok_or_else(|| unsupported_android_target(self.expected_target))?;
        let configuration_path =
            CoreLaunchPath::new(path_argument(&configuration_path)?).map_err(config_failure)?;
        CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id,
                profile_id: self.profile_document.identity.profile_id.clone(),
                document_revision: self.profile_document.identity.revision.clone(),
            },
            CoreLaunchBinary::SingBoxManaged {
                version: selection.version,
                target,
            },
            self.working_directory.clone(),
            CoreLaunchArguments::SingBox { configuration_path },
        )
        .map_err(config_failure)
    }
}

fn validate_provider_engine(
    profile_engine: EngineId,
    selection_engine: EngineId,
    expected_engine: &EngineId,
) -> Result<(), PortError> {
    if profile_engine == *expected_engine && selection_engine == *expected_engine {
        Ok(())
    } else {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "Android sing-box launch provider received a foreign engine binding",
        ))
    }
}

fn validate_document_identity(
    document: &ProfileDocument,
    expected_engine: &EngineId,
) -> Result<(), PortError> {
    if document.identity.engine_id == *expected_engine {
        Ok(())
    } else {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "Android sing-box launch document belongs to a foreign engine",
        ))
    }
}

fn validate_binary_selection(
    selection: &CoreBinarySelection,
    expected_engine: &EngineId,
    expected_target: EngineBinaryTarget,
    working_directory: &Path,
) -> Result<(), PortError> {
    selection.validate_for(expected_engine, expected_target)?;
    if !matches!(selection.source, CoreBinarySelectionSource::ManagedVersion) {
        return Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "Android sing-box launch requires a managed version binary",
        ));
    }
    match &selection.verification {
        CoreArtifactVerification::Verified { engine_id, version }
            if engine_id == expected_engine && version == selection.version.raw() => {}
        _ => {
            return Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "Android sing-box binary selection lacks exact managed verification",
            ));
        }
    }
    let target_triple = expected_target
        .target_triple()
        .ok_or_else(|| unsupported_android_target(expected_target))?;
    if AndroidCoreBinaryTarget::from_target_triple(target_triple).is_none() {
        return Err(unsupported_android_target(expected_target));
    }
    let expected_path = working_directory
        .join("engines")
        .join("sing-box")
        .join("targets")
        .join(target_triple)
        .join("versions")
        .join(selection.version.raw())
        .join("sing-box");
    if selection.binary_path != expected_path {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "Android sing-box selected binary path does not match its managed target/version",
            false,
        )));
    }
    Ok(())
}

fn unsupported_android_target(target: EngineBinaryTarget) -> PortError {
    PortError::unsupported(
        Capability::CoreLifecycle,
        format!("sing-box target {target:?} is not a registered Android ABI"),
    )
}

async fn validate_binary_storage_path(
    binary_path: &Path,
    working_directory: &Path,
) -> Result<(), PortError> {
    let canonical_root = fs::canonicalize(working_directory)
        .await
        .map_err(|error| PortError::Io(format!("resolve Android app data directory: {error}")))?;
    let version_directory = binary_path.parent().ok_or_else(|| {
        PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "Android sing-box managed binary has no version directory",
            false,
        ))
    })?;
    let canonical_version_directory = fs::canonicalize(version_directory)
        .await
        .map_err(|error| PortError::Io(format!("resolve managed sing-box version: {error}")))?;
    let canonical_binary = fs::canonicalize(binary_path)
        .await
        .map_err(|error| PortError::Io(format!("resolve managed sing-box binary: {error}")))?;
    let metadata = fs::metadata(&canonical_binary)
        .await
        .map_err(|error| PortError::Io(format!("inspect managed sing-box binary: {error}")))?;
    if !canonical_version_directory.starts_with(&canonical_root)
        || canonical_binary.parent() != Some(canonical_version_directory.as_path())
        || canonical_binary.file_name().and_then(|name| name.to_str()) != Some("sing-box")
        || !metadata.is_file()
    {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "Android sing-box binary escaped its exact managed version directory",
            false,
        )));
    }
    Ok(())
}

fn config_failure(error: impl fmt::Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        format!("Android sing-box launch configuration is unavailable: {error}"),
        false,
    ))
}

fn stale_profile_document() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::NotReady,
        "Android sing-box profile changed after launch preflight",
        true,
    ))
}

fn path_argument(path: &Path) -> Result<String, PortError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| PortError::Failed("Android Core launch path is not valid UTF-8".to_owned()))
}

#[cfg(test)]
#[path = "singbox_tests.rs"]
mod tests;
