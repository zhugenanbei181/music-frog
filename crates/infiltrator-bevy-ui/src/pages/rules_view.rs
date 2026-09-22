//! Reactive view state and shared reductions for the Rules page (DUAL-11-13),
//! including the DUAL-11-08 virtual window that renders the rule list.
//!
//! The page scene lives in [`super::rules`]. This module owns the parts that
//! react to input: the keyword search field, the page cursor and the scroll
//! offset of the rule-list viewport. The match predicate, the page arithmetic
//! and the render window all delegate to `infiltrator_domain::rules::view`, so
//! the Bevy page and the Iced editor filter, page and clip identically.
//!
//! **DUAL-11-08**: only the rows inside the shared window exist as entities.
//! The window is derived from the list viewport's scroll offset and measured
//! height, so a 50,000-rule projection mounts the same bounded row set as a
//! five-rule one; scrolling shifts the window instead of moving every row.

use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{Changed, QueryFilter, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{ComputedNode, Node, ScrollPosition, percent, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_domain::rules::view::{self, RuleView};

use crate::pages::rules::{LastRulesProjection, RuleItem, RulesProjection, rule_row_scene};

/// Marker on a rule row root; the payload is the row index into the last
/// projection. Rows outside the shared window are not spawned at all.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleRow(pub usize);

/// Marker on the wrapper of the keyword search text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleSearchField;

/// Marker on the previous-page button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesPagePrevButton;

/// Marker on the next-page button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesPageNextButton;

/// Marker on the page indicator caption.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesPageIndicator;

/// DUAL-11-08: the clipped scroll area that owns the rule-list viewport. Its
/// `ScrollPosition` (driven by the shared scroll-area widget on real input)
/// is the single source of the render window.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesListScrollArea;

/// DUAL-11-08: the container whose children are rebuilt to the current window
/// (top spacer, rendered rows, bottom spacer).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesWindowRows;

/// Shared view state. The keyword is read live from the mounted field; the
/// scroll offset is read from the list viewport.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct RulesViewState {
    pub page: usize,
    pub page_size: usize,
    /// Last observed scroll offset of the rule-list viewport.
    pub scroll_offset_px: f32,
    /// Measured viewport height, or the shared declared fallback until the
    /// layout reports one.
    pub viewport_height_px: f32,
    /// Projection indices surviving the shared keyword filter, in list order.
    pub filtered_indices: Vec<usize>,
    /// Bumped every time a new projection replaces the last one.
    pub projection_generation: u64,
    /// `(keyword, projection generation)` the filtered list belongs to; the
    /// shared filter reduction only re-runs when this no longer matches.
    pub filtered_for: Option<(String, u64)>,
    /// Bumped whenever `filtered_indices` is recomputed.
    pub filter_generation: u64,
    /// `filter_generation` the mounted rows were built from.
    pub rendered_generation: u64,
    /// A keyword change reset the view offset; the viewport must follow it
    /// before the next window is mounted.
    pub pending_viewport_reset: bool,
    /// `(start, end)` filtered positions the mounted rows cover.
    pub rendered_window: Option<(usize, usize)>,
    /// Number of rule row entities mounted for the current window.
    pub rendered_rows: usize,
}

impl Default for RulesViewState {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: view::DEFAULT_RULE_PAGE_SIZE,
            scroll_offset_px: 0.0,
            viewport_height_px: view::RULE_DEFAULT_VIEWPORT_PX,
            filtered_indices: Vec::new(),
            projection_generation: 0,
            filtered_for: None,
            filter_generation: 0,
            rendered_generation: 0,
            pending_viewport_reset: false,
            rendered_window: None,
            rendered_rows: 0,
        }
    }
}

/// The Bevy projection row satisfies the shared reduction seam. The projection
/// stores the rule split into type/payload/proxy, so the searchable form is the
/// recombined expression.
impl RuleView for RuleItem {
    fn view_rule_search(&self) -> String {
        if self.payload.is_empty() {
            format!("{},{}", self.rule_type, self.proxy)
        } else {
            format!("{},{},{}", self.rule_type, self.payload, self.proxy)
        }
    }
}

