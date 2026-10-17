//! Engine-scoped store identity shared by every product persistence facade.
//!
//! [`ProfileStoreCore`] owns only what is identical for every engine, platform
//! and UI: the engine identity, the on-disk layout resolution, the injected
//! document codec, the injected [`StorePlatform`] and the one-shot namespace
//! initialization state. Credentials, subscription metadata, catalog ordering
//! and reset transactions belong to the product facade that wraps this core.

use crate::file_lock::ProfileWriteGuard;
use crate::layout::ProfileStoreLayout;
use crate::namespace::ProfileNamespaceInitialization;
use crate::platform::{NativeStorePlatform, StorePlatform};
use crate::settings::{
    read_settings_file_with_platform, remove_profile_metadata_with_platform,
    write_settings_file_with_platform,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Native filename suffix selected alongside the engine-owned document codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileDocumentExtension {
    Json,
    Yaml,
}

impl ProfileDocumentExtension {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
        }
    }
}

/// Shared persistence identity for one engine-scoped profile namespace.
///
/// The format adapter and the platform remain injected; this owner stores only
/// opaque bytes and immutable scope identity, and never resolves a credential
/// or a product path literal.
pub struct ProfileStoreCore {
    engine_id: EngineId,
    layout: Arc<ProfileStoreLayout>,
    config_root: PathBuf,
    config_dir: PathBuf,
    settings_file: PathBuf,
    document_codec: Arc<dyn ProfileDocumentCodecPort>,
    document_codec_scope: ProfileDocumentCodecScope,
    document_extension: ProfileDocumentExtension,
    namespace_initialization: ProfileNamespaceInitialization,
    namespace_initialized: OnceCell<()>,
    platform: Arc<dyn StorePlatform>,
}

impl ProfileStoreCore {
    /// Build a store over the native platform.
    pub fn new(
        engine_id: EngineId,
        config_root: PathBuf,
        layout: Arc<ProfileStoreLayout>,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self, PortError> {
        Self::with_platform(
            Arc::new(NativeStorePlatform),
            engine_id,
            config_root,
            layout,
            document_codec,
            document_extension,
            namespace_initialization,
        )
    }

    /// Build a store whose filesystem and kernel locks come from `platform`.
    pub fn with_platform(
        platform: Arc<dyn StorePlatform>,
        engine_id: EngineId,
        config_root: PathBuf,
        layout: Arc<ProfileStoreLayout>,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self, PortError> {
        let document_codec_scope = document_codec.scope();
        if document_codec_scope.engine_id != engine_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                format!(
                    "profile codec for {} cannot initialize store for {}",
                    document_codec_scope.engine_id.as_str(),
                    engine_id.as_str()
                ),
                false,
            )));
        }
        let config_dir = layout.engine_config_dir(&config_root, engine_id.as_str());
        let settings_file = layout.profile_state_file(&config_dir);

        Ok(Self {
            engine_id,
            layout,
            config_root,
            config_dir,
            settings_file,
            document_codec,
            document_codec_scope,
            document_extension,
            namespace_initialization,
            namespace_initialized: OnceCell::new(),
            platform,
        })
    }

    pub fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    pub fn layout(&self) -> &Arc<ProfileStoreLayout> {
        &self.layout
    }

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn settings_file(&self) -> &Path {
        &self.settings_file
    }

    /// The filesystem and kernel-lock implementation behind this store.
    pub fn platform(&self) -> &Arc<dyn StorePlatform> {
        &self.platform
    }

    pub fn document_codec(&self) -> &Arc<dyn ProfileDocumentCodecPort> {
        &self.document_codec
    }

    pub fn document_codec_scope(&self) -> &ProfileDocumentCodecScope {
        &self.document_codec_scope
    }

    pub const fn document_extension(&self) -> ProfileDocumentExtension {
        self.document_extension
    }

    pub fn namespace_initialization(&self) -> &ProfileNamespaceInitialization {
        &self.namespace_initialization
    }

    pub fn namespace_is_initialized(&self) -> bool {
        self.namespace_initialized.get().is_some()
    }

    /// Run one caller-provided namespace initialization exactly once.
    pub async fn initialize_namespace_with<F, Fut, E>(&self, initialize: F) -> Result<(), E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), E>>,
    {
        self.namespace_initialized
            .get_or_try_init(initialize)
            .await
            .map(|_| ())
    }

    /// Acquire the shared engine write fence in process-local and cross-process
    /// order. Every profile read/write path must hold it.
    pub async fn lock_profile_writes(&self) -> Result<ProfileWriteGuard, PortError> {
        let fence = self
            .platform
            .lock_profile_writes(&self.layout, &self.config_dir)
            .await?;
        Ok(ProfileWriteGuard::new(fence))
    }

    /// Read the engine-scoped settings document.
    pub async fn read_settings_value(&self) -> Result<toml::Value, PortError> {
        read_settings_file_with_platform(&self.platform, &self.settings_file).await
    }

    /// Replace the engine-scoped settings document.
    pub async fn write_settings_value(&self, settings: &toml::Value) -> Result<(), PortError> {
        write_settings_file_with_platform(&self.platform, &self.settings_file, settings).await
    }

    /// Drop one profile's row from the engine-scoped settings document.
    pub async fn remove_profile_metadata(&self, profile: &str) -> Result<(), PortError> {
        remove_profile_metadata_with_platform(&self.platform, &self.settings_file, profile).await
    }
}
