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
//! * `paths` — configs-dir resolution and typed engine-scoped path construction

use std::path;
use std::result;
mod active;
mod active_profile_reference;
mod controller;
mod defaults;
#[cfg(test)]
mod manager_test;
mod namespace_facade;
pub mod paths;
pub(crate) mod profiles;

use crate::profile_document_codec::MihomoProfileConfigurationCodec;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::reset_fence::{
    acquire_reset_fenced_profile_mutation_for_dir, ensure_reset_not_pending_for_dir,
};
use infiltrator_profile_store::store::EngineProfileStore;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecPort;
use lurker_profile_store::namespace::{
    LegacyProfileNamespaceMigrationV1, ProfileNamespaceInitialization,
};
use lurker_profile_store::owner_switch::reset::ResetFencedProfileMutationGuard;
use lurker_profile_store::store::ProfileDocumentExtension;
use mihomo_driver::error::{MihomoError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ConfigManager<S: SecureStore> {
    pub(crate) profile_store: Arc<EngineProfileStore<S>>,
}

pub(crate) async fn ensure_profile_reset_not_pending_for_dir(
    config_dir: &Path,
) -> result::Result<(), PortError> {
    ensure_reset_not_pending_for_dir(config_dir).await
}

pub(crate) async fn lock_reset_fenced_profile_mutation_for_dir(
    config_dir: &Path,
) -> result::Result<ResetFencedProfileMutationGuard, PortError> {
    acquire_reset_fenced_profile_mutation_for_dir(config_dir).await
}

impl<S: SecureStore> ConfigManager<S> {
    pub fn engine_id(&self) -> EngineId {
        self.profile_store.engine_id()
    }

    pub fn engine_profile_store(&self) -> &EngineProfileStore<S> {
        self.profile_store.as_ref()
    }

    pub fn shared_engine_profile_store(&self) -> Arc<EngineProfileStore<S>> {
        Arc::clone(&self.profile_store)
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
        let migration =
            ProfileNamespaceInitialization::LegacyV1(LegacyProfileNamespaceMigrationV1::new(
                home.join("config.toml"),
                Some("sync_state.db".into()),
                vec!["options".into(), "snapshots".into()],
            ));
        Self::build_for_engine(
            home,
            configs_dir,
            credential_store,
            crate::engine_id(),
            Arc::new(MihomoProfileConfigurationCodec),
            ProfileDocumentExtension::Yaml,
            migration,
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
        config_root: PathBuf,
        credential_store: S,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
    ) -> Result<Self> {
        let codec_scope = document_codec.scope();
        Self::build_for_engine_at_root(
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
            config_root,
            credential_store,
            engine_id,
            document_codec,
            document_extension,
            namespace_initialization,
        )
    }

    fn build_for_engine_at_root(
        config_root: PathBuf,
        credential_store: S,
        engine_id: EngineId,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self> {
        if matches!(
            &namespace_initialization,
            ProfileNamespaceInitialization::LegacyV1(_)
        ) && (engine_id != crate::engine_id()
            || document_extension != ProfileDocumentExtension::Yaml)
        {
            return Err(MihomoError::Config(
                "legacy shared profile migration is only valid for Mihomo YAML".to_owned(),
            ));
        }
        let profile_store = Arc::new(
            EngineProfileStore::new_with_namespace_initialization(
                engine_id,
                config_root,
                document_codec,
                document_extension,
                credential_store,
                namespace_initialization,
            )
            .map_err(|error| MihomoError::Config(error.to_string()))?,
        );

        Ok(Self { profile_store })
    }

    /// 解析后的 Mihomo namespace 目录。它位于共享 configs 根下的
    /// `engines/mihomo/`，并可能尚不存在（由迁移、doctor fix 或 save 创建）。
    pub fn config_dir(&self) -> &path::Path {
        self.profile_store.config_dir()
    }

    /// Shared root before adding this adapter's engine namespace.
    pub fn config_root(&self) -> &path::Path {
        self.profile_store.config_root()
    }

    pub(crate) fn settings_file(&self) -> &Path {
        self.profile_store.settings_file()
    }

    pub(crate) fn credential_store(&self) -> &S {
        self.profile_store.credential_store()
    }

    pub(crate) fn namespace_is_initialized(&self) -> bool {
        self.profile_store.namespace_is_initialized()
    }

    /// Adapter transaction boundary shared by every manager of this store.
    /// Hold only across persistence comparison/publication; never across
    /// lifecycle callbacks that can themselves repair or save a profile.
    pub async fn lock_profile_writes(&self) -> Result<impl Send + '_> {
        self.profile_store
            .lock_profile_writes()
            .await
            .map_err(|error| MihomoError::Service(error.to_string()))
    }
}
