//! Behavior cases for dns projection.
//! test-intent: behavior

use super::*;

#[test]
fn test_dns_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    let mut updated = DnsProjection::demo();
    updated.mode = DnsEnhancedMode::RedirHost;
    updated.cache_entries = 999;
    updated.fake_ip_range = "198.19.0.0/16".to_owned();
    updated.servers[0].latency_ms = Some(12);

    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "域名解析 · 真实 IP (Redir-Host)（缓存条目：999）"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "分配网段：198.19.0.0/16"
    ));
    assert!(subtree_has_text(app.world(), root, "12 ms"));
}
