//! Shared deterministic facts for native grouping scenarios and contract tests.
use crate::connection_rate_application::connections_page_snapshot;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::shell_readout_fold::ShellReadoutProjection;
use infiltrator_test_support::connection_grouping_fixtures::grouping_snapshot;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::{PageData, SurfaceOrigin};

/// Publish fixture facts as one coherent surface, including shell and overview counts.
pub fn grouping_surface(mut surface: SurfaceSnapshot) -> SurfaceSnapshot {
    let connections = grouping_snapshot();
    let page = connections_page_snapshot(&connections, &Default::default());
    surface.origin = SurfaceOrigin::Demo;
    surface.failure = None;
    surface.core.failure = None;
    surface.core.lifecycle = CoreLifecycle::Running;
    surface.revision += 1;
    surface.core.revision += 1;
    surface.core.active_connections = page.total_connections as u32;
    if let Some(overview) = surface.pages.overview.data.as_mut() {
        overview.active_connections = surface.core.active_connections;
    }
    surface.pages.connections = PageData::ready(page);
    surface.shell_readout = ShellReadoutProjection::default().project(&surface);
    surface
}
