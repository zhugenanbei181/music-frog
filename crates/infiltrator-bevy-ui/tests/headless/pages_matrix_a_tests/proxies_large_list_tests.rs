use super::*;

/// BANDROID-009: a 2000-node projection mounts through the recycler window,
/// not the whole collection, and the tail is reachable once the window slides.
#[test]
fn test_proxies_large_list_mounts_a_bounded_recycled_window() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);

    // Publish a 2000-node projection before the page mounts, so the large-list
    // path is selected from the very first scene build.
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.core.revision = snapshot.revision;
    set_outbound_inventory(
        &mut snapshot,
        "BIG",
        (0..2000).map(|index| format!("node-{index:04}")).collect(),
        SelectionAvailability::Selectable,
        Some("node-0000"),
        2,
    );
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
    navigate_to(&mut app, Route::Proxies);
    app.update();
    app.update();

    let mounted = app
        .world_mut()
        .query::<&ProxyNodeButton>()
        .iter(app.world())
        .count();
    assert!(mounted > 0, "the recycler must mount the visible window");
    assert!(
        mounted < 2000,
        "the full 2000-node collection must not be mounted, got {mounted}"
    );
    assert!(
        mounted <= 200,
        "mounted rows must be viewport-bounded, got {mounted}"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<ProxiesVirtualNodes>>()
            .iter(app.world())
            .next()
            .is_some(),
        "the virtual node container must own the large list"
    );

    // Scrolling to the bottom slides the window to the tail row.
    let scroll = app
        .world_mut()
        .query_filtered::<Entity, With<ProxiesScrollArea>>()
        .single(app.world())
        .expect("proxies scroll area");
    app.world_mut()
        .get_mut::<ScrollPosition>(scroll)
        .expect("scroll position")
        .0
        .y = 1.0e6;
    app.update();
    assert!(
        app.world_mut()
            .query::<&ProxyNodeButton>()
            .iter(app.world())
            .any(|button| button.node_name == "node-1999"),
        "the tail node must become reachable"
    );

    // A live shrink below the threshold keeps rendering through the mounted
    // recycler (the scene is not rebuilt) and never duplicates the collection.
    let mut small = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    small.revision += 1;
    small.core.revision = small.revision;
    set_outbound_inventory(
        &mut small,
        "BIG",
        (0..10).map(|index| format!("node-{index}")).collect(),
        SelectionAvailability::Selectable,
        Some("node-0"),
        3,
    );
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(small));
    app.update();
    app.update();
    let mounted_small = app
        .world_mut()
        .query::<&ProxyNodeButton>()
        .iter(app.world())
        .count();
    assert_eq!(
        mounted_small, 10,
        "a shrunk list keeps rendering through the recycler without duplication"
    );
}
