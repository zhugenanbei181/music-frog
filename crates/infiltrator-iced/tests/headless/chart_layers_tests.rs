//! The three-layer chart cache facts (EFFECTS-031 #122-#125): what rebuilds
//! the static background, what rebuilds the data geometry, and that the
//! crosshair layer simply has no cache.
//! test-intent: behavior
use infiltrator_iced::view::chart::{
    BackgroundFacts, ChartLayers, DataFacts, needs_rebuild,
};

fn background(width: f32, height: f32, skin: &'static str) -> BackgroundFacts {
    BackgroundFacts { width, height, skin }
}

fn data(width: f32, height: f32, skin: &'static str, samples: usize, head: (u64, u64)) -> DataFacts {
    DataFacts {
        width,
        height,
        skin,
        samples,
        head,
    }
}

#[test]
fn a_fresh_layer_always_rebuilds() {
    assert!(needs_rebuild(None, &background(100.0, 40.0, "dark")));
    assert!(needs_rebuild(None, &data(100.0, 40.0, "dark", 60, (1, 2))));
}

#[test]
fn identical_facts_keep_the_cached_layers() {
    let seen = background(100.0, 40.0, "dark");
    assert!(!needs_rebuild(Some(seen), &background(100.0, 40.0, "dark")));
    let seen = data(100.0, 40.0, "dark", 60, (1, 2));
    assert!(!needs_rebuild(Some(seen), &data(100.0, 40.0, "dark", 60, (1, 2))));
}

#[test]
fn bounds_or_skin_changes_rebuild_the_background() {
    let seen = background(100.0, 40.0, "dark");
    assert!(needs_rebuild(Some(seen), &background(120.0, 40.0, "dark")));
    assert!(needs_rebuild(Some(seen), &background(100.0, 48.0, "dark")));
    assert!(needs_rebuild(Some(seen), &background(100.0, 40.0, "light")));
}

#[test]
fn a_sliding_window_head_rebuilds_the_data_geometry() {
    let seen = data(100.0, 40.0, "dark", 60, (1, 2));
    // Same length, new head: the window slid.
    assert!(needs_rebuild(Some(seen), &data(100.0, 40.0, "dark", 60, (2, 3))));
    // Same head, new length: a sample landed or left.
    assert!(needs_rebuild(Some(seen), &data(100.0, 40.0, "dark", 61, (1, 2))));
}

#[test]
fn chart_layers_sync_without_rebuilding_on_equal_facts() {
    let layers = ChartLayers::default();
    let facts = background(100.0, 40.0, "dark");
    layers.sync_background(facts);
    // A second sync with equal facts is a no-op (and must not panic through
    // the interior-mutable facts cell).
    layers.sync_background(facts);
    layers.sync_data(data(100.0, 40.0, "dark", 60, (1, 2)));
    layers.sync_data(data(100.0, 40.0, "dark", 60, (1, 2)));
}
