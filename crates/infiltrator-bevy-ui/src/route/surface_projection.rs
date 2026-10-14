use super::*;

#[derive(Event, Clone, Debug, PartialEq)]
pub(super) struct SurfaceStatusChanged(pub SurfaceSnapshot);

/// Reconcile the status banner after a live snapshot replaces the initial
/// placeholder. A page keeps exactly one banner for each non-ready status;
/// Ready removes it, and a changed failure/loading message replaces it.
pub(super) fn reconcile_surface_status(
    update: On<SurfaceStatusChanged>,
    roots: Query<(Entity, &PageRoot)>,
    banners: Query<(Entity, &SurfaceStatusBanner)>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    let snapshot = &update.0;
    let mut retained_pages = HashSet::new();
    for (entity, banner) in &banners {
        let wanted = route_page_status(snapshot, banner.page);
        if matches!(wanted, PageStatus::Ready) || wanted != banner.status {
            commands.entity(entity).despawn_children();
            commands.entity(entity).despawn();
        } else {
            retained_pages.insert(banner.page);
        }
    }

    for (root_entity, root) in &roots {
        let page = page_id_for_route(root.0);
        let status = route_page_status(snapshot, page);
        if !matches!(status, PageStatus::Ready) && !retained_pages.contains(&page) {
            commands
                .spawn_scene(status_banner_scene(page, &status, &palette))
                .insert(ChildOf(root_entity));
        }
    }
}

pub(super) fn route_page_status(snapshot: &SurfaceSnapshot, page: PageId) -> PageStatus {
    snapshot.page_status(page)
}

pub(super) fn apply_surface_snapshot(
    update: On<SurfaceSnapshotUpdated>,
    latest: Res<LatestSurfaceSnapshot>,
    source: Option<Res<SurfaceSourceHandle>>,
    mut commands: Commands,
) {
    let snapshot = update.0.clone();
    if !snapshot.is_newer_than(&latest.0) {
        return;
    }
    if let Some(source) = source {
        source.0.observe_snapshot(&snapshot);
    }
    replay_surface_snapshot(snapshot, &mut commands);
}

/// Replay one composed source coherently. Capture-host initialization uses the
/// same projection wiring when it replaces its initial explicit demo source.
pub(crate) fn replay_surface_snapshot(snapshot: SurfaceSnapshot, commands: &mut Commands) {
    commands.insert_resource(LatestSurfaceSnapshot(snapshot.clone()));
    commands.insert_resource(LatestCoreLifecycle(core_lifecycle_projection(&snapshot)));
    commands.insert_resource(SidebarToggleProjection(
        system_toggle_projection::from_surface(&snapshot),
    ));
    trigger_page_projection_events(&snapshot, commands);
    commands.trigger(SurfaceStatusChanged(snapshot));
}
