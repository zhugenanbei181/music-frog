//! Headless tests for the list: the virtual-window clamp math, the
//! scrollable column scene, in-place selection re-projection onto the nav
//! vocabulary, and theme reskin — entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::ecs::world::World;
use bevy::feathers::theme::{SurfaceLevel, UiTheme};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::list::scroll_core::{
    SlotBinding, VirtualEntityPool, VirtualSlot, clamp_scroll_offset, visible_window,
    visible_window_with_overscan,
};
use infiltrator_bevy_widgets::list::{
    FixedExtentUpdate, RowHeightMode, RowMeasurementOutcome, ScrollOffsetOutcome,
    ScrollToItemOutcome, SelectionAndRevealOutcome, SelectionOutcome,
};
use infiltrator_bevy_widgets::list::{
    List, ListSelection, VirtualList, VirtualListScroll, VirtualListSelect, VirtualListState,
    list_row_scene, list_scene,
};
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, nav_fill_token};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{Breakpoint, Theme, ThemeSkin};
use zhuge_ui_core::virtual_list::{ExtentUpdate, VirtualListConfig};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

fn expected_fill(world: &World, selected: bool) -> Color {
    world
        .resource::<UiTheme>()
        .context_color(&nav_fill_token(selected).theme_token(), SurfaceLevel::Base)
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
    assert!(matches!(
        state.scroll_to_index_centered(500_000),
        ScrollToItemOutcome::Scrolled { index: 500_000, .. }
    ));
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
    let _ = state.scroll_to_bottom();
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
    assert_eq!(
        state.scroll_by(90.0),
        ScrollOffsetOutcome::Changed {
            previous_offset_px: 0.0,
            offset_px: 90.0,
        }
    );
    assert_eq!(state.scroll_offset_px(), 90.0);
    assert_eq!(state.visible_range(), (3, 13));

    // page_down & page_up
    assert_eq!(
        state.page_down(),
        ScrollOffsetOutcome::Changed {
            previous_offset_px: 90.0,
            offset_px: 390.0,
        }
    );
    assert_eq!(state.scroll_offset_px(), 390.0);
    assert_eq!(
        state.page_up(),
        ScrollOffsetOutcome::Changed {
            previous_offset_px: 390.0,
            offset_px: 90.0,
        }
    );
    assert_eq!(state.scroll_offset_px(), 90.0);

    // scroll_to_index: item already visible -> no scroll
    assert_eq!(
        state.scroll_to_index(5),
        ScrollToItemOutcome::AlreadyVisible { index: 5 }
    );
    assert_eq!(state.scroll_offset_px(), 90.0);

    // scroll_to_index: item below viewport -> scrolls so item bottom aligns with viewport bottom
    assert_eq!(
        state.scroll_to_index(20),
        ScrollToItemOutcome::Scrolled {
            index: 20,
            previous_offset_px: 90.0,
            offset_px: 330.0,
        }
    );
    let expected_offset = (21.0 * 30.0) - 300.0;
    assert_eq!(state.scroll_offset_px(), expected_offset);
    assert!(state.is_item_visible(20));

    // scroll_to_index: item above viewport -> scrolls so item top aligns with viewport top
    assert_eq!(
        state.scroll_to_index(2),
        ScrollToItemOutcome::Scrolled {
            index: 2,
            previous_offset_px: 330.0,
            offset_px: 60.0,
        }
    );
    assert_eq!(state.scroll_offset_px(), 60.0);
    assert!(state.is_item_visible(2));

    // scroll_to_top
    assert_eq!(
        state.scroll_to_top(),
        ScrollOffsetOutcome::Changed {
            previous_offset_px: 60.0,
            offset_px: 0.0,
        }
    );
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
    assert_eq!(state.item_at_offset(f32::NAN), None);

    // Item offsets and rects
    assert_eq!(state.item_offset(0), Some(0.0));
    assert_eq!(state.item_offset(5), Some(200.0));
    assert_eq!(state.item_rect_y(5), Some((200.0, 240.0)));
    assert_eq!(state.item_offset(100), None);
    assert_eq!(state.item_rect_y(100), None);
}

