use infiltrator_application::core_application::CoreScopeObservation;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::surface::{SurfaceSource, overview_projection};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct DesktopSurfaceSource {
    pump: SurfacePump,
    observed_scope: Arc<RwLock<CoreScopeObservation>>,
}

impl DesktopSurfaceSource {
    pub fn new(pump: SurfacePump) -> Self {
        Self {
            pump,
            observed_scope: Arc::new(RwLock::new(CoreScopeObservation::Unavailable)),
        }
    }

    pub fn scope_observation(&self) -> Arc<RwLock<CoreScopeObservation>> {
        Arc::clone(&self.observed_scope)
    }
}

impl OverviewSource for DesktopSurfaceSource {
    fn current(&self) -> OverviewProjection {
        overview_projection(&self.pump.current())
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

impl SurfaceSource for DesktopSurfaceSource {
    fn surface_snapshot(&self) -> SurfaceSnapshot {
        self.pump.current()
    }

    fn observe_snapshot(&self, snapshot: &SurfaceSnapshot) {
        let scope = match snapshot.core.scope() {
            Some(scope) => CoreScopeObservation::Observed(scope),
            None => CoreScopeObservation::Unavailable,
        };
        *self
            .observed_scope
            .write()
            .expect("desktop surface Core scope lock") = scope;
    }
}

pub(super) fn observed_scope(observed: &Arc<RwLock<CoreScopeObservation>>) -> CoreScopeObservation {
    observed
        .read()
        .expect("desktop command Core scope lock")
        .clone()
}
