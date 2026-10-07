//! Behavior cases for dns leak.
//! test-intent: behavior

use super::*;
use infiltrator_contract::dns_leak::DnsLeakReport;

/// DUAL-14-08: the demo fixture's divergent cross-source facts are rendered
/// verbatim, and a host without a fact source renders the typed unsupported
/// state instead of a verdict.
#[test]
fn test_dns_leak_card_renders_the_shared_cross_source_report() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "交叉不一致：观测到 2 个不同解析器身份（仅列事实）"
    ));
    assert!(subtree_has_text(app.world(), root, "观测身份：203.0.113.9"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "观测身份：198.51.100.7"
    ));
    assert!(!subtree_has_text(app.world(), root, "未观测"));

    let mut without_sources = DnsProjection::demo();
    without_sources.leak = DnsLeakReport::unsupported("no echo authority is configured");
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(without_sources));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "宿主未提供泄漏探测事实源（no echo authority is configured）"
    ));
    assert!(subtree_has_text(app.world(), root, "尚无泄漏探测观测结果"));
    assert!(!subtree_has_text(app.world(), root, "观测身份"));
}
