use super::*;
use infiltrator_presentation::proxies_page_model::ProxyFilterQuery;

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
    assert!(ProxyFilterQuery::parse("xg").matches(&node));
    assert!(ProxyFilterQuery::parse("hk").matches(&node));
    assert!(ProxyFilterQuery::parse("vless").matches(&node));
    assert!(ProxyFilterQuery::parse("reality").matches(&node));
    assert!(ProxyFilterQuery::parse("<100").matches(&node));
    assert!(!ProxyFilterQuery::parse("日本").matches(&node));
}
