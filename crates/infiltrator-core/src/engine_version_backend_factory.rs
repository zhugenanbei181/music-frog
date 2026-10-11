//! Mihomo-only engine version backend composition for the current product.

use crate::mihomo_binary_selection::MihomoBinarySelectionAdapter;
use crate::version_port::MihomoVersionPort;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_ports::core_binary_selection::CoreBinarySelectionPort;
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::error::PortError;
use infiltrator_ports::version::VersionPort;
use mihomo_platform::paths::get_home_dir;
use mihomo_version::manager::VersionManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct MihomoEngineVersionBackendFactory {
    home: PathBuf,
    verification: Arc<Mutex<CoreArtifactVerification>>,
}

impl MihomoEngineVersionBackendFactory {
    pub fn current() -> anyhow::Result<Self> {
        Ok(Self::with_home(get_home_dir()?))
    }

    pub fn with_home(home: PathBuf) -> Self {
        Self::with_shared_verification(
            home,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
    }

    pub fn with_shared_verification(
        home: PathBuf,
        verification: Arc<Mutex<CoreArtifactVerification>>,
    ) -> Self {
        Self { home, verification }
    }
}

impl EngineVersionBackendFactory for MihomoEngineVersionBackendFactory {
    fn version_port(&self, engine_id: &EngineId) -> Result<Arc<dyn VersionPort>, PortError> {
        let mihomo_id = MihomoVersionPort::engine_identifier();
        if engine_id != &mihomo_id {
            return Err(PortError::unsupported(
                Capability::CoreVersionInstall,
                format!(
                    "this product currently assembles Mihomo only; engine {} is unsupported",
                    engine_id.as_str()
                ),
            ));
        }

        let manager = VersionManager::for_engine(self.home.clone(), engine_id.clone())
            .map_err(|error| PortError::Failed(error.to_string()))?;
        let port = MihomoVersionPort::with_shared_verification(
            engine_id.clone(),
            manager,
            Arc::clone(&self.verification),
        )?;
        Ok(Arc::new(port))
    }

    fn binary_selection_port(
        &self,
        engine_id: &EngineId,
    ) -> Result<Arc<dyn CoreBinarySelectionPort>, PortError> {
        let mihomo_id = MihomoVersionPort::engine_identifier();
        if engine_id != &mihomo_id {
            return Err(PortError::unsupported(
                Capability::CoreVersionInstall,
                format!(
                    "this product currently selects binaries for Mihomo only; engine {} is unsupported",
                    engine_id.as_str()
                ),
            ));
        }

        let manager = VersionManager::for_engine(self.home.clone(), engine_id.clone())
            .map_err(|error| PortError::Failed(error.to_string()))?;
        let port = MihomoBinarySelectionAdapter::new(
            engine_id.clone(),
            manager,
            Arc::clone(&self.verification),
        )?;
        Ok(Arc::new(port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
    use tempfile::TempDir;

    #[test]
    fn factory_builds_only_the_requested_mihomo_backend() {
        let home = TempDir::new().unwrap();
        let factory = MihomoEngineVersionBackendFactory::with_home(home.path().into());
        let engine_id = MihomoVersionPort::engine_identifier();

        let version_port = factory.version_port(&engine_id).unwrap();
        assert_eq!(version_port.engine_id(), engine_id);
        let selection_port = factory.binary_selection_port(&engine_id).unwrap();
        assert_eq!(selection_port.engine_id(), engine_id);
        assert!(
            !home.path().join("engines/mihomo/versions").exists(),
            "constructing a backend must not create storage before use"
        );
    }

    #[test]
    fn factory_returns_typed_unsupported_for_unknown_engine() {
        let home = TempDir::new().unwrap();
        let factory = MihomoEngineVersionBackendFactory::with_home(home.path().into());
        let engine_id = EngineId::new("sing-box").unwrap();

        let error = match factory.version_port(&engine_id) {
            Ok(_) => panic!("factory must reject an unimplemented engine"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::CoreVersionInstall,
                ..
            }
        ));
        assert!(
            !home.path().join("engines/sing-box").exists(),
            "unsupported engines must not receive placeholder storage"
        );

        let error = match factory.binary_selection_port(&engine_id) {
            Ok(_) => panic!("factory must reject an unimplemented binary selector"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::CoreVersionInstall,
                ..
            }
        ));
        assert!(
            !home.path().join("engines/sing-box").exists(),
            "unsupported engines must not receive binary selection storage"
        );
    }
}
