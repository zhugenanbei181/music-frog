//! DUAL-05-09/10 contract tests: the shared chain vocabulary both surfaces
//! render, including the hard rule that a loop is never a valid chain.

use super::*;

fn hop(name: &str, node_type: &str, label: &str) -> DialerHopView {
    DialerHopView {
        name: name.to_string(),
        kind: DialerHopKind::Node,
        node_type: node_type.to_string(),
        type_label: label.to_string(),
        candidates: Vec::new(),
    }
}

#[test]
fn a_valid_chain_renders_arrow_hops_and_stays_valid() {
    let chain = DialerChainView {
        root: "nas".to_string(),
        hops: vec![
            hop("nas", "ss", "Shadowsocks"),
            hop("gateway", "vless", "VLESS"),
        ],
        end: DialerChainEnd::Complete,
    };
    assert!(chain.valid());
    assert!(!chain.has_loop());
    assert_eq!(chain.chain_line(), "nas(Shadowsocks) → gateway(VLESS)");
    assert!(chain.chips().contains(&"chain:2".to_string()));
    assert_eq!(
        chain.summary_zh(),
        "nas(Shadowsocks) → gateway(VLESS) · 链路完整"
    );
}

#[test]
fn a_loop_is_never_presented_as_a_valid_chain() {
    let chain = DialerChainView {
        root: "a".to_string(),
        hops: vec![hop("a", "ss", "Shadowsocks"), hop("b", "ss", "Shadowsocks")],
        end: DialerChainEnd::Cycle {
            path: vec!["a".to_string(), "b".to_string(), "a".to_string()],
        },
    };
    assert!(!chain.valid());
    assert!(chain.has_loop());
    assert!(
        chain.chain_line().contains("环路"),
        "{}",
        chain.chain_line()
    );
    assert!(chain.chain_line().contains("⛔"), "{}", chain.chain_line());
    assert!(!chain.warning_lines().is_empty());
    assert!(chain.chips().contains(&"loop:yes".to_string()));

    let report = DialerChainReport {
        chains: vec![chain],
        loops: vec![DialerLoopFinding {
            kind: DialerLoopKind::Mutual,
            path: vec!["a".to_string(), "b".to_string(), "a".to_string()],
            spans_dialer_proxy: true,
            message: loop_message(
                DialerLoopKind::Mutual,
                &["a".to_string(), "b".to_string(), "a".to_string()],
            ),
        }],
        warnings: Vec::new(),
    };
    assert!(report.valid_chains().is_empty());
    assert!(report.has_loops());
    assert!(report.chips().iter().any(|chip| chip.starts_with("loop:")));
    assert!(report.summary_zh().contains("环路 1 处"));
    assert!(report.loop_lines()[0].contains("该链路不可用"));
}

#[test]
fn a_missing_target_is_invalid_and_a_group_boundary_is_not() {
    let missing = DialerChainView {
        root: "nas".to_string(),
        hops: vec![hop("nas", "ss", "Shadowsocks")],
        end: DialerChainEnd::MissingTarget {
            name: "nowhere".to_string(),
        },
    };
    assert!(!missing.valid());
    assert!(!missing.has_loop());
    assert!(missing.chain_line().contains("缺失"));

    let boundary = DialerChainView {
        root: "nas".to_string(),
        hops: vec![
            hop("nas", "ss", "Shadowsocks"),
            DialerHopView {
                name: "dial".to_string(),
                kind: DialerHopKind::Group,
                node_type: "select".to_string(),
                type_label: "select".to_string(),
                candidates: vec!["hk".to_string(), "jp".to_string()],
            },
        ],
        end: DialerChainEnd::GroupBoundary {
            name: "dial".to_string(),
        },
    };
    assert!(boundary.valid());
    assert!(boundary.chain_line().contains("dial(select: hk, jp)"));
    assert!(!boundary.warning_lines().is_empty());
}

#[test]
fn hop_labels_and_group_detection_are_shared_vocabulary() {
    assert_eq!(hop_type_label("vless", false), "VLESS");
    assert_eq!(hop_type_label("ss", false), "Shadowsocks");
    assert_eq!(hop_type_label("select", true), "select");
    assert!(is_group_type("url-test"));
    assert!(!is_group_type("vless"));
    assert!(target_is_group(&["dial".to_string()], "dial"));
    assert!(!target_is_group(&["dial".to_string()], "nas"));
    assert_eq!(DialerLoopKind::SelfLoop.label_zh(), "自引用");
}
