use super::*;
use infiltrator_presentation::filter_query::FilterQuery;

#[test]
fn test_proxies_pinyin_fuzzy_and_protocol_filtering() {
    let node = ProxyNode {
        outbound_id: None,
        name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
        node_type: "VLESS".to_owned(),
        delay_ms: Some(45),
        health: OutboundHealthObservation::Unknown,
        selected: true,
        favorite: true,
        features: vec!["Reality".to_owned(), "Vision".to_owned()],
    };

    // The shared filter rule answers for the Bevy node: name, identity,
    // protocol, feature tag, the UDP capability, the latency bound and the
    // pinyin initials.
    assert!(FilterQuery::parse("xg").matches(&node));
    assert!(FilterQuery::parse("hk").matches(&node));
    assert!(FilterQuery::parse("vless").matches(&node));
    assert!(FilterQuery::parse("reality").matches(&node));
    assert!(FilterQuery::parse("<100").matches(&node));
    assert!(!FilterQuery::parse("日本").matches(&node));
}
