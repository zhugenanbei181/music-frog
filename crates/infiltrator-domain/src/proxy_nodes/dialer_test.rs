//! DUAL-05-09/10 domain tests: static dialer chain resolution + loop detection.

use super::*;

const PROFILE: &str = r#"
proxies:
  - name: nas
    type: ss
    server: 1.1.1.1
    port: 8388
    cipher: aes-128-gcm
    password: pw
    dialer-proxy: gateway
  - name: gateway
    type: vless
    server: 2.2.2.2
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    dialer-proxy: hop
  - name: hop
    type: trojan
    server: 3.3.3.3
    port: 443
    password: pw
  - name: free
    type: ss
    server: 4.4.4.4
    port: 8388
    cipher: aes-128-gcm
    password: pw
proxy-groups:
  - name: PROXY
    type: select
    proxies: [nas, free]
  - name: relay-legacy
    type: relay
    proxies: [nas, free]
"#;

#[test]
fn dialer_chain_resolves_hops_in_order() {
    let facts = DialerGraphFacts::from_profile_yaml(PROFILE).expect("facts");
    let chain = DialerTopology::resolve_chain(&facts, "nas");
    assert_eq!(chain.root, "nas");
    let names: Vec<&str> = chain.hops.iter().map(|hop| hop.name.as_str()).collect();
    assert_eq!(names, vec!["nas", "gateway", "hop"]);
    assert_eq!(chain.hops[0].node_type, "ss");
    assert_eq!(chain.end, DialerChainEnd::Complete);
    assert!(chain.is_valid());
    assert!(!chain.is_loop());
}

#[test]
fn chain_stops_at_a_group_boundary_instead_of_guessing() {
    let profile = r#"
proxies:
  - name: nas
    type: ss
    server: 1.1.1.1
    port: 8388
    cipher: aes-128-gcm
    password: pw
    dialer-proxy: dial
  - name: hk
    type: ss
    server: 9.9.9.9
    port: 8388
    cipher: aes-128-gcm
    password: pw
  - name: jp
    type: ss
    server: 8.8.8.8
    port: 8388
    cipher: aes-128-gcm
    password: pw
proxy-groups:
  - name: dial
    type: select
    proxies: [hk, jp]
"#;
    let facts = DialerGraphFacts::from_profile_yaml(profile).expect("facts");
    let chain = DialerTopology::resolve_chain(&facts, "nas");
    assert_eq!(chain.end, DialerChainEnd::GroupBoundary("dial".to_string()));
    assert!(chain.is_valid());
    assert_eq!(chain.hops.len(), 2);
    assert!(chain.hops[1].is_group);
    assert_eq!(chain.hops[1].candidates, vec!["hk", "jp"]);
    // The group members are declared nodes, but the static chain must still
    // not pretend to know which one the runtime selects.
    assert_eq!(chain.hops[1].node_type, "select");
}

#[test]
fn missing_dialer_target_is_reported_not_hidden() {
    let profile = r#"
proxies:
  - name: nas
    type: ss
    server: 1.1.1.1
    port: 8388
    dialer-proxy: nowhere
"#;
    let facts = DialerGraphFacts::from_profile_yaml(profile).expect("facts");
    let chain = DialerTopology::resolve_chain(&facts, "nas");
    assert_eq!(
        chain.end,
        DialerChainEnd::MissingTarget("nowhere".to_string())
    );
    assert!(!chain.is_valid());
}

#[test]
fn self_loop_and_mutual_loop_are_detected_and_never_valid() {
    let self_loop = r#"
proxies:
  - name: a
    type: ss
    server: 1.1.1.1
    port: 8388
    dialer-proxy: a
"#;
    let facts = DialerGraphFacts::from_profile_yaml(self_loop).expect("facts");
    let cycles = DialerTopology::detect_cycles(&facts);
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].kind, DialerCycleKind::SelfLoop);
    assert!(cycles[0].spans_dialer_proxy);
    let chain = DialerTopology::resolve_chain(&facts, "a");
    assert!(chain.is_loop());
    assert!(!chain.is_valid());

    let mutual = r#"
