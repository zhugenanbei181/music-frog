//! DUAL-05-09/10 application tests: the shared dialer report is what both
//! surfaces render, and a loop never appears among the valid chains.

use super::*;
use crate::protocol_codec_application::{ProtocolCodecApplication, clear_studio};

const PROFILE: &str = r#"
mode: rule
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
  - name: legacy-relay
    type: relay
    proxies: [nas, free]
rules:
  - MATCH,PROXY
"#;

const LOOP_PROFILE: &str = r#"
proxies:
  - name: a
    type: ss
    server: 1.1.1.1
    port: 8388
    cipher: aes-128-gcm
    password: pw
    dialer-proxy: b
  - name: b
    type: ss
    server: 2.2.2.2
    port: 8388
    cipher: aes-128-gcm
    password: pw
    dialer-proxy: a
"#;

#[test]
fn the_report_carries_chains_labels_and_document_warnings() {
    let report = DialerChainApplication::analyze_profile(PROFILE).expect("report");
    let chain = report.chain_for("nas").expect("nas chain");
    assert!(chain.valid());
    assert_eq!(chain.hops.len(), 3);
    assert_eq!(chain.hops[0].type_label, "Shadowsocks");
    assert_eq!(chain.hops[1].type_label, "VLESS");
    assert_eq!(chain.end, DialerChainEnd::Complete);
    assert!(
        chain
            .chain_line()
            .contains("nas(Shadowsocks) → gateway(VLESS)")
    );
    // The profile-level documents are represented, not hidden.
    assert!(report.warnings.iter().any(|line| line.contains("relay")));
    assert!(!report.has_loops());
    assert_eq!(report.valid_chains().len(), report.chains.len());
    assert!(report.summary_zh().contains("跳板链路 4 条"));
}

#[test]
fn a_loop_is_typed_and_never_a_valid_chain() {
    let report = DialerChainApplication::analyze_profile(LOOP_PROFILE).expect("report");
    assert!(report.has_loops());
    let finding = report.loop_for("a").expect("loop finding");
    assert_eq!(finding.path, vec!["a", "b", "a"]);
    assert!(finding.spans_dialer_proxy);
    assert!(finding.message.contains("该链路不可用"));
    for root in ["a", "b"] {
        let chain = report.chain_for(root).expect("chain");
        assert!(chain.has_loop(), "{root}");
        assert!(!chain.valid(), "{root}");
        assert!(chain.chain_line().contains("环路"));
    }
    assert!(report.valid_chains().is_empty());
    assert!(report.chips().iter().any(|chip| chip.starts_with("loop:")));
}

#[test]
fn publishing_the_report_reaches_both_surfaces_snapshot() {
    clear_studio();
    let report = DialerChainApplication::analyze_profile(PROFILE).expect("report");
    let published = ProtocolCodecApplication::publish_dialer_report(PROFILE).expect("published");
    assert_eq!(published, report);
    assert_eq!(ProtocolCodecApplication::dialer_report(), report);

    // A draft edit must not drop the profile-level report. The studio is
    // process-wide (other tests publish into it concurrently), so this checks
    // the *shape* — an analysis result is carried over, never cleared.
    let studio = ProtocolCodecApplication::publish_draft(
        infiltrator_contract::protocol_fidelity::ProtocolDraft::new("vless"),
        None,
    );
    assert!(!studio.dialer.chains.is_empty());

    // An invalid document is a typed failure, not an empty report.
    assert!(DialerChainApplication::analyze_profile("proxies: 42\n").is_err());
    clear_studio();
}

#[test]
fn a_node_save_publishes_the_dialer_verdict_for_the_written_document() {
    clear_studio();
    let mut draft = infiltrator_contract::protocol_fidelity::ProtocolDraft::new("vless");
    draft.name = "landing".into();
    draft.server = "5.5.5.5".into();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".into();
    draft.dialer_proxy = "gateway".into();
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile(PROFILE, &draft).expect("commit");
    let chain = commit.dialer.chain_for("landing").expect("chain");
    assert!(chain.valid());
    assert_eq!(chain.hops.len(), 3);
    assert_eq!(
        ProtocolCodecApplication::dialer_report().chain_for("landing"),
        Some(chain)
    );
    clear_studio();
}
