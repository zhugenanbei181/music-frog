//! test-intent: behavior
//! Each proxy group owns its own filter query; no group's query narrows
//! another group's members.

use crate::state::proxy_group_filters::{GroupFilter, ProxyGroupFilterState};
use infiltrator_presentation::filter_query::{FilterQuery, LatencyBound};

#[test]
fn a_group_starts_without_a_filter_and_a_filter_belongs_to_one_group_only() {
    let mut filters = ProxyGroupFilterState::default();
    assert_eq!(filters.filter("GLOBAL"), None);
    assert_eq!(filters.text("GLOBAL"), "");

    filters.open("GLOBAL");
    assert_eq!(
        filters.filter("GLOBAL"),
        Some(&GroupFilter::Open {
            text: String::new(),
            query: FilterQuery::MatchAll,
        })
    );
    assert_eq!(filters.filter("FALLBACK"), None);
    assert!(!filters.is_open("FALLBACK"));
}

#[test]
fn opening_twice_keeps_the_text_the_user_typed() {
    let mut filters = ProxyGroupFilterState::default();
    filters.open("GLOBAL");
    filters.set_text("GLOBAL", "HK".into());
    filters.open("GLOBAL");
    assert_eq!(filters.text("GLOBAL"), "HK");
    assert_eq!(
        filters.query("GLOBAL"),
        Some(&FilterQuery::Text("hk".to_owned()))
    );
}

#[test]
fn a_typed_query_is_parsed_once_and_owned_by_the_group() {
    let mut filters = ProxyGroupFilterState::default();
    filters.open("GLOBAL");
    filters.open("FALLBACK");
    filters.set_text("GLOBAL", "<80".into());
    filters.set_text("FALLBACK", "JP".into());
    assert_eq!(
        filters.query("GLOBAL"),
        Some(&FilterQuery::Latency(LatencyBound::FasterThan(80)))
    );
    assert_eq!(
        filters.query("FALLBACK"),
        Some(&FilterQuery::Text("jp".to_owned()))
    );
    // A group without an open filter cannot acquire a query.
    filters.set_text("RELAY", "anything".into());
    assert_eq!(filters.filter("RELAY"), None);
}

#[test]
fn closing_a_filter_forgets_its_query() {
    let mut filters = ProxyGroupFilterState::default();
    filters.open("GLOBAL");
    filters.set_text("GLOBAL", "HK".into());
    filters.close("GLOBAL");
    assert_eq!(filters.filter("GLOBAL"), None);
    assert_eq!(filters.query("GLOBAL"), None);
    filters.open("GLOBAL");
    assert_eq!(filters.text("GLOBAL"), "");
}
