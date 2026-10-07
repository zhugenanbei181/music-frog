//! Behavior cases for dns cache.
//! test-intent: behavior

use super::*;
use infiltrator_contract::dns_cache::{DnsCacheFlushReport, DnsFlushOutcome};

#[test]
fn test_dns_cache_flush_report_renders_honest_status() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "Fake-IP 缓存：尚未执行 · 系统 DNS 缓存：尚未执行"
    ));

    let mut updated = DnsProjection::demo();
    updated.cache_flush = DnsCacheFlushReport {
        fake_ip: DnsFlushOutcome::Flushed,
        os_cache: DnsFlushOutcome::Unsupported {
            reason: "host did not provide a system DNS cache adapter".to_owned(),
        },
    };
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "Fake-IP 缓存：已清空 · 系统 DNS 缓存：宿主不支持（host did not provide a system DNS cache adapter）"
    ));
}
