//! Engine-keyed version backend registry and the Mihomo product adapter.

use crate::error::CoreError;
use crate::mihomo_binary_selection::MihomoBinarySelectionAdapter;
use crate::version_port::MihomoVersionPort;
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::core_binary_selection::CoreBinarySelectionPort;
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::version::VersionPort;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use mihomo_version::manager::VersionManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Resolves version and executable-selection ports only through the adapter
/// registered for the exact engine identity.
pub struct EngineVersionBackendFactoryRegistry {
    factories: HashMap<EngineId, Arc<dyn EngineVersionBackendFactory>>,
}

impl EngineVersionBackendFactoryRegistry {
    pub fn new(
        registrations: impl IntoIterator<Item = (EngineId, Arc<dyn EngineVersionBackendFactory>)>,
    ) -> Result<Self, PortError> {
        let mut factories = HashMap::new();
        for (engine_id, factory) in registrations {
            if factories.insert(engine_id.clone(), factory).is_some() {
                return Err(PortError::Failed(format!(
                    "duplicate version backend registration for engine {}",
                    engine_id.as_str()
                )));
            }
        }
        Ok(Self { factories })
    }

    fn factory(
        &self,
        engine_id: &EngineId,
    ) -> Result<&Arc<dyn EngineVersionBackendFactory>, PortError> {
        self.factories.get(engine_id).ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreVersionInstall,
                format!(
                    "no version backend is registered for engine {}",
                    engine_id.as_str()
                ),
            )
        })
    }
}

impl EngineVersionBackendFactory for EngineVersionBackendFactoryRegistry {
    fn version_port(&self, engine_id: &EngineId) -> Result<Arc<dyn VersionPort>, PortError> {
        let port = self.factory(engine_id)?.version_port(engine_id)?;
        if port.engine_id() != *engine_id {
            return Err(PortError::Failed(format!(
                "version backend registered for {} returned a port for {}",
                engine_id.as_str(),
                port.engine_id().as_str()
            )));
        }
        Ok(port)
    }

    fn binary_selection_port(
        &self,
        engine_id: &EngineId,
    ) -> Result<Arc<dyn CoreBinarySelectionPort>, PortError> {
        let port = self.factory(engine_id)?.binary_selection_port(engine_id)?;
        if port.engine_id() != *engine_id {
            return Err(PortError::Failed(format!(
                "binary selection backend registered for {} returned a port for {}",
                engine_id.as_str(),
                port.engine_id().as_str()
            )));
        }
        Ok(port)
    }
}

pub struct MihomoEngineVersionBackendFactory {
    home: PathBuf,
    verification: Arc<Mutex<CoreArtifactVerification>>,
}