#[test]
fn virtual_list_selection_reports_typed_transitions() {
    let mut state = VirtualListState::new(10, 30.0, 150.0);

    assert_eq!(state.selected_index(), None);
    assert_eq!(
        state.select(Some(3)),
        SelectionOutcome::Changed {
            previous_index: None,
            selected_index: Some(3),
        }
    );
    assert_eq!(state.selected_index(), Some(3));
    assert_eq!(
        state.select(Some(3)),
        SelectionOutcome::Unchanged {
            selected_index: Some(3),
        }
    );

    // select_next
    assert_eq!(
        state.select_next(),
        SelectionOutcome::Changed {
            previous_index: Some(3),
            selected_index: Some(4),
        }
    );
    assert_eq!(state.selected_index(), Some(4));

    // select_previous
    assert_eq!(
        state.select_previous(),
        SelectionOutcome::Changed {
            previous_index: Some(4),
            selected_index: Some(3),
        }
    );
    assert_eq!(state.selected_index(), Some(3));

    // select_and_reveal
    assert_eq!(
        state.select_and_reveal(9),
        SelectionAndRevealOutcome {
            selection: SelectionOutcome::Changed {
                previous_index: Some(3),
                selected_index: Some(9),
            },
            reveal: ScrollToItemOutcome::Scrolled {
                index: 9,
                previous_offset_px: 0.0,
                offset_px: 150.0,
            },
        }
    );
    assert_eq!(state.selected_index(), Some(9));
    assert!(state.is_item_visible(9));

    // Out of bounds selection is rejected, keeping previous selection
    assert_eq!(
        state.select(Some(100)),
        SelectionOutcome::IndexOutOfRange {
            index: 100,
            item_count: 10,
        }
    );
    assert_eq!(state.selected_index(), Some(9));

    // Clear selection
    assert_eq!(
        state.select(None),
        SelectionOutcome::Changed {
            previous_index: Some(9),
            selected_index: None,
        }
    );
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
    assert_eq!(
        state.set_fixed_extent(50.0),
        FixedExtentUpdate::Updated {
            previous_extent_px: 30.0,
            extent_px: 50.0,
        }
    );
    assert_eq!(state.row_height_px(), 50.0);
    assert_eq!(state.total_height_px(), 500.0);
}

#[test]
fn variable_extent_adapter_reports_measurement_and_mode_outcomes() {
    let mut variable = VirtualListState::new_variable_extent(4, 20.0, 40.0);
    assert_eq!(
        variable.update_measured_extent(1, 35.0),
        RowMeasurementOutcome::VariableExtent(ExtentUpdate::Updated {
            index: 1,
            previous_extent: 20.0,
            extent: 35.0,
        })
    );
    assert_eq!(variable.total_height_px(), 95.0);
    assert_eq!(
        variable.update_measured_extent(8, 10.0),
        RowMeasurementOutcome::VariableExtent(ExtentUpdate::IndexOutOfRange {
            index: 8,
            item_count: 4,
        })
    );
    assert_eq!(
        variable.set_fixed_extent(24.0),
        FixedExtentUpdate::VariableExtentMode
    );

    let mut fixed = VirtualListState::new(4, 20.0, 40.0);
    assert_eq!(
        fixed.update_measured_extent(1, 35.0),
        RowMeasurementOutcome::FixedExtentMode
    );
}

