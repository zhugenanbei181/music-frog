//! Engine-scoped profile persistence with Mihomo configuration operations.
//!
//! [`ConfigManager`] owns one codec-bound engine directory, while product
//! selection and Core owner state use the shared configs-root protocol. The
//! implementation is split along business seams:
//!
//! * `profiles` — profile CRUD and per-profile metadata
//! * `active` — active-profile selection and settings-file plumbing
//! * `defaults` — default-config bootstrap and proxy-port conflict repair
//! * `controller` — external-controller endpoint management
//! * `paths` — configs-dir resolution, profile name validation and
//!   canonicalized path construction
//! * `subscription_store` — credential-store persistence of subscription URLs
//! * `metadata` — settings-TOML mapping helpers

use std::path;
use std::result;
mod active;
mod active_profile_reference;
mod controller;
mod defaults;
#[cfg(test)]
mod manager_test;
mod metadata;
mod namespace_migration;
pub mod paths;
mod profile_protection;
pub(crate) mod profiles;
mod subscription_store;

use crate::profile_document_codec::MihomoProfileDocumentCodec;
use crate::profile_write_boundary::shared_boundary;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::instance_lock::{
    ThreadedInstanceLockGuard, try_acquire_threaded_instance_lock,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::fs::{canonicalize, create_dir_all};
use tokio::sync::{OnceCell, OwnedMutexGuard};
use tokio::task::spawn_blocking;
use tokio::time::sleep;

pub struct ConfigManager<S: SecureStore> {
    engine_id: EngineId,
    config_root: PathBuf,
    config_dir: PathBuf,
    settings_file: PathBuf,
    legacy_settings_file: PathBuf,
    document_codec: Arc<dyn ProfileDocumentCodecPort>,
    document_codec_scope: ProfileDocumentCodecScope,
    document_extension: ProfileDocumentExtension,
    namespace_initialization: ProfileNamespaceInitialization,
    credential_store: S,
    profile_namespace_initialized: OnceCell<()>,
    pub(crate) apply_observations: RwLock<BTreeMap<String, ApplyTransactionSnapshot>>,
}

/// On-disk profile document suffix selected by the engine's storage adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileDocumentExtension {
    Json,
    Yaml,
}

impl ProfileDocumentExtension {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProfileNamespaceInitialization {
    MigrateLegacyMihomo,
    EngineScoped,
}

/// Both the process-local mutex and the stable OS lock are held across a
/// profile read/write transaction. The OS lock file lives beside the engine
/// directory so a directory reset cannot unlink its own fence.
pub(crate) struct ProfileWriteGuard {
    _local: OwnedMutexGuard<()>,
    _cross_process: ThreadedInstanceLockGuard,
}

/// Result of a reset whose profile files and active reference share one
/// unowned, revision-fenced transaction. `CleanupPending` means the default
/// reference already committed; a retry can finish deleting inactive files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceResetOutcome {
    NoPendingIntent,
    Committed {
        previous: ActiveProfileReferenceState,
        current: ActiveProfileReferenceState,
    },
    Stale {
        expected: ActiveProfileReferenceState,
        observed: ActiveProfileReferenceState,
    },
    CleanupPending {
        previous: ActiveProfileReferenceState,
        current: ActiveProfileReferenceState,
        error: String,
    },
}

/// A reset either resumes the durable transaction as recorded on disk or
/// starts a new transaction with caller-provided default document bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceResetRequest {
    ResumePendingIntent,
    Start {
        expected: ActiveProfileReferenceState,
        default_document: String,
    },
}

pub(crate) async fn lock_profile_writes_for_dir(config_dir: &Path) -> Result<ProfileWriteGuard> {
    let boundary = shared_boundary(config_dir)?;
    let lock_path = profile_write_lock_path(config_dir).await?;
    let cross_process = loop {
        let attempt_path = lock_path.clone();
        let attempt = spawn_blocking(move || try_acquire_threaded_instance_lock(&attempt_path))
            .await
            .map_err(|error| {
                MihomoError::Service(format!("profile write lock worker: {error}"))
            })??;
        if let Some(guard) = attempt {
            break guard;
        }
        sleep(Duration::from_millis(10)).await;
    };
    let local = boundary.lock_owned().await;
    Ok(ProfileWriteGuard {
        _local: local,
        _cross_process: cross_process,
    })
}

async fn profile_write_lock_path(config_dir: &Path) -> Result<PathBuf> {
    let engine = config_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| MihomoError::Config("engine config directory has no name".into()))?;
    let parent = config_dir
        .parent()
        .ok_or_else(|| MihomoError::Config("engine config directory has no parent".into()))?;
    create_dir_all(parent).await?;
    let canonical_parent = canonicalize(parent).await?;
    Ok(canonical_parent.join(format!(".profile-write-{engine}.lock")))
}

pub(crate) async fn ensure_profile_reset_not_pending_for_dir(
    config_dir: &Path,
) -> result::Result<(), PortError> {
    active_profile_reference::ensure_reset_not_pending_for_dir(config_dir).await
}

pub(crate) async fn lock_reset_fenced_profile_mutation_for_dir(
    config_dir: &Path,
) -> result::Result<active_profile_reference::ResetFencedProfileMutationGuard, PortError> {
    active_profile_reference::acquire_reset_fenced_profile_mutation_for_dir(config_dir).await
}

