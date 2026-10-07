//! Behavior cases for dns quick.
//! test-intent: behavior

use super::*;

#[test]
fn test_dns_quick_template_chip_appends_unique_entry() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    navigate_to(&mut app, Route::Dns);

    let chip = app
        .world_mut()
        .query::<(Entity, &DnsEditTemplate)>()
        .iter(app.world())
        .find(|(_, chip)| chip.field == DnsFormField::Fallback && chip.server == "8.8.8.8")
        .map(|(entity, _)| entity)
        .expect("fallback template chip");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: chip });
    app.update();

    let parent = app
        .world_mut()
        .query::<(Entity, &DnsEditField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == DnsFormField::Fallback)
        .map(|(entity, _)| entity)
        .expect("fallback field");
    let text = app
        .world()
        .get::<Children>(parent)
        .and_then(|children| children.iter().next().copied())
        .and_then(|child| app.world().get::<TextField>(child))
        .expect("fallback text field")
        .0
        .text()
        .to_owned();
    assert!(text.contains("8.8.8.8"), "{text}");
    assert!(
        text.contains("https://cloudflare-dns.com/dns-query"),
        "{text}"
    );
}
