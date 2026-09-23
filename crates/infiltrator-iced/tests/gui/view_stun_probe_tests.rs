use super::*;
use infiltrator_contract::stun_probe::{StunMappedAddress, StunProbeObservation, StunProbeReport};

// ===========================================================================
// DUAL-14-09 (re-scoped): STUN UDP-egress panel
// ===========================================================================

fn observed(ip: &str, expected: Option<&str>) -> StunProbeReport {
    StunProbeReport::from_observation(
        StunProbeObservation::observed(
            "stun.l.google.com:19302",
            StunMappedAddress::new(ip, 51234),
        ),
        expected.map(|ip| StunMappedAddress::new(ip, 0)),
    )
}

#[test]
fn test_stun_panel_renders_the_shared_report_honestly() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");

    // Nothing reported yet is not a result either.
    let pending = StunProbeReport::default();
    let (pending_zh, _) = stun_status_copy(&pending, &zh);
    assert!(pending_zh.contains("尚未探测"), "{pending_zh}");
    assert!(stun_comparison_copy(&pending, &zh).contains("无法比较"));

    // A host without a prober renders the typed refusal.
    let unsupported = StunProbeReport::unsupported("no STUN prober is composed");
    let (unsupported_zh, _) = stun_status_copy(&unsupported, &zh);
    assert!(
        unsupported_zh.contains("宿主未提供 STUN 探测能力"),
        "{unsupported_zh}"
    );
    let (unsupported_en, _) = stun_status_copy(&unsupported, &en);
    assert!(
        unsupported_en.contains("provides no STUN probe"),
        "{unsupported_en}"
    );

    // An observed mapping with no expected egress keeps the comparison unknown.
    let no_expected = observed("203.0.113.9", None);
    let (status_zh, _) = stun_status_copy(&no_expected, &zh);
    assert!(status_zh.contains("203.0.113.9:51234"), "{status_zh}");
    let comparison_zh = stun_comparison_copy(&no_expected, &zh);
    assert!(comparison_zh.contains("无法比较"), "{comparison_zh}");
    assert!(
        comparison_zh.contains("no expected proxied egress"),
        "{comparison_zh}"
    );

    // Matching expectations claim consistency; differing ones list both facts.
    let consistent = observed("203.0.113.9", Some("203.0.113.9"));
    assert!(stun_comparison_copy(&consistent, &zh).contains("与期望代理出网一致"));
    assert!(stun_comparison_copy(&consistent, &en).contains("consistent"));

    let divergent = observed("198.51.100.7", Some("203.0.113.9"));
    let divergent_zh = stun_comparison_copy(&divergent, &zh);
    assert!(divergent_zh.contains("不一致"), "{divergent_zh}");
    assert!(
        divergent_zh.contains("198.51.100.7:51234"),
        "{divergent_zh}"
    );
    assert!(divergent_zh.contains("203.0.113.9"), "{divergent_zh}");
    assert!(divergent_zh.contains("仅列事实"), "{divergent_zh}");

    // Timeouts and failures stay typed.
    let timed_out = StunProbeReport::from_observation(
        StunProbeObservation::timed_out("stun.l.google.com:19302"),
        Some(StunMappedAddress::new("203.0.113.9", 0)),
    );
    let (timed_out_zh, _) = stun_status_copy(&timed_out, &zh);
    assert!(timed_out_zh.contains("超时"), "{timed_out_zh}");
    let failed = StunProbeReport::from_observation(
        StunProbeObservation::failed("stun.l.google.com:19302", "network unreachable"),
        None,
    );
    let (failed_zh, _) = stun_status_copy(&failed, &zh);
    assert!(failed_zh.contains("探测失败"), "{failed_zh}");
    assert!(failed_zh.contains("network unreachable"), "{failed_zh}");

    // Both locales render the panel from the shared report.
    let (mut state, _) = AppState::new();
    state.editor.dns_stun = divergent;
    let _ = stun_panel(&state, &zh);
    let _ = stun_panel(&state, &en);
    state.editor.dns_stun = unsupported;
    let _ = stun_panel(&state, &zh);
    assert!(!state.diag.is_probing_stun);

    // The probe message reaches the shared handler; without a host runtime it
    // is a no-op instead of a fabricated mapping.
    let _ = state.update(Message::RunStunProbe);
    assert!(!state.diag.is_probing_stun);
}

#[test]
fn test_stun_panel_names_the_non_webrtc_boundary() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    let boundary_zh = zh.tr("dns_stun_not_webrtc");
    assert!(boundary_zh.contains("本机"), "{boundary_zh}");
    assert!(boundary_zh.contains("不是"), "{boundary_zh}");
    assert!(boundary_zh.contains("WebRTC"), "{boundary_zh}");
    let boundary_en = en.tr("dns_stun_not_webrtc");
    assert!(boundary_en.contains("host"), "{boundary_en}");
    assert!(boundary_en.contains("not a WebRTC"), "{boundary_en}");

    // The card renders the boundary copy as part of the shared report, and
    // never claims a browser verdict.
    let (state, _) = AppState::new();
    let _ = stun_panel(&state, &zh);
    let _ = stun_panel(&state, &en);
    assert!(!boundary_zh.contains("泄漏结论"));
}
