//! Behavior cases for profiles fetch.
//! test-intent: behavior

use super::*;
use bevy::ui::Checked;
use infiltrator_bevy_ui::pages::profiles_subscription_copy::SubscriptionStatusKind;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};

#[test]
fn test_profiles_fetch_options_projection_restamps_ua_insecure_and_validators() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "ClashVerge/2.0"),
        "active profile User-Agent reaches the text field"
    );
    assert!(
        subtree_has_text(app.world(), root, "条件请求已缓存"),
        "cached ETag / Last-Modified reaches the status line"
    );
    let insecure_checked = {
        let mut toggles = app
            .world_mut()
            .query::<(&SubscriptionInsecureToggle, &Children)>();
        let (_, children) = toggles.single(app.world()).expect("insecure toggle");
        children
            .iter()
            .any(|child| app.world().get::<Checked>(*child).is_some())
    };
    assert!(
        insecure_checked,
        "insecure-TLS toggle reflects the projection"
    );
}

#[test]
fn subscription_status_locale_replay_keeps_last_modified_only_cache_and_editor_draft() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Profiles);
    let mut projection = subscription_fetch_projection();
    let profile = projection
        .profiles
        .iter_mut()
        .find(|profile| profile.is_active)
        .unwrap();
    profile.etag = None;
    profile.last_modified = Some("Mon {etag}".into());
    profile.auto_update_enabled = false;
    profile.cron_expression = None;
    profile.update_interval_hours = Some(3);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    let labels = app
        .world_mut()
        .query::<(Entity, &SubscriptionStatusKind)>()
        .iter(app.world())
        .map(|(entity, kind)| (entity, *kind))
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 4);
    for expected_kind in [
        SubscriptionStatusKind::Conditional,
        SubscriptionStatusKind::Backup,
        SubscriptionStatusKind::Filter,
        SubscriptionStatusKind::Schedule,
    ] {
        assert_eq!(
            labels
                .iter()
                .filter(|(_, kind)| *kind == expected_kind)
                .count(),
            1,
            "each mounted status uses its own typed kind"
        );
    }
    let field = {
        let children = app
            .world_mut()
            .query::<(&SubscriptionUserAgentField, &Children)>()
            .single(app.world())
            .unwrap()
            .1;
        children
            .iter()
            .copied()
            .find(|entity| app.world().get::<TextField>(*entity).is_some())
            .unwrap()
    };
    let mut editing = app.world_mut().get_mut::<TextField>(field).unwrap();
    editing
        .0
        .apply(TextFieldInput::SetText("user draft {modified}".into()));
    editing.0.apply(TextFieldInput::SelectAll);
    editing.0.set_preedit("zhong");
    let expected = editing.0.clone();
    app.world_mut()
        .get_mut::<TextFieldFocused>(field)
        .unwrap()
        .0 = true;
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    for (entity, kind) in labels {
        assert_eq!(
            app.world().get::<SubscriptionStatusKind>(entity),
            Some(&kind)
        );
        let text = &app.world().get::<Text>(entity).unwrap().0;
        match kind {
            SubscriptionStatusKind::Conditional => assert_eq!(
                text,
                "Conditional request cached · ETag: — · Last-Modified: Mon {etag}"
            ),
            SubscriptionStatusKind::Schedule => {
                assert_eq!(text, "Scheduled updates paused · every 3 hours")
            }
            SubscriptionStatusKind::Backup => assert!(text.starts_with("Safe backup")),
            SubscriptionStatusKind::Filter => assert!(text.starts_with("Filter pipeline")),
        }
    }
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, expected);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert!(
        sink.submitted().is_empty(),
        "status and locale replay never submit a command"
    );
}
