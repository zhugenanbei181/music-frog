//! Profile/config management for the mihomo infiltrator.
//!
//! [`ConfigManager`] owns an engine-scoped config directory and profile-state
//! file. The implementation is split along business seams:
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

use crate::profile_write_boundary::shared_boundary;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::instance_lock::{
    ThreadedInstanceLockGuard, try_acquire_threaded_instance_lock,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::{OnceCell, OwnedMutexGuard};

pub struct ConfigManager<S: SecureStore> {
    config_root: PathBuf,
    config_dir: PathBuf,
    settings_file: PathBuf,
    legacy_settings_file: PathBuf,
    credential_store: S,
    profile_namespace_initialized: OnceCell<()>,
    pub(crate) apply_observations: RwLock<BTreeMap<String, ApplyTransactionSnapshot>>,
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
        let attempt =
            tokio::task::spawn_blocking(move || try_acquire_threaded_instance_lock(&attempt_path))
                .await
                .map_err(|error| {
                    MihomoError::Service(format!("profile write lock worker: {error}"))
                })??;
        if let Some(guard) = attempt {
            break guard;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
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
    tokio::fs::create_dir_all(parent).await?;
    let canonical_parent = tokio::fs::canonicalize(parent).await?;
    Ok(canonical_parent.join(format!(".profile-write-{engine}.lock")))
}

pub(crate) async fn ensure_profile_reset_not_pending_for_dir(
    config_dir: &Path,
) -> std::result::Result<(), PortError> {
    active_profile_reference::ensure_reset_not_pending_for_dir(config_dir).await
}

pub(crate) async fn lock_reset_fenced_profile_mutation_for_dir(
    config_dir: &Path,
) -> std::result::Result<active_profile_reference::ResetFencedProfileMutationGuard, PortError> {
    active_profile_reference::acquire_reset_fenced_profile_mutation_for_dir(config_dir).await
}

impl<S: SecureStore> ConfigManager<S> {
    pub fn engine_id(&self) -> EngineId {
        crate::engine_id()
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
        let config_root = paths::resolve_configs_dir_in(configs_dir, &home)?;
        let engine = crate::engine_id();
        let config_dir = paths::engine_config_dir(&config_root, &engine);
        let settings_file = config_dir.join("profile-state.toml");
        let legacy_settings_file = home.join("config.toml");

        Ok(Self {
            config_root,
            config_dir,
            settings_file,
            legacy_settings_file,
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
