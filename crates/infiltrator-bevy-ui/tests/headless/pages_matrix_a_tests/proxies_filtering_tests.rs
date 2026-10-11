use super::*;

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

    assert!(matches_proxy_filter(&node, "xg"));
    assert!(matches_proxy_filter(&node, "hk"));
    assert!(matches_proxy_filter(&node, "vless"));
    assert!(matches_proxy_filter(&node, "reality"));
    assert!(matches_proxy_filter(&node, "<100"));
    assert!(!matches_proxy_filter(&node, "日本"));
}
