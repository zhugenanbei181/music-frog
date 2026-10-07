//! Behavior cases for settings pac.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_settings_pac_projection_and_apply_submit_shared_request() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.pac = PacSnapshot {
        state: PacServiceState::Running {
            url: "http://127.0.0.1:31000/proxy.pac".to_owned(),
        },
        script_bytes: 2048,
        bypass_domains: vec!["example.com".to_owned(), "*.lan".to_owned()],
        revision: 3,
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "运行中 · http://127.0.0.1:31000/proxy.pac · 2048 字节"
    ));

    let apply_button = app
        .world_mut()
        .query_filtered::<Entity, With<PacApplyButton>>()
        .single(app.world())
        .expect("PAC apply button");
    app.world_mut().commands().trigger(Activate {
        entity: apply_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ApplyPac {
            enabled: true,
            bypass_domains: vec!["example.com".to_owned(), "*.lan".to_owned()],
            bypass_lan: true,
            minify: false,
        }]
    );
    let _ = app
        .world_mut()
        .query::<&PacBypassField>()
        .single(app.world())
        .expect("PAC bypass field");
}
