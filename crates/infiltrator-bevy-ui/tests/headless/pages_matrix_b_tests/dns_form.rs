//! Behavior cases for dns form.
//! test-intent: behavior

use super::*;

#[test]
fn test_dns_form_local_validation_blocks_invalid_scheme() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    set_dns_field(&mut app, DnsFormField::Nameserver, "ftp://dns.example");
    trigger_dns_edit_apply(&mut app);

    assert!(sink.submitted().is_empty(), "invalid form must not submit");
    let status = {
        let world = app.world_mut();
        let mut query = world.query::<(&Text, &DnsEditStatusLine)>();
        query
            .iter(world)
            .map(|(text, _)| text.0.clone())
            .next()
            .expect("status line")
    };
    assert!(status.contains("本地校验未通过"), "{status}");
    assert!(status.contains("ftp://dns.example"), "{status}");
}
