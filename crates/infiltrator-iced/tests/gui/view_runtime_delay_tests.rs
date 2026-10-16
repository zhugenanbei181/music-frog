use super::*;
use infiltrator_contract::runtime_settings::RuntimeDelaySortOrder;

#[test]
fn test_sort_delay_nodes() {
    let mut nodes: Vec<DelayNodeItem> = vec![
        ("Node B".to_string(), "Shadowsocks".to_string(), Some(150)),
        ("Node A".to_string(), "VMess".to_string(), Some(50)),
        ("Node C".to_string(), "Trojan".to_string(), None),
        ("Node D".to_string(), "Hysteria2".to_string(), Some(300)),
    ];

    sort_delay_nodes(&mut nodes, RuntimeDelaySortOrder::LatencyAscending);
    assert_eq!(nodes[0].0, "Node A");
    assert_eq!(nodes[1].0, "Node B");
    assert_eq!(nodes[2].0, "Node D");
    assert_eq!(nodes[3].0, "Node C");

    sort_delay_nodes(&mut nodes, RuntimeDelaySortOrder::LatencyDescending);
    assert_eq!(nodes[0].0, "Node D");
    assert_eq!(nodes[1].0, "Node B");
    assert_eq!(nodes[2].0, "Node A");
    assert_eq!(nodes[3].0, "Node C");

    sort_delay_nodes(&mut nodes, RuntimeDelaySortOrder::NameAscending);
    assert_eq!(nodes[0].0, "Node A");
    assert_eq!(nodes[1].0, "Node B");
    assert_eq!(nodes[2].0, "Node C");
    assert_eq!(nodes[3].0, "Node D");

    sort_delay_nodes(&mut nodes, RuntimeDelaySortOrder::NameDescending);
    assert_eq!(nodes[0].0, "Node D");
    assert_eq!(nodes[1].0, "Node C");
    assert_eq!(nodes[2].0, "Node B");
    assert_eq!(nodes[3].0, "Node A");
}

#[test]
fn equal_delay_and_name_keys_keep_the_original_order() {
    let mut nodes: Vec<DelayNodeItem> = vec![
        ("same".into(), "first measured sample".into(), Some(12)),
        ("same".into(), "second measured sample".into(), Some(12)),
        ("same".into(), "first unavailable sample".into(), None),
        ("same".into(), "second unavailable sample".into(), None),
    ];

    sort_delay_nodes(&mut nodes, RuntimeDelaySortOrder::LatencyAscending);
    assert_eq!(nodes[0].1, "first measured sample");
    assert_eq!(nodes[1].1, "second measured sample");
    assert_eq!(nodes[2].1, "first unavailable sample");
    assert_eq!(nodes[3].1, "second unavailable sample");

    sort_delay_nodes(&mut nodes, RuntimeDelaySortOrder::NameAscending);
    assert_eq!(nodes[0].1, "first measured sample");
    assert_eq!(nodes[1].1, "second measured sample");
    assert_eq!(nodes[2].1, "first unavailable sample");
    assert_eq!(nodes[3].1, "second unavailable sample");
}
