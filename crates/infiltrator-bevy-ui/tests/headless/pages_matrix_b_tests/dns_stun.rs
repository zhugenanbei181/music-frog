//! Behavior cases for dns stun.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_contract::stun_probe::StunProbeReport;

#[test]
fn test_dns_stun_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<TestStunProbeButton>>()
        .single(app.world())
        .expect("test stun probe button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RunStunProbe]);
}

/// DUAL-14-09 (re-scoped): the panel renders this host's UDP egress mapping
/// and its comparison as facts only, names the non-WebRTC boundary, and a host
/// without a prober renders the typed refusal instead of a mapping.
#[test]
fn test_dns_stun_card_renders_the_shared_egress_report() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "观测映射：203.0.113.9:51234"
    ));
    assert!(subtree_has_text(app.world(), root, "与期望代理出网一致"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "边界：这是本机/本进程的 UDP 出网映射，不是某个浏览器实例的 WebRTC 穿透结论"
    ));

    let mut without_prober = DnsProjection::demo();
    without_prober.stun = StunProbeReport::unsupported("no STUN prober is composed");
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(without_prober));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "宿主未提供 STUN 探测能力（no STUN prober is composed）"
    ));
    assert!(!subtree_has_text(
        app.world(),
        root,
        "观测映射：203.0.113.9:51234"
    ));
}
