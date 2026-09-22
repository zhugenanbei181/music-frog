//! Reactive view state and shared reductions for the Rules page (DUAL-11-13).
//!
//! The page scene lives in [`super::rules`]. This module owns the parts that
//! react to input: the keyword search field and the page cursor. Both the
//! match predicate and the page arithmetic delegate to
//! `infiltrator_domain::rules::view`, so the Bevy page and the Iced editor
//! filter and page identically.

use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Changed, QueryFilter, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_domain::rules::view::{self, RuleView};

use crate::pages::rules::{LastRulesProjection, RuleItem};

/// Marker on a rule row root; the payload is the row index into the last
/// projection so search and pagination can hide rows in place.
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

/// Shared page cursor. The search keyword is read live from the mounted field.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulesViewState {
    pub page: usize,
    pub page_size: usize,
}

impl Default for RulesViewState {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: view::DEFAULT_RULE_PAGE_SIZE,
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

/// DUAL-11-13: hide rows outside the active keyword filter and page, and
/// restamp the page indicator. Runs only when the page cursor changed or a
/// search field was edited.
pub(crate) fn sync_rules_view(
    search_fields: Query<&Children, With<RuleSearchField>>,
    text_fields: Query<&TextField, Changed<TextField>>,
    view: Option<Res<RulesViewState>>,
    last: Option<Res<LastRulesProjection>>,
    mut rows: Query<(&mut Node, &RuleRow)>,
    mut indicator: Query<&mut Text, With<RulesPageIndicator>>,
) {
    let Some(view) = view else {
        return;
    };
    if !view.is_changed() && text_fields.iter().next().is_none() {
        return;
    }
    let Some(projection) = last.and_then(|last| last.0.clone()) else {
        return;
    };
    let query = search_field_text(&search_fields, &text_fields).unwrap_or_default();
    let filtered = view::filter_rule_indices(&projection.rules, &query);
    let (start, end) = view::page_bounds(view.page, filtered.len(), view.page_size);
    let visible: std::collections::HashSet<usize> = filtered[start..end].iter().copied().collect();

    for (mut node, row) in &mut rows {
        node.display = if visible.contains(&row.0) {
            Display::Flex
        } else {
            Display::None
        };
    }

    let total_pages = view::page_count(filtered.len(), view.page_size);
    let want = format!(
        "第 {}/{} 页 · 共 {} 条",
        view.page + 1,
        total_pages,
        filtered.len()
    );
    for mut text in &mut indicator {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// Move the page cursor from the paging buttons, clamped to the filtered list.
pub(crate) fn on_rules_paging_activated(
    activate: On<Activate>,
    prev: Query<(), With<RulesPagePrevButton>>,
    next: Query<(), With<RulesPageNextButton>>,
    view: Option<ResMut<RulesViewState>>,
    last: Option<Res<LastRulesProjection>>,
    search_fields: Query<&Children, With<RuleSearchField>>,
    text_fields: Query<&TextField>,
) {
    let Some(mut view) = view else {
        return;
    };
    let Some(projection) = last.and_then(|last| last.0.clone()) else {
        return;
    };
    let query = search_field_text(&search_fields, &text_fields).unwrap_or_default();
    let len = view::filter_rule_indices(&projection.rules, &query).len();
    if prev.contains(activate.entity) {
        view.page = view.page.saturating_sub(1);
    } else if next.contains(activate.entity) {
        let total_pages = view::page_count(len, view.page_size);
        if view.page + 1 < total_pages {
            view.page += 1;
        }
    }
    view.page = view::clamp_page(view.page, len, view.page_size);
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
    }
}
