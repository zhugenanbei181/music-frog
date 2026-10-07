//! Behavior cases for dns latency.
//! test-intent: behavior

use super::*;
use infiltrator_contract::dns_latency::DnsLatencyReport;

#[test]
fn test_dns_latency_policy_line_is_honest() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    // The demo fixture carries the pinned screenshot values, and every row is
    // rendered from the shared report type.
    assert!(subtree_has_text(app.world(), root, "4 个上游中仅 3 个应答"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "tls://8.8.8.8:853 [主上游] 未探测: DNS over TLS is not probed by this host"
    ));

    // A host with no prober publishes the typed refusal, never a number.
    let mut without_prober = DnsProjection::demo();
    without_prober.latency = DnsLatencyReport::default();
    without_prober.self_heal = Default::default();
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(without_prober));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "宿主未提供逐 Nameserver 延迟事实，不填充假延迟 (this host did not inject a DNS latency prober)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "宿主未提供逐 Nameserver 延迟事实，不填充假延迟"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "尚未观测到 DNS 健康事实"
    ));
}
