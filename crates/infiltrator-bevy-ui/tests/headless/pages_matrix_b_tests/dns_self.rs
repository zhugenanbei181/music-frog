//! Behavior cases for dns self.
//! test-intent: behavior

use super::*;

#[test]
fn test_dns_self_heal_card_renders_the_shared_observation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    // The demo fixture's snapshot (one warning, two healthy) is rendered with
    // shared localized labels and the human-readable suggested action.
    assert!(subtree_has_text(
        app.world(),
        root,
        "监听端口 (dns.listen) [正常] dns.listen port 1053 is available"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "上游解析可达性 [注意] only 3 of 4 upstreams answered · 建议修复: 重新探测上游"
    ));
}
