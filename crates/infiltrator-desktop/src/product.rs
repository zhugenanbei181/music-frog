//! Ownership boundary for an independently launched desktop UI product.
use crate::composition::open_default_product_backend;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_ports::host_runtime::HostRuntime;
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

pub struct DesktopProductSession {
    executor: Runtime,
    application: Arc<CoreApplication>,
    state: DesktopProductSessionState,
}

enum DesktopProductSessionState {
    Open {
        host_runtime: Arc<dyn HostRuntime>,
        pump: SurfacePump,
    },
    Closed,
}
impl DesktopProductSession {
    /// Native ports are prepared offline; starting a stopped kernel remains an explicit intent.
    pub fn open(surface: SurfaceKind) -> anyhow::Result<Self> {
        let executor = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let backend = executor.block_on(open_default_product_backend(surface))?;
        Ok(Self {
            executor,
            application: backend.application,
            state: DesktopProductSessionState::Open {
                host_runtime: backend.host_runtime,
                pump: backend.surface_pump,
            },
        })
    }
    pub fn application(&self) -> Arc<CoreApplication> {
        self.application.clone()
    }
    pub fn surface_pump(&self) -> SurfacePump {
        match &self.state {
            DesktopProductSessionState::Open { pump, .. } => pump.clone(),
            DesktopProductSessionState::Closed => panic!("desktop product session is closed"),
        }
    }
    pub fn host_runtime(&self) -> Arc<dyn HostRuntime> {
        match &self.state {
            DesktopProductSessionState::Open { host_runtime, .. } => host_runtime.clone(),
            DesktopProductSessionState::Closed => panic!("desktop product session is closed"),
        }
    }
}
impl Drop for DesktopProductSession {
    fn drop(&mut self) {
        let state = std::mem::replace(&mut self.state, DesktopProductSessionState::Closed);
        if let DesktopProductSessionState::Open { host_runtime, pump } = state {
            drop(pump);
            if let Err(error) = self.executor.block_on(self.application.close()) {
                log::error!("desktop product shutdown failed: {}", error.message);
            }
            drop(host_runtime);
        }
    }
}
