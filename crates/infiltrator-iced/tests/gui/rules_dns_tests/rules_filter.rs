//! Behavior cases for rules filter.
//! test-intent: behavior

use super::*;
use crate::view::rules_window::{rendered_rule_rows, rules_window_page};
use infiltrator_application::rule_list_fixtures::list_document;
use infiltrator_domain::rules::view::{
    clamp_page, filter_rule_indices, page_bounds, rendered_row_bound, rule_scroll_offset_for_index,
};

#[test]
fn test_rules_filter_and_pagination_delegate_to_shared_reduction() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..5)
        .map(|index| RuleEntry {
            rule: format!("DOMAIN-SUFFIX,site{index}.com,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(rules))));
    assert_eq!(
        state.editor.rules_filtered_indices,
        filter_rule_indices(&state.editor.rule_list.draft, "")
    );

    state.editor.rules_page_size = 2;
    let _ = state.update(Message::RulesSetPage(1));
    assert_eq!(page_bounds(1, 5, 2), (2, 4));
    assert_eq!(state.editor.rules_page, 1);
    // DUAL-11-08: the visible rows are the shared window at the page's scroll
    // offset, not the whole page slice; the count is bounded by the viewport.
    assert_eq!(
        state.editor.rules_scroll_offset_px,
        rule_scroll_offset_for_index(2)
    );
    assert_eq!(state.editor.rules_page, rules_window_page(&state));
    assert_eq!(
        state.diag.perf_snapshot.rules_visible_rows,
        rendered_rule_rows(&state)
    );
    assert!(
        state.diag.perf_snapshot.rules_visible_rows
            <= rendered_row_bound(state.editor.rules_viewport_px)
    );

    // Forward paging stops at the shared last page; a stale page clamps back.
    let _ = state.update(Message::RulesNextPage);
    assert_eq!(state.editor.rules_page, 2);
    let _ = state.update(Message::RulesNextPage);
    assert_eq!(state.editor.rules_page, 2);
    let _ = state.update(Message::RulesSetPage(99));
    assert_eq!(state.editor.rules_page, clamp_page(99, 5, 2));

    let _ = state.update(Message::FilterRules("site4".into()));
    assert_eq!(state.editor.rules_page, 0);
    assert_eq!(state.editor.rules_filtered_indices.len(), 1);
}
