//! Process-local owner of engine-keyed version factories for Android `:vpn`.

use std::path::PathBuf;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::map_port_error;
use infiltrator_application::core_version_management_application::CoreVersionManagementApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_core::engine_version_backend_factory::{
    EngineVersionBackendFactoryRegistry, MihomoEngineVersionBackendFactory,
};
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::core_binary_selection::CoreBinarySelectionPort;
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::error::PortError;
use singbox_version::version::SingBoxEngineVersionBackendFactory;
use std::sync::{Arc, Mutex, OnceLock};

enum AndroidCoreVersionOwnerState {
    Uninitialized,
    Ready(Arc<AndroidCoreVersionOwner>),
}

struct AndroidCoreVersionOwner {
    factory: Arc<EngineVersionBackendFactoryRegistry>,
    application: Arc<CoreVersionManagementApplication>,
}

impl AndroidCoreVersionOwner {
    fn build() -> Result<Self, FfiStatus> {
        let home = get_home_dir().map_err(map_port_error)?;
        Self::build_at_home(home)
    }

    fn build_at_home(home: PathBuf) -> Result<Self, FfiStatus> {
        let mut registrations: Vec<(EngineId, Arc<dyn EngineVersionBackendFactory>)> = vec![(
            MihomoVersionPort::engine_identifier(),
            Arc::new(MihomoEngineVersionBackendFactory::with_home(home.clone())),
        )];
        match SingBoxEngineVersionBackendFactory::current(home) {
            Ok(factory) => registrations.push((
                singbox_api::engine_id(),
                Arc::new(factory) as Arc<dyn EngineVersionBackendFactory>,
            )),
            Err(PortError::Unsupported { .. }) => {}
            Err(error) => {
                return Err(FfiStatus::err(
                    FfiErrorCode::NotReady,
                    format!("Android sing-box version backend is unavailable: {error}"),
                ));
            }
        }
        let factory = Arc::new(
            EngineVersionBackendFactoryRegistry::new(registrations).map_err(|error| {
                FfiStatus::err(
                    FfiErrorCode::NotReady,
                    format!("Android Core version factory registry is invalid: {error}"),
                )
            })?,
        );
        let runtime = infiltrator_composition::tokio_application_runtime().map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android Core version runtime is unavailable: {error}"),
            )
        })?;
        let application = Arc::new(CoreVersionManagementApplication::new(
            Arc::clone(&factory) as Arc<dyn EngineVersionBackendFactory>,
            runtime,
        ));
        Ok(Self {
            factory,
            application,
        })
    }
}

fn owner_state() -> &'static Mutex<AndroidCoreVersionOwnerState> {
    static OWNER: OnceLock<Mutex<AndroidCoreVersionOwnerState>> = OnceLock::new();
    OWNER.get_or_init(|| Mutex::new(AndroidCoreVersionOwnerState::Uninitialized))
}

fn with_owner<T>(
    select: impl FnOnce(&AndroidCoreVersionOwner) -> Result<T, FfiStatus>,
) -> Result<T, FfiStatus> {
    let mut state = owner_state().lock().map_err(|_| {
        FfiStatus::err(
            FfiErrorCode::Unknown,
            "Android Core version owner state is unavailable",
        )
    })?;
    if matches!(*state, AndroidCoreVersionOwnerState::Uninitialized) {
        *state = AndroidCoreVersionOwnerState::Ready(Arc::new(AndroidCoreVersionOwner::build()?));
    }
    let AndroidCoreVersionOwnerState::Ready(owner) = &*state else {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core version owner was not initialized",
        ));
    };
    select(owner)
}

pub(crate) fn core_version_application(
    engine_id: EngineId,
) -> Result<VersionApplication, FfiStatus> {
    with_owner(|owner| {
        owner
            .application
            .version_application(&engine_id)
            .map_err(|failure| FfiStatus::err(FfiErrorCode::NotReady, failure.to_string()))
    })
}

pub(crate) fn core_version_management_application()
-> Result<Arc<CoreVersionManagementApplication>, FfiStatus> {
    with_owner(|owner| Ok(Arc::clone(&owner.application)))
}

pub(crate) fn core_binary_selection_port(
    engine_id: &EngineId,
) -> Result<Arc<dyn CoreBinarySelectionPort>, FfiStatus> {
    with_owner(|owner| {
        owner
            .factory
            .binary_selection_port(engine_id)
            .map_err(|error| FfiStatus::err(FfiErrorCode::NotReady, error.to_string()))
    })
}

#[cfg(test)]
#[path = "core_version_owner_tests.rs"]
mod tests;
