//! Ownership boundary for an independently launched desktop UI product.
use crate::runtime::MihomoRuntime;
use crate::storage::home_dir;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::surface::SurfaceKind;
use mihomo_version::manager::VersionManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};

pub struct DesktopProductSession {
    executor: Runtime,
    host: Option<Arc<MihomoRuntime>>,
    pump: Option<SurfacePump>,
}
impl DesktopProductSession {
    /// Native ports are prepared offline; starting a stopped kernel remains an explicit intent.
    pub fn open(surface: SurfaceKind) -> anyhow::Result<Self> {
        let executor = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let vm = VersionManager::new()?;
        let home = home_dir()?;
        let host =
            Arc::new(executor.block_on(MihomoRuntime::prepare_offline(&vm, true, &[], &home))?);
        // Establish the cleanup owner before installing a facade that borrows the host.
        let mut session = Self {
            executor,
            host: Some(host.clone()),
            pump: None,
        };
        session.executor.block_on(host.install_product_commands())?;
        session.pump = Some(
            session
                .executor
                .block_on(host.surface_pump(surface, Duration::from_millis(700)))?,
        );
        Ok(session)
    }
    pub fn application(&self) -> Arc<CoreApplication> {
        self.host
            .as_ref()
            .expect("live desktop session")
            .application()
    }
    pub fn surface_pump(&self) -> SurfacePump {
        self.pump.as_ref().expect("composed desktop source").clone()
    }
    pub fn host_runtime(&self) -> Arc<MihomoRuntime> {
        self.host.as_ref().expect("live desktop session").clone()
    }
}
impl Drop for DesktopProductSession {
    fn drop(&mut self) {
        self.pump.take();
        if let Some(host) = self.host.take()
            && let Err(error) = self.executor.block_on(host.application().close())
        {
            log::error!("desktop product shutdown failed: {}", error.message);
        }
    }
}