proxies:
  - name: a
    type: ss
    server: 1.1.1.1
    port: 8388
    dialer-proxy: b
  - name: b
    type: ss
    server: 2.2.2.2
    port: 8388
    dialer-proxy: a
"#;
    let facts = DialerGraphFacts::from_profile_yaml(mutual).expect("facts");
    let cycles = DialerTopology::detect_cycles(&facts);
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].kind, DialerCycleKind::Mutual);
    assert_eq!(cycles[0].path, vec!["a", "b", "a"]);
    for node in ["a", "b"] {
        let chain = DialerTopology::resolve_chain(&facts, node);
        assert!(chain.is_loop(), "{node} must not render a valid chain");
        assert!(!chain.is_valid());
        assert!(matches!(chain.end, DialerChainEnd::Cycle(_)));
    }
}

#[test]
fn cycle_through_a_proxy_group_is_detected() {
    let profile = r#"
proxies:
  - name: landing
    type: ss
    server: 1.1.1.1
    port: 8388
    dialer-proxy: dial
proxy-groups:
  - name: dial
    type: select
    proxies: [landing]
"#;
    let facts = DialerGraphFacts::from_profile_yaml(profile).expect("facts");
    let cycles = DialerTopology::detect_cycles(&facts);
    assert_eq!(cycles.len(), 1);
    // Two hop cycle (node <-> group) that walks a dialer edge.
    assert_eq!(cycles[0].kind, DialerCycleKind::Mutual);
    assert!(cycles[0].spans_dialer_proxy);
    // A chain that loops through the group must not look valid either.
    let chain = DialerTopology::resolve_chain(&facts, "landing");
    assert!(chain.is_loop());
}

#[test]
fn proxy_group_member_cycles_reuse_the_group_semantics() {
    let profile = r#"
proxies:
  - name: n1
    type: ss
    server: 1.1.1.1
    port: 8388
proxy-groups:
  - name: g1
    type: select
    proxies: [g2]
  - name: g2
    type: select
    proxies: [g1]
"#;
    let facts = DialerGraphFacts::from_profile_yaml(profile).expect("facts");
    let cycles = DialerTopology::detect_cycles(&facts);
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].kind, DialerCycleKind::GroupCycle);
    assert!(!cycles[0].spans_dialer_proxy);
}

#[test]
fn relay_group_and_provider_only_groups_are_reported_as_warnings() {
    let facts = DialerGraphFacts::from_profile_yaml(PROFILE).expect("facts");
    let warnings = DialerTopology::group_warnings(&facts);
    assert!(
        warnings.iter().any(|line| line.contains("relay")),
        "{warnings:?}"
    );

    let provider_only = r#"
proxies:
  - name: n1
    type: ss
    server: 1.1.1.1
    port: 8388
proxy-groups:
  - name: dial
    type: select
    use: [provider1]
"#;
    let facts = DialerGraphFacts::from_profile_yaml(provider_only).expect("facts");
    let warnings = DialerTopology::group_warnings(&facts);
    assert!(
        warnings.iter().any(|line| line.contains("use:")),
        "{warnings:?}"
    );
}

#[test]
fn empty_graph_stays_empty_and_unknown_keys_are_preserved() {
    let facts = DialerGraphFacts::from_profile_yaml("proxies: []\n").expect("facts");
    assert!(facts.is_empty());
    assert!(DialerTopology::detect_cycles(&facts).is_empty());
    assert!(DialerTopology::resolve_all(&facts).is_empty());

    let with_extra = r#"
proxies:
  - name: nas
    type: custom-unknown
    server: 1.1.1.1
    port: 8388
    dialer-proxy: hop
    future-flag: 7
  - name: hop
    type: ss
    server: 2.2.2.2
    port: 8388
"#;
    let facts = DialerGraphFacts::from_profile_yaml(with_extra).expect("facts");
    assert_eq!(
        facts.node_types.get("nas").map(String::as_str),
        Some("custom-unknown")
    );
    let chain = DialerTopology::resolve_chain(&facts, "nas");
    assert_eq!(chain.hops[1].name, "hop");
    assert_eq!(chain.end, DialerChainEnd::Complete);
}