/// DUAL-11-08: the shared render window for the current state.
pub(crate) fn rules_window(view: &RulesViewState) -> view::RuleWindow {
    view::rule_window(
        view.scroll_offset_px,
        view.viewport_height_px,
        view.filtered_indices.len(),
    )
}

/// Projection indices the window renders, resolved against the filtered list.
/// This is exactly the entity set [`rebuild_rules_window`] mounts.
pub fn visible_projection_rows(view: &RulesViewState) -> Vec<usize> {
    let window = rules_window(view);
    view.filtered_indices
        .get(window.start..window.end)
        .unwrap_or_default()
        .to_vec()
}

/// Read the live keyword from the mounted search field, if any.
pub(crate) fn search_field_text<F: QueryFilter>(
    search_fields: &Query<&Children, With<RuleSearchField>>,
    text_fields: &Query<&TextField, F>,
) -> Option<String> {
    search_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

/// Recompute the shared filter result and keep the cursor inside it. A no-op
/// unless the keyword or the projection generation actually changed.
fn refilter(view: &mut RulesViewState, projection: &RulesProjection, query: &str) {
    let query = query.trim().to_owned();
    let filtered_for = (query.clone(), view.projection_generation);
    if view.filtered_for.as_ref() == Some(&filtered_for) {
        return;
    }
    // A changed keyword is a new result set and returns to the top; a live
    // projection refresh keeps the user's scroll position (only the rows the
    // window covers are rebuilt).
    let keyword_changed = view
        .filtered_for
        .as_ref()
        .is_none_or(|(previous, _)| previous != &query);
    view.filtered_indices = view::filter_rule_indices(&projection.rules, &query);
    view.filter_generation = view.filter_generation.wrapping_add(1);
    view.page = view::clamp_page(view.page, view.filtered_indices.len(), view.page_size);
    if keyword_changed {
        view.scroll_offset_px = 0.0;
        view.pending_viewport_reset = true;
    }
    view.filtered_for = Some(filtered_for);
}

/// DUAL-11-13: react to a new projection or an edited keyword by recomputing
/// the shared filter result. The window rebuild is owned by
/// [`sync_rules_window`], which the route chains after this system.
pub(crate) fn sync_rules_view(
    search_fields: Query<&Children, With<RuleSearchField>>,
    text_fields: Query<&TextField, Changed<TextField>>,
    view: Option<ResMut<RulesViewState>>,
    last: Option<Res<LastRulesProjection>>,
) {
    let Some(mut view) = view else {
        return;
    };
    let projection_changed = last.as_ref().is_some_and(|last| last.is_changed());
    if !projection_changed && text_fields.iter().next().is_none() {
        return;
    }
    let Some(projection) = last.and_then(|last| last.0.clone()) else {
        return;
    };
    if projection_changed {
        view.projection_generation = view.projection_generation.wrapping_add(1);
    }
    let query = search_field_text(&search_fields, &text_fields).unwrap_or_default();
    refilter(&mut view, &projection, &query);
}

/// DUAL-11-08: mount exactly the shared window of the filtered rule list into
/// the rows container, and report the page the viewport is on.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_rules_window(
    view: Option<ResMut<RulesViewState>>,
    last: Option<Res<LastRulesProjection>>,
    search_fields: Query<&Children, With<RuleSearchField>>,
    text_fields: Query<&TextField>,
    mut scroll_areas: Query<
        (&mut ScrollPosition, Option<&ComputedNode>),
        With<RulesListScrollArea>,
    >,
    containers: Query<Entity, With<RulesWindowRows>>,
    palette: Res<UiPalette>,
    mut indicator: Query<&mut Text, With<RulesPageIndicator>>,
    mut commands: Commands,
) {
    let Some(mut view) = view else {
        return;
    };
    let Some(projection) = last.and_then(|last| last.0.clone()) else {
        return;
    };
    // The filter is a shared reduction; recompute it here as well so a scroll
    // that arrives before the filter system still renders the right rows.
    let query = search_field_text(&search_fields, &text_fields).unwrap_or_default();
    refilter(&mut view, &projection, &query);

    if let Some((mut position, computed)) = scroll_areas.iter_mut().next() {
        if view.pending_viewport_reset {
            // The keyword change owns the offset this frame; the viewport
            // follows the window instead of re-adopting a stale scroll.
            position.0.y = view.scroll_offset_px;
        } else {
            view.scroll_offset_px = position.0.y.max(0.0);
        }
        let measured = computed
            .map(|node| node.size().y * node.inverse_scale_factor())
            .filter(|height| *height > 0.0);
        if let Some(height) = measured {
            view.viewport_height_px = height;
        }
    }
    view.pending_viewport_reset = false;
    let content_height = view::rule_scroll_offset_for_index(view.filtered_indices.len());
    if view.scroll_offset_px > content_height {
        view.scroll_offset_px = content_height;
    }

    let window = rules_window(&view);
    let stale = view.rendered_generation != view.filter_generation
        || view.rendered_window != Some((window.start, window.end));
    if stale {
        view.rendered_generation = view.filter_generation;
        view.rendered_window = Some((window.start, window.end));
        view.rendered_rows = window.rendered_rows();
        // The viewport moves with the window, so the mounted rows and the
        // scroll position can never disagree.
        for (mut position, _) in &mut scroll_areas {
            if (position.0.y - view.scroll_offset_px).abs() > 0.5 {
                position.0.y = view.scroll_offset_px;
            }
        }
        // Exactly the projection rows the shared window covers — the same
        // list the mounted-entity test counts.
        let visible = visible_projection_rows(&view);
        for container in &containers {
            rebuild_rules_window(
                &mut commands,
                container,
                &projection,
                &visible,
                &window,
                &palette,
            );
        }
    }

    // The page caption follows the scroll offset, so the paging controls and
    // the mounted rows can never disagree about where the viewport is.
    let total = view.filtered_indices.len();
    let page = if total == 0 {
        0
    } else {
        view::page_for_rule_index(
            view::rule_index_at_scroll_offset(view.scroll_offset_px).min(total - 1),
            view.page_size,
        )
    };
    let want = if total == 0 {
        "第 1/1 页 · 共 0 条".to_owned()
    } else {
        format!(
            "第 {}/{} 页 · 显示 {}–{} · 共 {} 条",
            page + 1,
            view::page_count(total, view.page_size),
            window.start + 1,
            window.end,
            total
        )
    };
    for mut text in &mut indicator {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// Replace the mounted rows with the current window: a top spacer of the rows
/// above it, one row per visible projection index, and a bottom spacer so the
/// scrollable content stays exactly `total * row_height` tall.
pub(crate) fn rebuild_rules_window(
    commands: &mut Commands<'_, '_>,
    container: Entity,
    projection: &RulesProjection,
    visible_rows: &[usize],
    window: &view::RuleWindow,
    palette: &UiPalette,
) {
    commands.entity(container).despawn_children();
    if window.is_empty() {
        return;
    }
    if window.top_spacer_px > 0.0 {
        commands
            .spawn_scene(spacer_scene(window.top_spacer_px))
            .insert(ChildOf(container));
    }
    for source_index in visible_rows {
        let Some(rule) = projection.rules.get(*source_index) else {
            continue;
        };
        let scene = Box::new(rule_row_scene(*source_index, rule, palette)) as Box<dyn Scene>;
        commands.spawn_scene(scene).insert(ChildOf(container));
    }
    if window.bottom_spacer_px > 0.0 {
        commands
            .spawn_scene(spacer_scene(window.bottom_spacer_px))
            .insert(ChildOf(container));
    }
}

/// One flat spacer node of exactly `height` logical pixels.
fn spacer_scene(height: f32) -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: px(height),
        }
    }
}