#[test]
fn variable_extent_window_uses_shared_core_geometry_after_measurements() {
    let mut state = VirtualListState::new_variable_extent(6, 20.0, 50.0).with_overscan(1);
    assert!(matches!(
        state.update_measured_extent(1, 40.0),
        RowMeasurementOutcome::VariableExtent(ExtentUpdate::Updated { index: 1, .. })
    ));
    assert!(matches!(
        state.update_measured_extent(2, 10.0),
        RowMeasurementOutcome::VariableExtent(ExtentUpdate::Updated { index: 2, .. })
    ));
    assert!(matches!(
        state.update_measured_extent(4, 35.0),
        RowMeasurementOutcome::VariableExtent(ExtentUpdate::Updated { index: 4, .. })
    ));
    assert_eq!(state.total_height_px(), 145.0);
    assert_eq!(
        state.set_scroll_offset(45.0),
        ScrollOffsetOutcome::Changed {
            previous_offset_px: 0.0,
            offset_px: 45.0,
        }
    );

    let RowHeightMode::VariableExtent(shared) = state.height_mode() else {
        panic!("variable-extent state must retain the shared index");
    };
    let visible = shared.compute_window(state.viewport_height_px(), state.scroll_offset_px(), 0);
    let mounted = shared.compute_window(
        state.viewport_height_px(),
        state.scroll_offset_px(),
        state.overscan(),
    );

    assert_eq!(state.visible_range(), (visible.start, visible.end));
    let product = state.window();
    assert_eq!((product.start, product.end), (mounted.start, mounted.end));
    assert_eq!(product.visible_count, mounted.materialized_len());
    assert_eq!(product.top_spacer_px, mounted.top);
    assert_eq!(product.bottom_spacer_px, mounted.bottom);
    assert_eq!(product.total_height_px, shared.total_extent());
}

#[test]
fn scroll_offset_outcomes_preserve_state_on_invalid_input_and_report_clamping() {
    let mut state = VirtualListState::new(10, 30.0, 90.0).with_scroll_offset(30.0);
    assert!(matches!(
        state.set_scroll_offset(f32::NAN),
        ScrollOffsetOutcome::InvalidRequest {
            requested_offset_px,
            current_offset_px: 30.0,
        } if requested_offset_px.is_nan()
    ));
    assert_eq!(state.scroll_offset_px(), 30.0);
    assert_eq!(
        state.set_scroll_offset(f32::MAX),
        ScrollOffsetOutcome::Clamped {
            requested_offset_px: f32::MAX,
            previous_offset_px: 30.0,
            offset_px: 210.0,
        }
    );
}

#[test]
fn empty_viewport_keeps_full_virtual_extent_without_mounting_rows() {
    let window = visible_window_with_overscan(100, 0.0, 30.0, 0.0, 4);
    assert_eq!((window.start, window.end, window.visible_count), (0, 0, 0));
    assert_eq!(
        (
            window.top_spacer_px,
            window.bottom_spacer_px,
            window.total_height_px,
        ),
        (3000.0, 0.0, 3000.0)
    );
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
    assert_eq!(Breakpoint::COMPACT_MAX_PX, 600.0);
    assert_eq!(Breakpoint::MEDIUM_MAX_PX, 840.0);
    assert_eq!(Breakpoint::EXPANDED_MAX_PX, 1200.0);

    // Compact range (< 600px)
    let compact = Breakpoint::from_width(375.0);
    assert_eq!(compact, Breakpoint::Compact);
    assert!(compact.is_compact());
    assert!(compact.is_mobile());
    assert!(!compact.is_medium());
    assert!(!compact.is_expanded());
    assert_eq!(compact.sidebar_width_px(), None);

    let compact_edge = Breakpoint::from_width(599.9);
    assert_eq!(compact_edge, Breakpoint::Compact);

    // Medium range (600px .. 840px)
    let med_min = Breakpoint::from_width(600.0);
    assert_eq!(med_min, Breakpoint::Medium);
    assert!(!med_min.is_compact());
    assert!(med_min.is_medium());
    assert!(med_min.is_tablet());
    assert!(!med_min.is_expanded());
    assert_eq!(med_min.sidebar_width_px(), Some(64.0));

    let med_mid = Breakpoint::from_width(768.0);
    assert_eq!(med_mid, Breakpoint::Medium);

    let med_edge = Breakpoint::from_width(839.9);
    assert_eq!(med_edge, Breakpoint::Medium);

    // Expanded range (840px .. 1200px)
    let exp_min = Breakpoint::from_width(840.0);
    assert_eq!(exp_min, Breakpoint::Expanded);
    assert!(!exp_min.is_compact());
    assert!(!exp_min.is_medium());
    assert!(exp_min.is_expanded());
    assert!(exp_min.is_desktop());
    assert_eq!(exp_min.sidebar_width_px(), Some(240.0));

    // Ultra range (>= 1200px)
    let ultra = Breakpoint::from_width(1280.0);
    assert_eq!(ultra, Breakpoint::Ultra);
    assert!(ultra.is_ultra());
    assert!(ultra.is_desktop());
    assert_eq!(ultra.sidebar_width_px(), Some(280.0));
}