impl MihomoEngineVersionBackendFactory {
    pub fn current() -> Result<Self, CoreError> {
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
    use async_trait::async_trait;
    use infiltrator_contract::core_capabilities::EngineBinaryTarget;
    use infiltrator_contract::version::{
        CoreRelease, CoreReleaseSummary, CoreRollbackSnapshot, EngineReleaseChannel,
        InstalledCoreVersion,
    };
    use infiltrator_ports::core_binary_selection::{
        CoreBinarySelection, CoreBinarySelectionRequest, CoreBinarySelectionSource,
    };
    use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
    use infiltrator_ports::version::{
        VersionActivationRequest, VersionInstallRequest, VersionRollbackRequest,
        VersionUninstallRequest,
    };
    use lurker_core_contract::identity::CoreVersion;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    struct FakeFactory {
        port_engine_id: EngineId,
        version_requests: Arc<AtomicUsize>,
        selection_requests: Arc<AtomicUsize>,
    }

    impl EngineVersionBackendFactory for FakeFactory {
        fn version_port(&self, _engine_id: &EngineId) -> Result<Arc<dyn VersionPort>, PortError> {
            self.version_requests.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(FakeVersionPort(self.port_engine_id.clone())))
        }

        fn binary_selection_port(
            &self,
            _engine_id: &EngineId,
        ) -> Result<Arc<dyn CoreBinarySelectionPort>, PortError> {
            self.selection_requests.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(FakeBinarySelectionPort(
                self.port_engine_id.clone(),
            )))
        }
    }

    struct FakeVersionPort(EngineId);

    #[async_trait]
    impl VersionPort for FakeVersionPort {
        fn engine_id(&self) -> EngineId {
            self.0.clone()
        }

        async fn list_installed(
            &self,
            _engine_id: &EngineId,
        ) -> Result<Vec<InstalledCoreVersion>, PortError> {
            Ok(Vec::new())
        }

        async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
            Ok(CoreRelease {
                engine_id: channel.engine_id,
                version: "v1.0.0".to_owned(),
                release_date: "2026-10-12".to_owned(),
            })
        }

        async fn list_releases(
            &self,
            _engine_id: &EngineId,
            _limit: usize,
        ) -> Result<Vec<CoreReleaseSummary>, PortError> {
            Ok(Vec::new())
        }

        async fn install(&self, _request: VersionInstallRequest) -> Result<(), PortError> {
            Ok(())
        }

        async fn activate(&self, _request: VersionActivationRequest) -> Result<(), PortError> {
            Ok(())
        }

        async fn uninstall(&self, _request: VersionUninstallRequest) -> Result<(), PortError> {
            Ok(())
        }

        async fn rollback(&self, _request: VersionRollbackRequest) -> Result<String, PortError> {
            Ok("v1.0.0".to_owned())
        }

        async fn rollback_snapshot(
            &self,
            engine_id: &EngineId,
        ) -> Result<CoreRollbackSnapshot, PortError> {
            Ok(CoreRollbackSnapshot::for_engine(engine_id.clone()))
        }
    }

    struct FakeBinarySelectionPort(EngineId);

    #[async_trait]
    impl CoreBinarySelectionPort for FakeBinarySelectionPort {
        fn engine_id(&self) -> EngineId {
            self.0.clone()
        }

        async fn select(
            &self,
            request: CoreBinarySelectionRequest,
        ) -> Result<CoreBinarySelection, PortError> {
            Ok(CoreBinarySelection {
                engine_id: request.engine_id,
                binary_target: EngineBinaryTarget::current(),
                version: CoreVersion::new("v1.0.0").expect("test version is valid"),
                binary_path: PathBuf::from("/fake/core"),
                related_asset_candidates: Vec::new(),
                source: CoreBinarySelectionSource::ManagedVersion,
                verification: CoreArtifactVerification::Unknown,
            })
        }
    }

    fn fake_factory(
        engine_id: EngineId,
    ) -> (
        Arc<dyn EngineVersionBackendFactory>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
    ) {
        let version_requests = Arc::new(AtomicUsize::new(0));
        let selection_requests = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(FakeFactory {
                port_engine_id: engine_id,
                version_requests: Arc::clone(&version_requests),
                selection_requests: Arc::clone(&selection_requests),
            }),
            version_requests,
            selection_requests,
        )
    }

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

    #[test]
    fn registry_routes_only_to_the_registered_engine_and_checks_port_identity() {
        let mihomo = MihomoVersionPort::engine_identifier();
        let sing_box = EngineId::new("sing-box").unwrap();
        let (factory, version_requests, selection_requests) = fake_factory(mihomo.clone());
        let registry = EngineVersionBackendFactoryRegistry::new([(mihomo.clone(), factory)])
            .expect("one engine registration");

        assert_eq!(registry.version_port(&mihomo).unwrap().engine_id(), mihomo);
        assert_eq!(
            registry
                .binary_selection_port(&MihomoVersionPort::engine_identifier())
                .unwrap()
                .engine_id(),
            MihomoVersionPort::engine_identifier()
        );
        assert!(matches!(
            registry.version_port(&sing_box),
            Err(PortError::Unsupported {
                capability: Capability::CoreVersionInstall,
                ..
            })
        ));
        assert_eq!(version_requests.load(Ordering::SeqCst), 1);
        assert_eq!(selection_requests.load(Ordering::SeqCst), 1);

        let mismatched =
            EngineVersionBackendFactoryRegistry::new([(sing_box.clone(), fake_factory(mihomo).0)])
                .expect("registration is syntactically valid");
        assert!(matches!(
            mismatched.version_port(&sing_box),
            Err(PortError::Failed(_))
        ));
        assert!(matches!(
            mismatched.binary_selection_port(&sing_box),
            Err(PortError::Failed(_))
        ));
    }

    #[test]
    fn registry_rejects_duplicate_engine_registration() {
        let engine_id = MihomoVersionPort::engine_identifier();
        let registrations = [
            (engine_id.clone(), fake_factory(engine_id.clone()).0),
            (
                engine_id,
                fake_factory(MihomoVersionPort::engine_identifier()).0,
            ),
        ];
        assert!(matches!(
            EngineVersionBackendFactoryRegistry::new(registrations),
            Err(PortError::Failed(_))
        ));
    }
}
