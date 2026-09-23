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

/// DUAL-11-08: maximum number of rules the application publishes into surface
/// snapshots. The Iced editor loads the profile list through its own port and
/// is not capped; the published view is, and both surfaces render the cap and
/// the omitted count instead of presenting a truncated list as complete.
pub const RULE_PUBLISH_LIMIT: usize = 5000;

/// DUAL-11-08: number of rules published for a received list.
pub fn published_rule_count(received: usize) -> usize {
    received.min(RULE_PUBLISH_LIMIT)
}

/// DUAL-11-08: rules the publish cap drops from a received list.
pub fn omitted_rule_count(received: usize) -> usize {
    received.saturating_sub(RULE_PUBLISH_LIMIT)
}

/// DUAL-11-08: whether publishing a received list truncates it.
pub fn is_truncated_rule_list(received: usize) -> bool {
    received > RULE_PUBLISH_LIMIT
}

/// DUAL-11-08: fixed row height of one rule row in the virtual window
/// (logical pixels). The window arithmetic below only holds for uniform rows,
/// so both surfaces render every rule row at exactly this height.
pub const RULE_ROW_HEIGHT_PX: f32 = 56.0;

/// DUAL-11-08: extra rows rendered above and below the visible band so a fast
/// scroll never exposes a blank strip while the window catches up.
pub const RULE_WINDOW_OVERSCAN: usize = 4;

/// DUAL-11-08: viewport height assumed before a surface has measured its real
/// list viewport. A declared fallback, never a fabricated measurement.
pub const RULE_DEFAULT_VIEWPORT_PX: f32 = 560.0;

/// DUAL-11-08: the fixed row height is also the scroll quantum: a rule at
/// filtered position `index` sits at `index * RULE_ROW_HEIGHT_PX`.
pub fn rule_scroll_offset_for_index(index: usize) -> f32 {
    index as f32 * RULE_ROW_HEIGHT_PX
}

/// DUAL-11-08: the filtered position a scroll offset lands on. Used by both
/// surfaces to report which page the viewport currently shows.
pub fn rule_index_at_scroll_offset(offset_px: f32) -> usize {
    if !offset_px.is_finite() || offset_px <= 0.0 {
        return 0;
    }
    (offset_px / RULE_ROW_HEIGHT_PX).floor() as usize
}

/// DUAL-11-08: the page a filtered position belongs to, so the paging
/// indicator follows the scroll-driven window instead of a second cursor.
pub fn page_for_rule_index(index: usize, page_size: usize) -> usize {
    index / effective_page_size(page_size)
}

/// DUAL-11-08: rows in the visible band, including the partially visible row
/// at each edge. Depends only on the viewport height, never on the list
/// length — this is what makes the render bound O(1).
pub fn visible_rule_rows(viewport_height_px: f32) -> usize {
    let viewport = if viewport_height_px.is_finite() && viewport_height_px > 0.0 {
        viewport_height_px
    } else {
        RULE_DEFAULT_VIEWPORT_PX
    };
    (viewport / RULE_ROW_HEIGHT_PX).ceil() as usize + 1
}

/// DUAL-11-08: the hard upper bound on rule rows any surface may render for
/// one window, whatever the list length is (50,000+ included).
pub fn rendered_row_bound(viewport_height_px: f32) -> usize {
    visible_rule_rows(viewport_height_px) + RULE_WINDOW_OVERSCAN * 2
}

/// DUAL-11-08: the slice of a filtered rule list a surface renders for the
/// current scroll position, plus the spacer heights that keep the scrollable
/// content the full `total * row_height` tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleWindow {
    /// First rendered filtered position.
    pub start: usize,
    /// One past the last rendered filtered position.
    pub end: usize,
    /// Filtered rows behind the window (top spacer height).
    pub top_spacer_px: f32,
    /// Filtered rows ahead of the window (bottom spacer height).
    pub bottom_spacer_px: f32,
    /// Full scrollable content height of the filtered list.
    pub content_height_px: f32,
    /// Total filtered rows the spacers + window cover.
    pub total: usize,
}