#[test]
fn visible_window_covers_the_viewport_with_clamping() {
    // Everything fits.
    assert_eq!(visible_window(10, 300.0, 30.0, 0.0), (0, 10));
    // Partially visible rows count as visible.
    assert_eq!(visible_window(100, 250.0, 30.0, 0.0), (0, 9));
    // A fractional first row still includes the partial row at the viewport end.
    assert_eq!(visible_window(100, 300.0, 30.0, 15.0), (0, 11));
    // A scrolled offset moves the window.
    assert_eq!(visible_window(100, 300.0, 30.0, 450.0), (15, 25));
    // Scrolling past the end pins to the last full window.
    assert_eq!(visible_window(100, 300.0, 30.0, 100_000.0), (90, 100));
    // A negative offset pins to the first window.
    assert_eq!(visible_window(100, 300.0, 30.0, -50.0), (0, 10));
    // Degenerate inputs stay total.
    assert_eq!(visible_window(0, 300.0, 30.0, 0.0), (0, 0));
    assert_eq!(visible_window(10, 0.0, 30.0, 0.0), (0, 0));
    assert_eq!(
        visible_window(10, 300.0, 0.0, 0.0),
        (0, 10),
        "invalid row extent uses the shared core's one-unit fallback"
    );
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
fn fixed_list_adapter_delegates_geometry_to_zhuge_ui_core() {
    for (item_count, viewport, row_height, offset, overscan) in [
        (100, 300.0, 30.0, 0.0, 0),
        (100, 250.0, 30.0, 15.0, 2),
        (100, 300.0, 30.0, 450.0, 5),
        (100, 300.0, 30.0, f32::MAX, 5),
    ] {
        let shared = VirtualListConfig::new(item_count, row_height, viewport)
            .with_scroll_offset(offset)
            .with_overscan(overscan);
        let visible = shared.compute_visible_viewport();
        assert_eq!(
            visible_window(item_count, viewport, row_height, offset),
            (visible.start, visible.end)
        );

        let shared = shared.compute_viewport();
        let product =
            visible_window_with_overscan(item_count, viewport, row_height, offset, overscan);
        assert_eq!((product.start, product.end), shared.key());
        assert_eq!(product.top_spacer_px, shared.top);
        assert_eq!(product.bottom_spacer_px, shared.bottom);
        assert_eq!(product.total_height_px, shared.total_extent(row_height));
        assert_eq!(
            clamp_scroll_offset(offset, item_count, viewport, row_height),
            VirtualListConfig::new(item_count, row_height, viewport)
                .with_scroll_offset(offset)
                .clamped_scroll_offset()
        );
    }
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
            commands.spawn_scene(list_scene(rows, Some(1)));
        },
    );
    app.update();

    let row_ids = |world: &mut World| -> Vec<Entity> {
        let mut lists = world.query::<(Entity, &List, &Children)>();
        let (_, _, children) = lists.iter(world).next().expect("one list");
        children.iter().copied().collect()
    };
    let ids = row_ids(app.world_mut());
    assert_eq!(ids.len(), 4);

    let expected_selected_fill = expected_fill(app.world(), true);
    let active_of =
        |world: &World, entity: Entity| world.get::<NavActive>(entity).expect("row active bit").0;
    let fill_of =
        |world: &World, entity: Entity| world.get::<BackgroundColor>(entity).expect("row fill").0;
    assert!(active_of(app.world_mut(), ids[1]), "row 1 selected");
    assert_eq!(fill_of(app.world_mut(), ids[1]), expected_selected_fill);

    // Flip the selection to row 3: bits restamp, ids stay, paints follow.
    let world = app.world_mut();
    let mut lists = world.query::<(Entity, &ListSelection)>();
    let (list_id, _) = lists.iter(world).next().expect("one list");
    app.world_mut()
        .entity_mut(list_id)
        .insert(ListSelection(Some(3)));
    app.update();

    let expected_selected_fill = expected_fill(app.world(), true);
    let world = app.world_mut();
    assert!(!active_of(world, ids[1]), "row 1 deselected in place");
    assert!(active_of(world, ids[3]), "row 3 selected in place");
    assert_eq!(fill_of(world, ids[3]), expected_selected_fill);
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
            commands.spawn_scene(list_scene(rows, Some(0)));
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
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let expected_selected_fill = expected_fill(app.world(), true);
    let world = app.world_mut();
    assert_eq!(
        world
            .get::<BackgroundColor>(ids[0])
            .expect("row survives")
            .0,
        expected_selected_fill,
        "the selected row re-derives from the light accent"
    );
    let mut lists = world.query::<&List>();
    assert_eq!(lists.iter(world).count(), 1, "the list itself survives");
}