impl<S: SecureStore> ConfigManager<S> {
    pub fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    pub(crate) fn document_codec_scope(&self) -> &ProfileDocumentCodecScope {
        &self.document_codec_scope
    }

    pub(crate) fn document_extension(&self) -> ProfileDocumentExtension {
        self.document_extension
    }

    pub(crate) fn validate_document_bytes(&self, document: &[u8]) -> result::Result<(), PortError> {
        match self.document_codec.inspect(document)? {
            DocumentValidation::Valid => Ok(()),
            DocumentValidation::Invalid { diagnostics } => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "engine profile codec rejected the document".to_owned()),
                false,
            ))),
        }
    }

    pub fn with_home_and_store(home: PathBuf, credential_store: S) -> Result<Self> {
        Self::with_home_configs_dir_and_store(home, None, credential_store)
    }

    /// 同时指定 home 与共享 configs 根目录来源。Profile 内容和 profile
    /// 状态都落入 `<configs_dir>/engines/<engine-id>/`；应用设置仍由
    /// `<home>/settings.toml` 的 owner 管理。
    pub fn with_home_configs_dir_and_store(
        home: PathBuf,
        configs_dir: Option<&str>,
        credential_store: S,
    ) -> Result<Self> {
        Self::build_for_engine(
            home,
            configs_dir,
            credential_store,
            crate::engine_id(),
            Arc::new(MihomoProfileDocumentCodec),
            ProfileDocumentExtension::Yaml,
            ProfileNamespaceInitialization::MigrateLegacyMihomo,
        )
    }

    /// Construct one engine-scoped profile store over the shared on-disk
    /// profile/reference protocol. The codec authors the engine, format and
    /// schema identity; this path never migrates legacy Mihomo files.
    pub fn with_profile_codec_and_store(
        home: PathBuf,
        configs_dir: Option<&str>,
        credential_store: S,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
    ) -> Result<Self> {
        let codec_scope = document_codec.scope();
        Self::build_for_engine(
            home,
            configs_dir,
            credential_store,
            codec_scope.engine_id.clone(),
            document_codec,
            document_extension,
            ProfileNamespaceInitialization::EngineScoped,
        )
    }

    /// Construct an engine-scoped store beside an existing product manager's
    /// already-resolved shared configs root. This preserves non-UTF paths and
    /// user-selected configs roots when a product registers another engine.
    pub fn with_profile_codec_and_store_at_root(
        home: PathBuf,
        config_root: PathBuf,
        credential_store: S,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
    ) -> Result<Self> {
        let codec_scope = document_codec.scope();
        Self::build_for_engine_at_root(
            home,
            config_root,
            credential_store,
            codec_scope.engine_id.clone(),
            document_codec,
            document_extension,
            ProfileNamespaceInitialization::EngineScoped,
        )
    }

    fn build_for_engine(
        home: PathBuf,
        configs_dir: Option<&str>,
        credential_store: S,
        engine_id: EngineId,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self> {
        let config_root = paths::resolve_configs_dir_in(configs_dir, &home)?;
        Self::build_for_engine_at_root(
            home,
            config_root,
            credential_store,
            engine_id,
            document_codec,
            document_extension,
            namespace_initialization,
        )
    }

    fn build_for_engine_at_root(
        home: PathBuf,
        config_root: PathBuf,
        credential_store: S,
        engine_id: EngineId,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self> {
        if namespace_initialization == ProfileNamespaceInitialization::MigrateLegacyMihomo
            && engine_id != crate::engine_id()
        {
            return Err(MihomoError::Config(
                "legacy shared profile migration is only valid for Mihomo".to_owned(),
            ));
        }
        let document_codec_scope = document_codec.scope();
        if document_codec_scope.engine_id != engine_id {
            return Err(MihomoError::Config(format!(
                "profile codec for {} cannot initialize store for {}",
                document_codec_scope.engine_id.as_str(),
                engine_id.as_str()
            )));
        }
        let config_dir = paths::engine_config_dir(&config_root, &engine_id);
        let settings_file = config_dir.join("profile-state.toml");
        let legacy_settings_file = home.join("config.toml");

        Ok(Self {
            engine_id,
            config_root,
            config_dir,
            settings_file,
            legacy_settings_file,
            document_codec,
            document_codec_scope,
            document_extension,
            namespace_initialization,
            credential_store,
            profile_namespace_initialized: OnceCell::new(),
            apply_observations: RwLock::new(BTreeMap::new()),
        })
    }

    /// 解析后的 Mihomo namespace 目录。它位于共享 configs 根下的
    /// `engines/mihomo/`，并可能尚不存在（由迁移、doctor fix 或 save 创建）。
    pub fn config_dir(&self) -> &path::Path {
        &self.config_dir
    }

    /// Shared root before adding this adapter's engine namespace.
    pub fn config_root(&self) -> &path::Path {
        &self.config_root
    }

    /// Adapter transaction boundary shared by every manager of this store.
    /// Hold only across persistence comparison/publication; never across
    /// lifecycle callbacks that can themselves repair or save a profile.
    pub async fn lock_profile_writes(&self) -> Result<impl Send + '_> {
        lock_profile_writes_for_dir(&self.config_dir).await
    }
}
