//! Engine-scoped storage identity and state shared by config facades.

use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::secure_store::SecureStore;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
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

/// Shared persistence state for one engine-scoped profile namespace.
///
/// The format adapter remains injected; this owner stores only opaque bytes and
/// immutable scope identity. Active-reference and workspace transactions are
/// added here as they leave product-specific config facades.
pub struct EngineProfileStore<S: SecureStore> {
    engine_id: EngineId,
    config_root: PathBuf,
    config_dir: PathBuf,
    settings_file: PathBuf,
    document_codec: Arc<dyn ProfileDocumentCodecPort>,
    document_codec_scope: ProfileDocumentCodecScope,
    document_extension: ProfileDocumentExtension,
    credential_store: S,
    namespace_initialized: OnceCell<()>,
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
        let config_dir = config_root.join("engines").join(engine_id.as_str());
        let settings_file = config_dir.join("profile-state.toml");

        Ok(Self {
            engine_id,
            config_root,
            config_dir,
            settings_file,
            document_codec,
            document_codec_scope,
            document_extension,
            credential_store,
            namespace_initialized: OnceCell::new(),
            apply_observations: RwLock::new(BTreeMap::new()),
        })
    }

    pub fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
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

    pub fn document_codec(&self) -> &Arc<dyn ProfileDocumentCodecPort> {
        &self.document_codec
    }

    pub fn document_codec_scope(&self) -> &ProfileDocumentCodecScope {
        &self.document_codec_scope
    }

    pub const fn document_extension(&self) -> ProfileDocumentExtension {
        self.document_extension
    }

    pub fn credential_store(&self) -> &S {
        &self.credential_store
    }

    pub fn namespace_is_initialized(&self) -> bool {
        self.namespace_initialized.get().is_some()
    }

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