#[test]
fn virtual_entity_pool_slot_rebind_protection_rejects_delayed_receipts_and_queued_clicks() {
    let mut app = headless_app();
    let slot_entities: Vec<Entity> = (0..5).map(|_| app.world_mut().spawn_empty().id()).collect();

    let mut pool = VirtualEntityPool::new(slot_entities);
    assert_eq!(pool.capacity(), 5);
    assert_eq!(pool.active_count(), 0);

    // Initial mount: items 0..5 into slots 0..5
    let actions = pool.sync_window(0, 5, |idx| idx as f32 * 40.0);
    assert_eq!(actions.len(), 5);
    assert_eq!(pool.active_count(), 5);

    // Check slot 2: bound to item 2, generation 1
    assert_eq!(
        pool.slot_binding(2),
        Some((SlotBinding::Bound { item_idx: 2 }, 1))
    );
    assert!(pool.is_receipt_valid(2, 2, 1));
    assert!(!pool.is_receipt_valid(2, 2, 0)); // stale generation rejected
    assert!(!pool.is_receipt_valid(2, 3, 1)); // wrong item rejected

    let mut slot_comp = VirtualSlot::new(2);
    slot_comp.apply_rebind(2, 1);
    assert!(slot_comp.is_bound_to(2));
    assert_eq!(slot_comp.bound_item_idx(), Some(2));
    assert!(slot_comp.is_active());
    assert!(slot_comp.is_valid_receipt(2, 1));
    assert!(!slot_comp.is_valid_receipt(2, 0));

    // Simulate user clicking slot 2 (item 2 at generation 1).
    // An async command or delayed callback is queued with (slot: 2, item: 2, gen: 1).
    let queued_click_target = (2usize, 2usize, 1u64);

    // Now user scrolls down: window slides to [10, 15).
    let rebind_actions = pool.sync_window(10, 15, |idx| idx as f32 * 40.0);
    assert_eq!(rebind_actions.len(), 5);
    // Slot 2 is now rebound to item 12 at generation 2
    assert_eq!(
        pool.slot_binding(2),
        Some((SlotBinding::Bound { item_idx: 12 }, 2))
    );
    slot_comp.apply_rebind(12, 2);

    // The delayed receipt / queued click from the earlier interaction arrives!
    // Rebind protection guard checks whether the receipt is still valid for slot 2:
    assert!(
        !pool.is_receipt_valid(
            queued_click_target.0,
            queued_click_target.1,
            queued_click_target.2
        ),
        "delayed click or receipt targeting item 2 must be rejected after slot 2 was rebound to item 12"
    );
    assert!(
        !slot_comp.is_valid_receipt(queued_click_target.1, queued_click_target.2),
        "VirtualSlot component must reject stale receipt targeting previous item and generation"
    );

    // Valid receipt for current binding succeeds
    assert!(pool.is_receipt_valid(2, 12, 2));
    assert!(slot_comp.is_valid_receipt(12, 2));

    // Deactivate slots when window shrinks to [10, 12) (only 2 active slots)
    let _deact_actions = pool.sync_window(10, 12, |idx| idx as f32 * 40.0);
    assert_eq!(pool.active_count(), 2);
    // Slot 2 was deactivated: generation advanced to 3, the slot is free
    assert_eq!(pool.slot_binding(2), Some((SlotBinding::Free, 3)));
    slot_comp.apply_deactivate(3);
    // One binding, one truth: a free slot has no item and is not active.
    assert_eq!(slot_comp.bound_item_idx(), None);
    assert!(!slot_comp.is_active());
    assert!(!pool.is_receipt_valid(2, 12, 2));
    assert!(!pool.is_receipt_valid(2, 12, 3));
    assert!(!slot_comp.is_valid_receipt(12, 2));
    assert!(!slot_comp.is_valid_receipt(12, 3));

    // Scrolling back so slot 2 rebinds to item 2 again: generation is now 4!
    let _scroll_back_actions = pool.sync_window(0, 5, |idx| idx as f32 * 40.0);
    assert_eq!(
        pool.slot_binding(2),
        Some((SlotBinding::Bound { item_idx: 2 }, 4))
    );
    slot_comp.apply_rebind(2, 4);

    // Even though slot 2 is once again bound to item 2, the old delayed click from generation 1 is STILL rejected!
    assert!(
        !pool.is_receipt_valid(
            queued_click_target.0,
            queued_click_target.1,
            queued_click_target.2
        ),
        "stale click targeting generation 1 must not match the new generation 4 binding of the same item"
    );
    assert!(!slot_comp.is_valid_receipt(queued_click_target.1, queued_click_target.2));
    assert!(pool.is_receipt_valid(2, 2, 4));
    assert!(slot_comp.is_valid_receipt(2, 4));
}

