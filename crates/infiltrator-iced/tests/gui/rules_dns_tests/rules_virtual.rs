//! Behavior cases for rules virtual.
//! test-intent: behavior

use super::*;
use crate::view::rules_window::{
    rendered_rule_rows, rules_window, rules_window_page, visible_rule_items,
};
use infiltrator_application::rule_list_fixtures::list_document;

#[test]
fn test_rules_virtual_window_renders_bounded_rows_for_50k_list() {
    use infiltrator_domain::rules::view;

    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..50_000)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(rules))));
    assert_eq!(state.editor.rules_render_cache.len(), 50_000);

    let viewport = view::RULE_DEFAULT_VIEWPORT_PX;
    let bound = view::rendered_row_bound(viewport);
    // The render list is a window, and a 50,000-entry list renders no more
    // rows than the bound derived from the viewport alone.
    assert!(
        rendered_rule_rows(&state) <= bound,
        "50k list must render a bounded window"
    );
    assert_eq!(
        state.diag.perf_snapshot.rules_visible_rows,
        rendered_rule_rows(&state),
        "the published perf fact is the real rendered-row count"
    );

    // Scrolling to the middle slides the window without growing it, and the
    // rendered band starts where the shared window says it does.
    let offset = view::rule_scroll_offset_for_index(25_000);
    let _ = state.update(Message::RulesListScrolled {
        offset_px: offset,
        viewport_px: viewport,
    });
    let window = rules_window(&state);
    assert_eq!(window.start + view::RULE_WINDOW_OVERSCAN, 25_000);
    assert!(window.contains(25_000));
    assert_eq!(rendered_rule_rows(&state), window.rendered_rows());
    assert!(rendered_rule_rows(&state) <= bound);
    assert_eq!(
        rules_window_page(&state),
        25_000 / view::DEFAULT_RULE_PAGE_SIZE
    );
    let rendered = visible_rule_items(&state);
    assert!(
        rendered
            .iter()
            .all(|index| *index >= 25_000 - view::RULE_WINDOW_OVERSCAN)
    );
    assert!(rendered.iter().all(|index| *index < 25_000 + bound));

    // Paging is a scroll jump to the shared page's first row; the window
    // follows the offset through the same shared reduction.
    let (page_start, _) = view::page_bounds(3, 50_000, view::DEFAULT_RULE_PAGE_SIZE);
    let _ = state.update(Message::RulesSetPage(3));
    assert_eq!(
        state.editor.rules_scroll_offset_px,
        view::rule_scroll_offset_for_index(page_start)
    );
    assert_eq!(rules_window_page(&state), 3);
    assert!(rendered_rule_rows(&state) <= bound);

    // A measured viewport replaces the declared fallback and tightens the
    // bound; a short viewport still renders at least one row band.
    let _ = state.update(Message::RulesListScrolled {
        offset_px: 0.0,
        viewport_px: 112.0,
    });
    assert_eq!(state.editor.rules_viewport_px, 112.0);
    let small_bound = view::rendered_row_bound(112.0);
    assert!(small_bound < bound);
    assert!(rendered_rule_rows(&state) <= small_bound);
    assert!(rendered_rule_rows(&state) > 0);
}
