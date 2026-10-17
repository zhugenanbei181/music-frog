//! Shared engine-scoped fixture for kernel unit tests.

use crate::layout::ProfileStoreLayout;
use crate::namespace::ProfileNamespaceInitialization;
use crate::store::{ProfileDocumentExtension, ProfileStoreCore};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

pub const FIXTURE_ENGINE: &str = "fixture-engine";

pub struct FixtureCodec(ProfileDocumentCodecScope);

impl ProfileDocumentCodecPort for FixtureCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.0.clone()
    }

    fn inspect(&self, _document: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

/// One neutral kernel store over a temporary config root.
pub struct TestCore {
    core: ProfileStoreCore,
    layout: Arc<ProfileStoreLayout>,
    root: TempDir,
}

impl TestCore {
    pub fn new(extension: ProfileDocumentExtension) -> Self {
        Self::with_layout(extension, ProfileStoreLayout::default())
    }

    pub fn with_layout(extension: ProfileDocumentExtension, layout: ProfileStoreLayout) -> Self {
        let root = TempDir::new().expect("temporary config root");
        let engine_id = EngineId::new(FIXTURE_ENGINE).expect("fixture engine id");
        let codec = Arc::new(FixtureCodec(ProfileDocumentCodecScope {
            engine_id: engine_id.clone(),
            format_id: ProfileFormatId::new("fixture-native").expect("fixture format id"),
            schema_version: ProfileSchemaVersion::new(1).expect("fixture schema version"),
        }));
        let layout = Arc::new(layout);
        let core = ProfileStoreCore::new(
            engine_id,
            root.path().to_path_buf(),
            layout.clone(),
            codec,
            extension,
            ProfileNamespaceInitialization::EngineScoped,
        )
        .expect("fixture core");
        Self { core, layout, root }
    }

    pub fn core(&self) -> &ProfileStoreCore {
        &self.core
    }

    pub fn layout(&self) -> &Arc<ProfileStoreLayout> {
        &self.layout
    }

    pub fn config_dir(&self) -> &Path {
        self.core.config_dir()
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }
}