#[test]
fn virtual_list_pool_capacity_and_mounted_slots_are_strictly_bounded_by_viewport_and_overscan() {
    // Test dataset with 50,000 items in an 800px viewport with row height 40px and overscan 3.
    let item_count = 50_000;
    let row_height = 40.0;
    let viewport = 800.0;
    let overscan = 3;

    let mut state = VirtualListState::new(item_count, row_height, viewport).with_overscan(overscan);

    // Visible rows in viewport = 800 / 40 = 20.
    // Mounted rows with overscan = 20 + 2 * 3 = 26 (at interior positions).
    // The maximum possible mounted rows for any scroll offset is strictly bounded:
    let max_mounted_bound = ((viewport / row_height).ceil() as usize) + overscan * 2 + 2;

    // Pool capacity needed:
    let pool_capacity = max_mounted_bound;
    let mut app = headless_app();
    let dummy_entities: Vec<Entity> = (0..pool_capacity)
        .map(|_| app.world_mut().spawn_empty().id())
        .collect();
    let mut pool = VirtualEntityPool::new(dummy_entities);

    // Test across various scroll positions from top to middle to bottom
    let scroll_offsets = [
        0.0,
        15.0, // fractional top
        120.0,
        5000.0,
        20_000.0,
        500_000.0,
        1_000_000.0,
        state.max_scroll_offset() - 50.0,
        state.max_scroll_offset(),
    ];

    for &offset in &scroll_offsets {
        let _ = state.set_scroll_offset(offset);
        let win = state.window();

        // 1. Mounted window length must never exceed the viewport + overscan upper bound
        assert!(
            win.visible_count <= max_mounted_bound,
            "mounted rows count {} must be bounded by {}",
            win.visible_count,
            max_mounted_bound
        );

        // 2. Sync pool to window
        let _actions = pool.sync_window(win.start, win.end, |idx| idx as f32 * row_height);

        // 3. Active pool slots must be exactly equal to the mounted window visible count
        assert_eq!(
            pool.active_count(),
            win.visible_count,
            "active slots in pool must match the mounted window count"
        );

        // 4. Pool capacity never changes and is independent of 50,000 item dataset
        assert_eq!(pool.capacity(), pool_capacity);
        assert!(pool.capacity() <= 30);
    }
}
