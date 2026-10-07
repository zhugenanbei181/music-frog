//! Behavior cases for dns fake.
//! test-intent: behavior

use super::*;
use infiltrator_contract::dns::FakeIpMappingPool;

#[test]
fn test_dns_fake_ip_pool_search_filters_the_observed_listing() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "198.18.0.5 ↔ music.example.org"
    ));

    let parent = app
        .world_mut()
        .query::<(Entity, &DnsFakeIpSearchField)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("fake-ip search field");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("search children")
        .iter()
        .next()
        .expect("search text field child");
    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("search text field")
        .0
        .apply(TextFieldInput::SetText("cdn".to_owned()));
    app.update();

    let listing = dns_line_text(&mut app, DnsLineKind::FakeIpMapping);
    assert_eq!(listing, "198.18.0.7 ↔ cdn.example.net");
    let count = dns_line_text(&mut app, DnsLineKind::FakeIpMappingCount);
    assert!(count.contains("显示 1 / 共观测 2 条"), "{count}");

    // A host without the controller connection feed never renders a binding.
    let mut unsupported = DnsProjection::demo();
    unsupported.fake_ip_pool = FakeIpMappingPool::default();
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(unsupported));
    app.update();
    assert!(dns_line_text(&mut app, DnsLineKind::FakeIpMapping).contains("宿主未提供映射事实"));
}
