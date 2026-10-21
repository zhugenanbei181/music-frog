use crate::filter_query::{FilterQuery, LatencyBound};

#[test]
fn an_empty_query_matches_every_row() {
    assert_eq!(FilterQuery::parse(""), FilterQuery::MatchAll);
    assert_eq!(FilterQuery::parse("   "), FilterQuery::MatchAll);
    assert_eq!(FilterQuery::parse_text(""), FilterQuery::MatchAll);
    assert!(FilterQuery::parse("").matches_all());
    assert_eq!(FilterQuery::parse("").text(), "");
}

#[test]
fn a_latency_bound_is_only_a_bound_for_a_list_that_carries_a_latency() {
    assert_eq!(
        FilterQuery::parse("<100"),
        FilterQuery::Latency(LatencyBound::FasterThan(100))
    );
    assert_eq!(
        FilterQuery::parse(" > 250 "),
        FilterQuery::Latency(LatencyBound::SlowerThan(250))
    );
    assert_eq!(
        FilterQuery::parse("<100").latency_bound(),
        Some(LatencyBound::FasterThan(100))
    );

    // A list without a latency never produces the bound: the query stays
    // free text instead of emptying the list.
    assert_eq!(
        FilterQuery::parse_text("<100"),
        FilterQuery::Text("<100".to_owned())
    );
    assert_eq!(FilterQuery::parse_text("<100").latency_bound(), None);
}

#[test]
fn a_free_text_query_is_normalized_the_same_way_for_every_page() {
    assert_eq!(
        FilterQuery::parse("<abc"),
        FilterQuery::Text("<abc".to_owned())
    );
    assert_eq!(
        FilterQuery::parse("Hong Kong"),
        FilterQuery::Text("hong kong".to_owned())
    );
    assert_eq!(
        FilterQuery::parse_text("Hong Kong"),
        FilterQuery::Text("hong kong".to_owned())
    );
    assert_eq!(
        FilterQuery::parse_text("Hong Kong").text(),
        "hong kong",
        "the text form is the normalized query every page matches with"
    );
}
