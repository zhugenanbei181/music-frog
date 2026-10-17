//! Engine-scoped storage facade: product credentials over the neutral kernel.

use crate::layout::product_layout_owned;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use lurker_profile_store::file_lock::{ProfileWriteGuard, lock_profile_writes_for_dir};
use lurker_profile_store::layout::ProfileStoreLayout;
use lurker_profile_store::namespace::ProfileNamespaceInitialization;
use lurker_profile_store::store::{ProfileDocumentExtension, ProfileStoreCore};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Product persistence owner for one engine namespace.
///
/// The neutral [`ProfileStoreCore`] owns layout, state schema and document I/O;
/// this facade adds the product credential store and the in-process apply
/// observations that config managers expose to surfaces.
pub struct EngineProfileStore<S: SecureStore> {
    core: ProfileStoreCore,
    credential_store: S,
    apply_observations: RwLock<BTreeMap<String, ApplyTransactionSnapshot>>,
}

impl<S: SecureStore> EngineProfileStore<S> {
    pub fn new(
        engine_id: EngineId,
        config_root: PathBuf,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        credential_store: S,
    ) -> Result<Self, PortError> {
        Self::new_with_namespace_initialization(
            engine_id,
            config_root,
            document_codec,
            document_extension,
            credential_store,
            ProfileNamespaceInitialization::EngineScoped,
        )
    }

    pub fn new_with_namespace_initialization(
        engine_id: EngineId,
        config_root: PathBuf,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        credential_store: S,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self, PortError> {
        Self::with_layout(
            engine_id,
            config_root,
            Arc::new(product_layout_owned()),
            document_codec,
            document_extension,
            credential_store,
            namespace_initialization,
        )
    }

    /// Construct a store with an explicitly injected layout instead of the
    /// product-pinned default.
    pub fn with_layout(
        engine_id: EngineId,
        config_root: PathBuf,
        layout: Arc<ProfileStoreLayout>,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
        document_extension: ProfileDocumentExtension,
        credential_store: S,
        namespace_initialization: ProfileNamespaceInitialization,
    ) -> Result<Self, PortError> {
        let core = ProfileStoreCore::new(
            engine_id,
            config_root,
            layout,
            document_codec,
            document_extension,
            namespace_initialization,
        )?;
        Ok(Self {
            core,
            credential_store,
            apply_observations: RwLock::new(BTreeMap::new()),
        })
    }

    pub fn engine_id(&self) -> EngineId {
        self.core.engine_id()
    }

    pub fn layout(&self) -> &Arc<ProfileStoreLayout> {
        self.core.layout()
    }

    pub fn config_root(&self) -> &Path {
        self.core.config_root()
    }

    pub fn config_dir(&self) -> &Path {
        self.core.config_dir()
    }

    pub fn settings_file(&self) -> &Path {
        self.core.settings_file()
    }

    pub fn document_codec(&self) -> &Arc<dyn ProfileDocumentCodecPort> {
        self.core.document_codec()
    }

    pub fn document_codec_scope(&self) -> &ProfileDocumentCodecScope {
        self.core.document_codec_scope()
    }

    pub fn document_extension(&self) -> ProfileDocumentExtension {
        self.core.document_extension()
    }

    pub fn credential_store(&self) -> &S {
        &self.credential_store
    }

    pub fn namespace_initialization(&self) -> &ProfileNamespaceInitialization {
        self.core.namespace_initialization()
    }

    pub fn namespace_is_initialized(&self) -> bool {
        self.core.namespace_is_initialized()
    }

    /// Acquire the shared engine write fence in process-local and cross-process
    /// order. Every profile read/write path must hold it.
    pub async fn lock_profile_writes(&self) -> Result<ProfileWriteGuard, PortError> {
        lock_profile_writes_for_dir(self.layout(), self.config_dir()).await
    }

    pub async fn read_settings_value(&self) -> Result<toml::Value, PortError> {
        self.core.read_settings_value().await
    }

    pub async fn write_settings_value(&self, settings: &toml::Value) -> Result<(), PortError> {
        self.core.write_settings_value(settings).await
    }

    pub async fn remove_profile_metadata(&self, profile: &str) -> Result<(), PortError> {
        self.core.remove_profile_metadata(profile).await
    }

    pub(crate) async fn initialize_namespace_with<F, Fut, E>(&self, initialize: F) -> Result<(), E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), E>>,
    {
        self.core.initialize_namespace_with(initialize).await
    }

    pub fn apply_transaction(&self, profile: &str) -> Option<ApplyTransactionSnapshot> {
        self.apply_observations
            .read()
            .expect("apply observations")
            .get(profile)
            .cloned()
    }

    pub fn record_apply_transaction(&self, record: ApplyTransactionSnapshot) {
        self.apply_observations
            .write()
            .expect("apply observations")
            .insert(record.profile.clone(), record);
    }

    pub fn remove_apply_transaction(&self, profile: &str) {
        self.apply_observations
            .write()
            .expect("apply observations")
            .remove(profile);
    }
}
