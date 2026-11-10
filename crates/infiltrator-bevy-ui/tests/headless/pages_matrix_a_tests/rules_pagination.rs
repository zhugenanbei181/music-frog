//! Behavior cases for rules pagination.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_domain::rules::view::{
    rendered_row_bound, rule_scroll_offset_for_index, rule_window,
};

#[test]
fn test_rules_pagination_hides_rows_outside_page() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Rules);

    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(rules_projection_with(1_000)));
    app.update();

    {
        let mut view = app.world_mut().resource_mut::<RulesViewState>();
        view.page_size = 200;
        view.page = 4;
    }
    // The paging buttons move the viewport: page 5 of size 200 starts at row
    // 800, and the window follows that offset instead of hiding rows in place.
    let next = app
        .world_mut()
        .query_filtered::<Entity, With<RulesPageNextButton>>()
        .single(app.world())
        .expect("next page button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: next });
    app.update();

    let rows = mounted_rule_rows(&mut app);
    let bound = rendered_row_bound(RulesViewState::default().viewport_height_px);
    assert!(!rows.is_empty() && rows.len() <= bound);
    assert_eq!(
        rows.len(),
        app.world().resource::<RulesViewState>().rendered_rows
    );
    assert_eq!(
        app.world().resource::<RulesViewState>().scroll_offset_px,
        rule_scroll_offset_for_index(800)
    );
    // The window covers the page's last rows (800..819 clamped to the list),
    // never the first rows of the list.
    assert!(rows.contains(&800));
    assert!(rows.iter().all(|row| *row >= 796));
    assert!(!rows.contains(&0));

    let indicator = app
        .world_mut()
        .query::<(&Text, &RulesPageIndicator)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .find(|text| text.contains("页"))
        .expect("page indicator");
    // The caption states the *window*, so the expectation reads from the same
    // shared arithmetic the mounted rows came from instead of a literal: batch
    // 142 moved the rule window onto `zhuge-ui-core`'s virtual-list clipping,
    // and a hardcoded "797–815" was the old `first + band + overscan` answer.
    let state = app.world().resource::<RulesViewState>();
    let window = rule_window(state.scroll_offset_px, state.viewport_height_px, 1_000);
    let expected = LocalizedText::new(
        "rules_pagination_live",
        vec![
            ("page", (state.page + 1).to_string()),
            ("pages", "5".to_owned()),
            ("start", (window.start + 1).to_string()),
            ("end", window.end.to_string()),
            ("total", "1000".to_owned()),
        ],
    )
    .render(&UiLocale::new("zh-CN"));
    assert_eq!(indicator, expected);
    assert_eq!(
        window.rendered_rows(),
        app.world().resource::<RulesViewState>().rendered_rows,
        "the caption's window is the window that mounted the rows"
    );
    assert!(
        window.end < 1_000,
        "the last page still leaves overscan below the fold: {window:?}"
    );
}