/// Move the page cursor from the paging buttons: the page decides the scroll
/// offset, and the window follows it through the shared reduction.
pub(crate) fn on_rules_paging_activated(
    activate: On<Activate>,
    prev: Query<(), With<RulesPagePrevButton>>,
    next: Query<(), With<RulesPageNextButton>>,
    view: Option<ResMut<RulesViewState>>,
    scroll_areas: Query<&mut ScrollPosition, With<RulesListScrollArea>>,
) {
    let Some(mut view) = view else {
        return;
    };
    let total = view.filtered_indices.len();
    if prev.contains(activate.entity) {
        view.page = view.page.saturating_sub(1);
    } else if next.contains(activate.entity) {
        let total_pages = view::page_count(total, view.page_size);
        if view.page + 1 < total_pages {
            view.page += 1;
        }
    } else {
        return;
    }
    view.page = view::clamp_page(view.page, total, view.page_size);
    let (start, _) = view::page_bounds(view.page, total, view.page_size);
    let offset = view::rule_scroll_offset_for_index(start);
    view.scroll_offset_px = offset;
    for mut position in scroll_areas {
        position.0.y = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(rule_type: &str, payload: &str, proxy: &str) -> RuleItem {
        RuleItem {
            id: 1,
            rule_type: rule_type.to_owned(),
            payload: payload.to_owned(),
            proxy: proxy.to_owned(),
            hit_count: 0,
            is_enabled: true,
            last_hit_secs: None,
            is_shadowed: false,
            shadow_reason: None,
        }
    }

    fn state_with(total: usize) -> RulesViewState {
        RulesViewState {
            filtered_indices: (0..total).collect(),
            ..RulesViewState::default()
        }
    }

    #[test]
    fn projection_row_reuses_shared_search() {
        let row = item("DOMAIN-SUFFIX", "github.com", "PROXY");
        assert!(view::matches_rule_search(&row, "github"));
        assert!(view::matches_rule_search(&row, "PROXY"));
        assert!(!view::matches_rule_search(&row, "cloudflare"));
        let rows = vec![row, item("GEOIP", "CN", "DIRECT")];
        assert_eq!(view::filter_rule_indices(&rows, "direct"), vec![1]);
        assert_eq!(view::filter_rule_indices(&rows, ""), vec![0, 1]);
    }

    #[test]
    fn default_page_size_matches_shared_constant() {
        assert_eq!(
            RulesViewState::default().page_size,
            view::DEFAULT_RULE_PAGE_SIZE
        );
        assert_eq!(
            RulesViewState::default().viewport_height_px,
            view::RULE_DEFAULT_VIEWPORT_PX,
            "the declared fallback viewport is the shared constant"
        );
    }

    #[test]
    fn window_is_bounded_for_a_50k_list_and_slides_with_the_offset() {
        let mut view = state_with(50_000);
        let bound = view::rendered_row_bound(view.viewport_height_px);

        let top = visible_projection_rows(&view);
        assert_eq!(top.len(), rules_window(&view).rendered_rows());
        assert!(top.len() <= bound);
        assert_eq!(top.first().copied(), Some(0));

        view.scroll_offset_px = view::rule_scroll_offset_for_index(25_000);
        let middle = visible_projection_rows(&view);
        assert!(middle.len() <= bound);
        assert!(middle.contains(&25_000));
        assert_eq!(
            middle.first().copied(),
            Some(25_000 - view::RULE_WINDOW_OVERSCAN)
        );

        view.scroll_offset_px = 1.0e9;
        let bottom = visible_projection_rows(&view);
        assert!(bottom.len() <= bound);
        assert_eq!(bottom.last().copied(), Some(49_999));

        // An empty filtered list mounts no rows at all.
        view.filtered_indices.clear();
        view.scroll_offset_px = 0.0;
        assert!(visible_projection_rows(&view).is_empty());
    }
}
