use super::*;

#[test]
fn test_sort_delay_nodes() {
    let mut nodes: Vec<DelayNodeItem> = vec![
        ("Node B".to_string(), "Shadowsocks".to_string(), Some(150)),
        ("Node A".to_string(), "VMess".to_string(), Some(50)),
        ("Node C".to_string(), "Trojan".to_string(), None),
        ("Node D".to_string(), "Hysteria2".to_string(), Some(300)),
    ];

    sort_delay_nodes(&mut nodes, "delay_asc");
    assert_eq!(nodes[0].0, "Node A");
    assert_eq!(nodes[1].0, "Node B");
    assert_eq!(nodes[2].0, "Node D");
    assert_eq!(nodes[3].0, "Node C");

    sort_delay_nodes(&mut nodes, "delay_desc");
    assert_eq!(nodes[0].0, "Node D");
    assert_eq!(nodes[1].0, "Node B");
    assert_eq!(nodes[2].0, "Node A");
    assert_eq!(nodes[3].0, "Node C");

    sort_delay_nodes(&mut nodes, "name_asc");
    assert_eq!(nodes[0].0, "Node A");
    assert_eq!(nodes[1].0, "Node B");
    assert_eq!(nodes[2].0, "Node C");
    assert_eq!(nodes[3].0, "Node D");

    sort_delay_nodes(&mut nodes, "name_desc");
    assert_eq!(nodes[0].0, "Node D");
    assert_eq!(nodes[1].0, "Node C");
    assert_eq!(nodes[2].0, "Node B");
    assert_eq!(nodes[3].0, "Node A");
}

#[test]
fn test_latency_bar_and_status_dot() {
    let _dot_some: Element<'_, Message> = delay_status_dot(Some(100));
    let _dot_none: Element<'_, Message> = delay_status_dot(None);

    let _bar_some: Element<'_, Message> = latency_bar(Some(200));
    let _bar_none: Element<'_, Message> = latency_bar(None);
}