impl RuleWindow {
    /// Number of rule rows a surface renders for this window.
    pub fn rendered_rows(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether a filtered position is inside the rendered window.
    pub fn contains(&self, index: usize) -> bool {
        index >= self.start && index < self.end
    }

    /// Empty window over an empty list.
    pub fn is_empty(&self) -> bool {
        self.rendered_rows() == 0
    }
}

/// DUAL-11-08: O(1) geometric clipping over a filtered rule list. The window
/// size is bounded by [`rendered_row_bound`] for any `total`; scrolling only
/// shifts `start`/`end` and the spacers, so no surface ever builds a row per
/// entry of a 50,000-entry list.
pub fn rule_window(scroll_offset_px: f32, viewport_height_px: f32, total: usize) -> RuleWindow {
    let content_height_px = total as f32 * RULE_ROW_HEIGHT_PX;
    if total == 0 {
        return RuleWindow {
            start: 0,
            end: 0,
            top_spacer_px: 0.0,
            bottom_spacer_px: 0.0,
            content_height_px: 0.0,
            total: 0,
        };
    }
    let offset = if scroll_offset_px.is_finite() {
        scroll_offset_px.clamp(0.0, content_height_px)
    } else {
        0.0
    };
    let first_visible = rule_index_at_scroll_offset(offset).min(total - 1);
    let start = first_visible.saturating_sub(RULE_WINDOW_OVERSCAN);
    let end = first_visible
        .saturating_add(visible_rule_rows(viewport_height_px))
        .saturating_add(RULE_WINDOW_OVERSCAN)
        .min(total);
    RuleWindow {
        start,
        end,
        top_spacer_px: start as f32 * RULE_ROW_HEIGHT_PX,
        bottom_spacer_px: (total - end) as f32 * RULE_ROW_HEIGHT_PX,
        content_height_px,
        total,
    }
}

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

/// DUAL-11-05: human-readable form of a provider's declared refresh interval
/// (seconds). Shared so both surfaces render the same schedule.
pub fn format_refresh_interval(secs: u64) -> String {
    if secs == 0 {
        return "0s".to_owned();
    }
    if secs.is_multiple_of(86_400) {
        format!("{}d", secs / 86_400)
    } else if secs.is_multiple_of(3_600) {
        format!("{}h", secs / 3_600)
    } else if secs.is_multiple_of(60) {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// DUAL-11-05: neutral body of a provider's local cache-content fingerprint —
/// the short digest, the real byte size and the file's last-modified time when
/// the host could read one. This is a local file observation, never an HTTP
/// validator; callers label it as such.
pub fn format_content_fingerprint(
    sha256: &str,
    size_bytes: u64,
    modified_unix_secs: Option<i64>,
) -> String {
    let digest: String = sha256.chars().take(12).collect();
    let mut parts = vec![format!("sha256:{digest}…"), format!("{size_bytes} B")];
    if let Some(secs) = modified_unix_secs
        && let Some(utc) = chrono::DateTime::from_timestamp(secs, 0)
    {
        parts.push(format!("mtime {}", utc.format("%Y-%m-%d %H:%M:%S UTC")));
    }
    parts.join(" · ")
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

    #[test]
    fn publish_cap_arithmetic_is_shared() {
        assert_eq!(RULE_PUBLISH_LIMIT, 5000);
        assert_eq!(published_rule_count(4_999), 4_999);
        assert_eq!(published_rule_count(5_001), 5_000);
        assert_eq!(omitted_rule_count(5_001), 1);
        assert_eq!(omitted_rule_count(120), 0);
        assert!(!is_truncated_rule_list(5_000));
        assert!(is_truncated_rule_list(50_000));
    }

    #[test]
    fn virtual_window_is_bounded_and_slides_with_the_scroll_offset() {
        let total = 50_000;
        let viewport = RULE_DEFAULT_VIEWPORT_PX;
        let bound = rendered_row_bound(viewport);

        let top = rule_window(0.0, viewport, total);
        assert_eq!(top.start, 0);
        assert_eq!(
            top.rendered_rows(),
            visible_rule_rows(viewport) + RULE_WINDOW_OVERSCAN,
            "at the top there is no overscan above, only below"
        );
        assert!(top.rendered_rows() <= bound);
        assert_eq!(top.top_spacer_px, 0.0);
        assert_eq!(
            top.bottom_spacer_px,
            (total - top.end) as f32 * RULE_ROW_HEIGHT_PX
        );
        assert_eq!(top.content_height_px, total as f32 * RULE_ROW_HEIGHT_PX);
        assert!(top.contains(0));
        assert!(!top.contains(top.end));

        // Middle of the list: the window follows the offset, never grows.
        let middle = rule_window(rule_scroll_offset_for_index(25_000), viewport, total);
        assert_eq!(
            middle.start + RULE_WINDOW_OVERSCAN,
            25_000,
            "the first visible row is the offset's row, overscan above"
        );
        assert!(middle.rendered_rows() <= bound);
        assert!(middle.contains(25_000));
        assert!(!middle.contains(middle.start - 1));
        assert_eq!(
            middle.top_spacer_px,
            middle.start as f32 * RULE_ROW_HEIGHT_PX
        );
        assert_eq!(
            middle.top_spacer_px
                + middle.rendered_rows() as f32 * RULE_ROW_HEIGHT_PX
                + middle.bottom_spacer_px,
            middle.content_height_px,
            "spacers + rendered rows always cover the whole list"
        );

        // Scrolled past the end: clamped onto the last rows, still bounded.
        let bottom = rule_window(1.0e9, viewport, total);
        assert!(bottom.end <= total);
        assert!(bottom.rendered_rows() <= bound);
        assert!(bottom.contains(total - 1));
        assert_eq!(bottom.bottom_spacer_px, 0.0);

        // Small lists render every row and never windows past the list.
        let small = rule_window(10_000.0, viewport, 3);
        assert_eq!((small.start, small.end), (0, 3));
        assert_eq!(small.bottom_spacer_px, 0.0);

        // Degenerate inputs stay total.
        let empty = rule_window(-4.0, 0.0, 0);
        assert!(empty.is_empty());
        assert_eq!(empty.content_height_px, 0.0);
        let nan = rule_window(f32::NAN, f32::NAN, 100);
        assert_eq!(nan.start, 0);
        assert!(nan.rendered_rows() <= bound);
    }

    #[test]
    fn window_index_and_page_arithmetic_follow_the_scroll_offset() {
        assert_eq!(rule_scroll_offset_for_index(0), 0.0);
        assert_eq!(
            rule_scroll_offset_for_index(120),
            120.0 * RULE_ROW_HEIGHT_PX
        );
        assert_eq!(rule_index_at_scroll_offset(0.0), 0);
        assert_eq!(rule_index_at_scroll_offset(-1.0), 0);
        assert_eq!(rule_index_at_scroll_offset(f32::NAN), 0);
        assert_eq!(
            rule_index_at_scroll_offset(rule_scroll_offset_for_index(7) + 1.0),
            7
        );
        assert_eq!(page_for_rule_index(0, 200), 0);
        assert_eq!(page_for_rule_index(199, 200), 0);
        assert_eq!(page_for_rule_index(200, 200), 1);
        assert_eq!(page_for_rule_index(50_000, 0), 250);
    }

    #[test]
    fn refresh_interval_formats_in_the_largest_whole_unit() {
        assert_eq!(format_refresh_interval(0), "0s");
        assert_eq!(format_refresh_interval(45), "45s");
        assert_eq!(format_refresh_interval(1_800), "30m");
        assert_eq!(format_refresh_interval(3_600), "1h");
        assert_eq!(format_refresh_interval(86_400), "1d");
        assert_eq!(format_refresh_interval(90_000), "25h");
    }

    #[test]
    fn content_fingerprint_formats_digest_size_and_mtime() {
        assert_eq!(
            format_content_fingerprint("abcdef0123456789deadbeef", 4_096, Some(0)),
            "sha256:abcdef012345… · 4096 B · mtime 1970-01-01 00:00:00 UTC"
        );
        // A host without a readable timestamp publishes two real facts, not a
        // placeholder date.
        assert_eq!(
            format_content_fingerprint("ab", 12, None),
            "sha256:ab… · 12 B"
        );
    }
}
