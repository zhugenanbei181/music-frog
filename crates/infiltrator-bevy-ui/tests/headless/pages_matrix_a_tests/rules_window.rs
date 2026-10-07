//! Behavior cases for rules window.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use bevy::math::Vec2;
use infiltrator_bevy_ui::pages::rules_view::visible_projection_rows;

#[test]
fn test_rules_window_mounts_bounded_rows_for_50k_projection() {
    use infiltrator_domain::rules::view;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Rules);

    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(rules_projection_with(50_000)));
    app.update();

    let viewport = RulesViewState::default().viewport_height_px;
    let bound = view::rendered_row_bound(viewport);
    let mounted = mounted_rule_rows(&mut app);
    assert!(
        !mounted.is_empty() && mounted.len() <= bound,
        "a 50,000-rule projection must mount a bounded window, got {}",
        mounted.len()
    );
    assert_eq!(mounted.first().copied(), Some(0));
    {
        let view = app.world().resource::<RulesViewState>();
        assert_eq!(view.rendered_rows, mounted.len());
        assert_eq!(view.filtered_indices.len(), 50_000);
        assert_eq!(
            visible_projection_rows(view).len(),
            mounted.len(),
            "the shared window is the mounted entity set"
        );
    }

    // Scrolling the list viewport shifts the mounted window; it never grows
    // and never walks the whole list.
    let scroll_entity = app
        .world_mut()
        .query_filtered::<Entity, With<RulesListScrollArea>>()
        .single(app.world())
        .expect("rules list scroll area");
    app.world_mut()
        .get_mut::<ScrollPosition>(scroll_entity)
        .expect("scroll position")
        .0 = Vec2::new(0.0, view::rule_scroll_offset_for_index(25_000));
    app.update();

    let scrolled = mounted_rule_rows(&mut app);
    assert!(scrolled.len() <= bound);
    assert_eq!(
        scrolled.len(),
        app.world().resource::<RulesViewState>().rendered_rows
    );
    assert_eq!(
        scrolled.first().copied(),
        Some(25_000 - view::RULE_WINDOW_OVERSCAN)
    );
    assert!(scrolled.contains(&25_000));
    assert!(!scrolled.contains(&0));

    // A live projection refresh rebuilds the covered rows but keeps the user's
    // scroll position: the viewport never jumps to the top on its own.
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(rules_projection_with(50_000)));
    app.update();
    assert_eq!(
        app.world().resource::<RulesViewState>().scroll_offset_px,
        view::rule_scroll_offset_for_index(25_000)
    );
    assert_eq!(
        mounted_rule_rows(&mut app).first().copied(),
        Some(25_000 - view::RULE_WINDOW_OVERSCAN)
    );

    // A recomputed filter moves the viewport together with the window, so the
    // mounted rows and the scroll position can never disagree.
    let field = rules_search_field(&mut app);
    app.world_mut()
        .get_mut::<TextField>(field)
        .expect("text field")
        .0 = TextFieldState::new("host-42.example");
    app.update();
    assert_eq!(mounted_rule_rows(&mut app), vec![42]);
    assert_eq!(
        app.world()
            .get::<ScrollPosition>(scroll_entity)
            .expect("scroll position")
            .0
            .y,
        0.0
    );
    assert_eq!(
        app.world().resource::<RulesViewState>().scroll_offset_px,
        0.0
    );
}
