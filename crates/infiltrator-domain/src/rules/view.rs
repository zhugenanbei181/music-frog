//! Shared rule-list view reductions (keyword search + pagination) consumed by
//! both surfaces (DUAL-11-13).
//!
//! The Iced editor cache and the Bevy projection are different in-memory
//! shapes, so the reduction is expressed over the [`RuleView`] seam: each
//! surface adapts its own row to the shared predicate instead of re-deriving
//! the match or page arithmetic locally.

use super::RuleEntry;

/// One rule row as seen by the shared search/pagination reduction.
pub trait RuleView {
    /// Searchable representation of the rule: the raw `TYPE,PAYLOAD,TARGET`
    /// expression (or logical form) for the editor, a recombined expression for
    /// projections that store the parts separately.
    fn view_rule_search(&self) -> String;
}

impl RuleView for RuleEntry {
    fn view_rule_search(&self) -> String {
        self.rule.clone()
    }
}

/// Default page size shared by both surfaces when a view has none set.
pub const DEFAULT_RULE_PAGE_SIZE: usize = 200;

/// Case-insensitive substring match over the rule expression. An empty query
/// matches every row.
pub fn matches_rule_search<T: RuleView>(item: &T, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }
    item.view_rule_search()
        .to_ascii_lowercase()
        .contains(&query)
}

/// Indices into `rules` that survive the keyword filter, in list order.
pub fn filter_rule_indices<T: RuleView>(rules: &[T], query: &str) -> Vec<usize> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_rule_search(*item, query))
        .map(|(index, _)| index)
        .collect()
}

/// Effective page size, falling back to [`DEFAULT_RULE_PAGE_SIZE`].
pub fn effective_page_size(page_size: usize) -> usize {
    if page_size == 0 {
        DEFAULT_RULE_PAGE_SIZE
    } else {
        page_size
    }
}

/// Number of pages needed for `len` filtered rows; an empty list still has one
/// (empty) page so paging controls never divide by zero.
pub fn page_count(len: usize, page_size: usize) -> usize {
    let page_size = effective_page_size(page_size);
    if len == 0 {
        1
    } else {
        (len - 1) / page_size + 1
    }
}

/// Clamp `page` into the valid range for the current filtered length.
pub fn clamp_page(page: usize, len: usize, page_size: usize) -> usize {
    page.min(page_count(len, page_size).saturating_sub(1))
}

/// Half-open `[start, end)` slice bounds of `page` over `len` filtered rows.
pub fn page_bounds(page: usize, len: usize, page_size: usize) -> (usize, usize) {
    let page_size = effective_page_size(page_size);
    let page = clamp_page(page, len, page_size);
    let start = page.saturating_mul(page_size);
    let end = start.saturating_add(page_size).min(len);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(rule: &str) -> RuleEntry {
        RuleEntry {
            rule: rule.to_owned(),
            enabled: true,
        }
    }

    #[test]
    fn search_is_case_insensitive_and_matches_payload_and_target() {
        let rules = vec![
            entry("DOMAIN-SUFFIX,github.com,PROXY"),
            entry("GEOIP,CN,DIRECT"),
            entry("DOMAIN-KEYWORD,google,GLOBAL"),
        ];
        assert_eq!(filter_rule_indices(&rules, "github"), vec![0]);
        assert_eq!(filter_rule_indices(&rules, "GOOGLE"), vec![2]);
        assert_eq!(filter_rule_indices(&rules, "direct"), vec![1]);
        assert_eq!(filter_rule_indices(&rules, "  "), vec![0, 1, 2]);
    }

    #[test]
    fn pagination_clamps_and_reports_bounds() {
        assert_eq!(page_count(0, 0), 1);
        assert_eq!(page_count(5, 2), 3);
        assert_eq!(clamp_page(9, 5, 2), 2);
        assert_eq!(page_bounds(0, 5, 2), (0, 2));
        assert_eq!(page_bounds(2, 5, 2), (4, 5));
        // A stale page past the end clamps back onto the last page.
        assert_eq!(page_bounds(9, 5, 2), (4, 5));
        assert_eq!(page_bounds(0, 0, 2), (0, 0));
    }
}
