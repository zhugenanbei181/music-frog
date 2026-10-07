//! Behavior cases for dns edit.
//! test-intent: behavior

use super::*;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_shared::locales::{Lang, Localizer};

#[test]
fn test_dns_edit_rows_cover_every_shared_text_field() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    let rendered: Vec<DnsFormField> = {
        let world = app.world_mut();
        let mut query = world.query::<&DnsEditField>();
        query.iter(world).map(|marker| marker.0).collect()
    };
    let form = DnsWorkbenchForm::default();
    for field in DnsFormField::ALL {
        if form.raw(field).is_none() {
            continue;
        }
        assert!(
            rendered.contains(&field),
            "Bevy workbench edit row missing for {field:?}"
        );
    }
    assert!(subtree_has_text(
        app.world(),
        root,
        Lang("zh-CN").tr("dns_upstream_form_title").as_ref()
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "GEOIP 触发回退 (fallback_filter.geoip)"
    ));
}
