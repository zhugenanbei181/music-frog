//! Behavior cases for dns page.
//! test-intent: behavior

use super::*;

#[test]
fn test_dns_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "域名解析 · 虚拟 IP (Fake-IP)（缓存条目：342）"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "https://1.1.1.1/dns-query"
    ));
    assert!(subtree_has_text(app.world(), root, "DoH (HTTPS)"));
    assert!(subtree_has_text(app.world(), root, "28 ms"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "分配网段：198.18.0.1/16"
    ));

    // Verify 6 DNS form switches matching Iced
    assert!(subtree_has_text(app.world(), root, "enable"));
    assert!(subtree_has_text(app.world(), root, "启用 DNS 服务"));
    assert!(subtree_has_text(app.world(), root, "ipv6"));
    assert!(subtree_has_text(app.world(), root, "IPv6 解析"));
    assert!(subtree_has_text(app.world(), root, "cache"));
    assert!(subtree_has_text(app.world(), root, "DNS 内存缓存"));
    assert!(subtree_has_text(app.world(), root, "use_hosts"));
    assert!(subtree_has_text(app.world(), root, "遵循系统 Hosts"));
    assert!(subtree_has_text(app.world(), root, "use_system_hosts"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "use_system_hosts（系统 Hosts）"
    ));
    assert!(subtree_has_text(app.world(), root, "respect_rules"));
    assert!(subtree_has_text(app.world(), root, "分流规则优先"));

    // Verify Domain Mapping Mode and Filter Mode segmented controls
    assert!(subtree_has_text(
        app.world(),
        root,
        "域名映射模式 (enhanced_mode)"
    ));
    assert!(subtree_has_text(app.world(), root, "虚拟 IP (Fake-IP)"));
    assert!(subtree_has_text(app.world(), root, "真实 IP (Redir-Host)"));
    assert!(subtree_has_text(app.world(), root, "取消映射 (None)"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "过滤模式 (fake_ip_filter_mode)"
    ));
    assert!(subtree_has_text(app.world(), root, "黑名单 (Blacklist)"));
    assert!(subtree_has_text(app.world(), root, "白名单 (Whitelist)"));
    assert!(subtree_has_text(app.world(), root, "规则 (Rules)"));

    let switch_count = app
        .world_mut()
        .query::<&DnsSwitchButton>()
        .iter(app.world())
        .count();
    assert_eq!(switch_count, 6);
}
