//! Headless tests for the list: the virtual-window clamp math, the
//! scrollable column scene, in-place selection re-projection onto the nav
//! vocabulary, and theme reskin — entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::list::{
    List, ListSelection, VirtualList, VirtualListScroll, VirtualListSelect, VirtualListState,
    clamp_scroll_offset, list_row_scene, list_scene, visible_window, visible_window_with_overscan,
};
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, nav_fill};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{Breakpoint, LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn virtual_list_massive_dataset_o1_geometry() {
    // 1,000,000 items with row height 40.0px in 800px viewport.
    let item_count = 1_000_000;
    let row_h = 40.0;
    let viewport_h = 800.0;
    let mut state = VirtualListState::new(item_count, row_h, viewport_h).with_overscan(5);

    assert_eq!(state.item_count(), 1_000_000);
    assert_eq!(state.row_height_px(), 40.0);
    assert_eq!(state.viewport_height_px(), 800.0);
    assert_eq!(state.overscan(), 5);
    assert_eq!(state.total_height_px(), 40_000_000.0);
    assert_eq!(state.max_scroll_offset(), 40_000_000.0 - 800.0);
    assert!(state.is_at_top());
    assert!(!state.is_at_bottom());
    assert_eq!(state.scroll_progress(), 0.0);

    // Initial window at top: visible is [0, 20), with overscan 5: [0, 25).
    let win = state.window();
    assert_eq!(win.start, 0);
    assert_eq!(win.end, 25);
    assert_eq!(win.visible_count, 25);
    assert_eq!(win.top_spacer_px, 0.0);
    assert_eq!(win.bottom_spacer_px, (1_000_000.0 - 25.0) * 40.0);
    assert_eq!(
        win.top_spacer_px + win.visible_count as f32 * row_h + win.bottom_spacer_px,
        state.total_height_px()
    );

    // Jump to middle: item 500,000.
    state.scroll_to_index_centered(500_000);
    assert!(!state.is_at_top());
    assert!(!state.is_at_bottom());
    assert!((state.scroll_progress() - 0.5).abs() < 0.01);
    assert!(state.is_item_visible(500_000));
    assert!(state.is_item_mounted(500_000));

    let mid_win = state.window();
    assert!(mid_win.start <= 500_000 && mid_win.end > 500_000);
    assert_eq!(
        mid_win.top_spacer_px + mid_win.visible_count as f32 * row_h + mid_win.bottom_spacer_px,
        state.total_height_px()
    );

    // Scroll to bottom.
    state.scroll_to_bottom();
    assert!(state.is_at_bottom());
    assert_eq!(state.scroll_progress(), 1.0);
    let end_win = state.window();
    assert_eq!(end_win.end, 1_000_000);
    assert_eq!(end_win.bottom_spacer_px, 0.0);
    assert_eq!(
        end_win.top_spacer_px + end_win.visible_count as f32 * row_h + end_win.bottom_spacer_px,
        state.total_height_px()
    );
}

#[test]
fn virtual_list_scrolling_and_navigation_actions() {
    let mut state = VirtualListState::new(100, 30.0, 300.0).with_overscan(2);

    // scroll_by delta
    assert!(state.scroll_by(90.0));
    assert_eq!(state.scroll_offset_px(), 90.0);
    assert_eq!(state.visible_range(), (3, 13));

    // page_down & page_up
    assert!(state.page_down());
    assert_eq!(state.scroll_offset_px(), 390.0);
    assert!(state.page_up());
    assert_eq!(state.scroll_offset_px(), 90.0);

    // scroll_to_index: item already visible -> no scroll
    assert!(!state.scroll_to_index(5));
    assert_eq!(state.scroll_offset_px(), 90.0);

    // scroll_to_index: item below viewport -> scrolls so item bottom aligns with viewport bottom
    assert!(state.scroll_to_index(20));
    let expected_offset = (21.0 * 30.0) - 300.0;
    assert_eq!(state.scroll_offset_px(), expected_offset);
    assert!(state.is_item_visible(20));

    // scroll_to_index: item above viewport -> scrolls so item top aligns with viewport top
    assert!(state.scroll_to_index(2));
    assert_eq!(state.scroll_offset_px(), 60.0);
    assert!(state.is_item_visible(2));

    // scroll_to_top
    assert!(state.scroll_to_top());
    assert_eq!(state.scroll_offset_px(), 0.0);
    assert!(state.is_at_top());
}

#[test]
fn virtual_list_hit_testing_and_item_queries() {
    let state = VirtualListState::new(50, 40.0, 400.0).with_scroll_offset(200.0);

    // Hit test item_at_offset in content space
    assert_eq!(state.item_at_offset(0.0), Some(0));
    assert_eq!(state.item_at_offset(39.9), Some(0));
    assert_eq!(state.item_at_offset(40.0), Some(1));
    assert_eq!(state.item_at_offset(250.0), Some(6));
    assert_eq!(state.item_at_offset(1999.0), Some(49));
    assert_eq!(state.item_at_offset(2000.0), None);
    assert_eq!(state.item_at_offset(-10.0), None);

    // Item offsets and rects
    assert_eq!(state.item_offset_y(0), Some(0.0));
    assert_eq!(state.item_offset_y(5), Some(200.0));
    assert_eq!(state.item_rect_y(5), Some((200.0, 240.0)));
    assert_eq!(state.item_offset_y(100), None);
    assert_eq!(state.item_rect_y(100), None);
}

#[test]
fn virtual_list_selection_state_machine() {
    let mut state = VirtualListState::new(10, 30.0, 150.0);

    assert_eq!(state.selected_index(), None);
    assert!(state.select(Some(3)));
    assert_eq!(state.selected_index(), Some(3));
    assert!(!state.select(Some(3))); // no-op returns false

    // select_next
    assert!(state.select_next());
    assert_eq!(state.selected_index(), Some(4));

    // select_previous
    assert!(state.select_previous());
    assert_eq!(state.selected_index(), Some(3));

    // select_and_reveal
    assert!(state.select_and_reveal(9));
    assert_eq!(state.selected_index(), Some(9));
    assert!(state.is_item_visible(9));

    // Out of bounds selection is rejected, keeping previous selection
    assert!(!state.select(Some(100)));
    assert_eq!(state.selected_index(), Some(9));

    // Clear selection
    assert!(state.select(None));
    assert_eq!(state.selected_index(), None);
}

#[test]
fn virtual_list_dynamic_updates_and_clamping() {
    let mut state = VirtualListState::new(100, 30.0, 300.0)
        .with_scroll_offset(2000.0)
        .with_selected(Some(95));

    assert_eq!(state.scroll_offset_px(), 2000.0);
    assert_eq!(state.selected_index(), Some(95));

    // Shrink item count below current scroll and selection
    state.set_item_count(10);
    assert_eq!(state.item_count(), 10);
    // max_scroll = (10 * 30) - 300 = 0.0
    assert_eq!(state.scroll_offset_px(), 0.0);
    assert_eq!(state.selected_index(), Some(9));

    // Update viewport height
    state.set_viewport_height(200.0);
    assert_eq!(state.viewport_height_px(), 200.0);
    assert_eq!(state.max_scroll_offset(), 100.0);

    // Update row height
    state.set_row_height(50.0);
    assert_eq!(state.row_height_px(), 50.0);
    assert_eq!(state.total_height_px(), 500.0);
}

#[test]
fn virtual_list_ecs_message_advancement() {
    let mut app = headless_app();
    app.world_mut().spawn(VirtualList(
        VirtualListState::new(100, 30.0, 300.0).with_overscan(2),
    ));
    app.update();

    // Send scroll message
    app.world_mut().write_message(VirtualListScroll(120.0));
    app.update();

    let world = app.world_mut();
    let mut lists = world.query::<&VirtualList>();
    let list = lists.single(world).expect("virtual list");
    assert_eq!(list.0.scroll_offset_px(), 120.0);

    // Send select message
    app.world_mut().write_message(VirtualListSelect(Some(7)));
    app.update();

    let world = app.world_mut();
    let mut lists = world.query::<&VirtualList>();
    let list = lists.single(world).expect("virtual list");
    assert_eq!(list.0.selected_index(), Some(7));
}

#[test]
fn responsive_breakpoint_classification() {
    assert_eq!(Breakpoint::MOBILE_PX, 600.0);
    assert_eq!(Breakpoint::TABLET_PX, 1024.0);

    // Mobile range (< 600px)
    let mobile = Breakpoint::from_width(375.0);
    assert_eq!(mobile, Breakpoint::Mobile);
    assert!(mobile.is_mobile());
    assert!(mobile.is_compact());
    assert!(!mobile.is_tablet());
    assert!(!mobile.is_desktop());
    assert_eq!(mobile.sidebar_width_px(), None);

    let mobile_edge = Breakpoint::from_width(599.9);
    assert_eq!(mobile_edge, Breakpoint::Mobile);

    // Tablet range (600px .. 1024px)
    let tablet_min = Breakpoint::from_width(600.0);
    assert_eq!(tablet_min, Breakpoint::Tablet);
    assert!(!tablet_min.is_mobile());
    assert!(!tablet_min.is_compact());
    assert!(tablet_min.is_tablet());
    assert!(!tablet_min.is_desktop());
    assert_eq!(tablet_min.sidebar_width_px(), Some(240.0));

    let tablet_mid = Breakpoint::from_width(768.0);
    assert_eq!(tablet_mid, Breakpoint::Tablet);

    let tablet_edge = Breakpoint::from_width(1023.9);
    assert_eq!(tablet_edge, Breakpoint::Tablet);

    // Desktop range (>= 1024px)
    let desktop_min = Breakpoint::from_width(1024.0);
    assert_eq!(desktop_min, Breakpoint::Desktop);
    assert!(!desktop_min.is_mobile());
    assert!(!desktop_min.is_compact());
    assert!(!desktop_min.is_tablet());
    assert!(desktop_min.is_desktop());
    assert_eq!(desktop_min.sidebar_width_px(), Some(240.0));

    let desktop_wide = Breakpoint::from_width(1920.0);
    assert_eq!(desktop_wide, Breakpoint::Desktop);
}

#[test]
fn visible_window_covers_the_viewport_with_clamping() {
    // Everything fits.
    assert_eq!(visible_window(10, 300.0, 30.0, 0.0), (0, 10));
    // Partially visible rows count as visible.
    assert_eq!(visible_window(100, 250.0, 30.0, 0.0), (0, 9));
    // A scrolled offset moves the window.
    assert_eq!(visible_window(100, 300.0, 30.0, 450.0), (15, 25));
    // Scrolling past the end pins to the last full window.
    assert_eq!(visible_window(100, 300.0, 30.0, 100_000.0), (90, 100));
    // A negative offset pins to the first window.
    assert_eq!(visible_window(100, 300.0, 30.0, -50.0), (0, 10));
    // Degenerate inputs stay total.
    assert_eq!(visible_window(0, 300.0, 30.0, 0.0), (0, 0));
    assert_eq!(visible_window(10, 0.0, 30.0, 0.0), (0, 0));
    assert_eq!(visible_window(10, 300.0, 0.0, 0.0), (0, 0));
    assert_eq!(
        visible_window(3, 300.0, 30.0, 0.0),
        (0, 3),
        "never out of bounds"
    );
}

#[test]
fn virtual_window_with_overscan_calculates_spacers_and_buffers() {
    let win = visible_window_with_overscan(100, 300.0, 30.0, 300.0, 2);
    // Base visible is [10, 20). With overscan 2: [8, 22).
    assert_eq!(win.start, 8);
    assert_eq!(win.end, 22);
    assert_eq!(win.visible_count, 14);
    assert_eq!(win.top_spacer_px, 8.0 * 30.0);
    assert_eq!(win.bottom_spacer_px, (100.0 - 22.0) * 30.0);
    assert_eq!(win.total_height_px, 3000.0);
    assert_eq!(
        win.top_spacer_px + win.visible_count as f32 * 30.0 + win.bottom_spacer_px,
        win.total_height_px
    );

    // Overscan clamped at start (index 0).
    let win_start = visible_window_with_overscan(100, 300.0, 30.0, 0.0, 5);
    assert_eq!(win_start.start, 0);
    assert_eq!(win_start.end, 15);
    assert_eq!(win_start.top_spacer_px, 0.0);

    // Overscan clamped at end (index 100).
    let win_end = visible_window_with_overscan(100, 300.0, 30.0, 5000.0, 5);
    assert_eq!(win_end.start, 85);
    assert_eq!(win_end.end, 100);
    assert_eq!(win_end.bottom_spacer_px, 0.0);

    // Degenerate inputs.
    let win_empty = visible_window_with_overscan(0, 300.0, 30.0, 0.0, 2);
    assert_eq!(win_empty.start, 0);
    assert_eq!(win_empty.end, 0);
    assert_eq!(win_empty.total_height_px, 0.0);
}

#[test]
fn scroll_offset_clamps_to_valid_extent() {
    assert_eq!(clamp_scroll_offset(-10.0, 100, 300.0, 30.0), 0.0);
    assert_eq!(clamp_scroll_offset(500.0, 100, 300.0, 30.0), 500.0);
    // Max scroll = 3000 - 300 = 2700.
    assert_eq!(clamp_scroll_offset(5000.0, 100, 300.0, 30.0), 2700.0);
    // Content smaller than viewport -> max scroll 0.
    assert_eq!(clamp_scroll_offset(100.0, 5, 300.0, 30.0), 0.0);
}

#[test]
fn list_scene_mounts_rows_and_selection_flip_reprojects_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let rows = (0..4)
                .map(|index| list_row_scene(format!("proxy {index}"), index == 1, &palette))
                .collect();
            commands.spawn_scene(list_scene(rows, Some(1), &palette));
        },
    );
    app.update();

    let row_ids = |world: &mut bevy::ecs::world::World| -> Vec<Entity> {
        let mut lists = world.query::<(Entity, &List, &Children)>();
        let (_, _, children) = lists.iter(world).next().expect("one list");
        children.iter().copied().collect()
    };
    let ids = row_ids(app.world_mut());
    assert_eq!(ids.len(), 4);

    let palette = UiPalette::new(&Theme::dark());
    let active_of = |world: &bevy::ecs::world::World, entity: Entity| {
        world.get::<NavActive>(entity).expect("row active bit").0
    };
    let fill_of = |world: &bevy::ecs::world::World, entity: Entity| {
        world.get::<BackgroundColor>(entity).expect("row fill").0
    };
    assert!(active_of(app.world_mut(), ids[1]), "row 1 selected");
    assert_eq!(fill_of(app.world_mut(), ids[1]), nav_fill(true, &palette));

    // Flip the selection to row 3: bits restamp, ids stay, paints follow.
    let world = app.world_mut();
    let mut lists = world.query::<(Entity, &ListSelection)>();
    let (list_id, _) = lists.iter(world).next().expect("one list");
    app.world_mut()
        .entity_mut(list_id)
        .insert(ListSelection(Some(3)));
    app.update();

    let world = app.world_mut();
    assert!(!active_of(world, ids[1]), "row 1 deselected in place");
    assert!(active_of(world, ids[3]), "row 3 selected in place");
    assert_eq!(fill_of(world, ids[3]), nav_fill(true, &palette));
    assert!(
        world.get::<NavItem>(ids[1]).is_some() && world.get::<NavItem>(ids[3]).is_some(),
        "the rows kept their entity ids across the flip"
    );
}

#[test]
fn theme_flip_repaints_the_list_and_rows_without_respawn() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let rows = (0..3)
                .map(|index| list_row_scene(format!("proxy {index}"), index == 0, &palette))
                .collect();
            commands.spawn_scene(list_scene(rows, Some(0), &palette));
        },
    );
    app.update();

    let ids: Vec<Entity> = {
        let world = app.world_mut();
        let mut lists = world.query::<(Entity, &List, &Children)>();
        let (_, _, children) = lists.iter(world).next().expect("one list");
        children.iter().copied().collect()
    };

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    assert_eq!(
        world
            .get::<BackgroundColor>(ids[0])
            .expect("row survives")
            .0,
        nav_fill(true, &light),
        "the selected row re-derives from the light accent"
    );
    let mut lists = world.query::<&List>();
    assert_eq!(lists.iter(world).count(), 1, "the list itself survives");
}
