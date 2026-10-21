//! Behavior cases for settings lan.
//! test-intent: behavior

use super::*;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::settings::settings_core::{SettingsLine, SettingsLineKind};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::lan::LanSharingSnapshot;

/// The LAN card's current-binding line: the owner's bind address, or the
/// shared "not observed" copy when the owner published no readback.
fn lan_binding_line(app: &mut App) -> String {
    app.world_mut()
        .query::<(&SettingsLine, &Text)>()
        .iter(app.world())
        .find(|(line, _)| line.0 == SettingsLineKind::LanBindAddress)
        .map(|(_, text)| text.0.clone())
        .expect("the LAN card's current-binding line")
}

/// The LAN card reads the owner's *one* LAN sharing readback.
///
/// The owner publishes the configuration whole or not at all, so the card
/// shows the owner's facts when the owner published them and says "not
/// observed" when it published none — it never claims the sharing is off.
#[test]
fn the_lan_card_reads_the_owner_lan_sharing_readback() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);
    app.insert_resource(UiLocale::new("en-US"));
    app.update();

    // The observed readback is rendered verbatim.
    let mut observed = SettingsProjection::demo();
    observed.lan_sharing = Some(LanSharingSnapshot::new(3, true, 8080, "192.168.1.10"));
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(observed));
    app.update();
    assert!(subtree_has_text(app.world(), root, "192.168.1.10"));
    assert_eq!(lan_binding_line(&mut app), "192.168.1.10");

    // An owner that published no readback is "not observed", never "off".
    let mut unobserved = SettingsProjection::demo();
    unobserved.lan_sharing = None;
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(unobserved));
    app.update();
    assert_eq!(
        lan_binding_line(&mut app),
        "Not observed",
        "an unobserved readback says so instead of claiming the sharing is off"
    );
}

#[test]
fn test_settings_lan_fields_submit_the_live_listener_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.lan_sharing = Some(LanSharingSnapshot::new(2, true, 8080, "192.168.1.10"));
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "192.168.1.10"));

    let apply_button = app
        .world_mut()
        .query_filtered::<Entity, With<LanSharingApplyButton>>()
        .single(app.world())
        .expect("Allow-LAN apply button");
    app.world_mut().commands().trigger(Activate {
        entity: apply_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetLanSharing {
            enabled: true,
            mixed_port: 8080,
            bind_address: "192.168.1.10".to_owned(),
        }]
    );

    let _ = app
        .world_mut()
        .query::<&LanSharingToggle>()
        .single(app.world())
        .expect("Allow-LAN toggle");
    let _ = app
        .world_mut()
        .query::<&LanMixedPortField>()
        .single(app.world())
        .expect("mixed-port field");
    let _ = app
        .world_mut()
        .query::<&LanBindAddressField>()
        .single(app.world())
        .expect("bind-address field");
    let _ = app
        .world_mut()
        .query::<&TextField>()
        .iter(app.world())
        .count();
}

#[test]
fn test_settings_lan_security_submits_acl_and_redacted_basic_auth_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.lan_security = Some(LanSecuritySnapshot::new(
        5,
        vec!["192.168.1.0/24".to_owned()],
        vec!["192.168.1.10/32".to_owned()],
        vec!["127.0.0.0/8".to_owned()],
        true,
        1,
        Some("lan-user".to_owned()),
    ));
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "已启用 · 1 个账号"));

    let password_source = {
        let mut fields = app
            .world_mut()
            .query::<(&LanAuthPasswordField, &Children)>();
        *fields
            .single(app.world())
            .expect("LAN password field")
            .1
            .iter()
            .next()
            .expect("LAN password text field")
    };
    app.world_mut()
        .get_mut::<TextField>(password_source)
        .expect("LAN password text field state")
        .0
        .apply(TextFieldInput::SetText("secret-value".to_owned()));

    let apply_button = app
        .world_mut()
        .query_filtered::<Entity, With<LanSecurityApplyButton>>()
        .single(app.world())
        .expect("LAN security apply button");
    app.world_mut().commands().trigger(Activate {
        entity: apply_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetLanSecurity {
            allowed_ips: vec!["192.168.1.0/24".to_owned()],
            disallowed_ips: vec!["192.168.1.10/32".to_owned()],
            skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
            authentication_enabled: true,
            credentials: Some(LanCredentials {
                username: "lan-user".to_owned(),
                password: "secret-value".to_owned(),
            }),
        }]
    );
    assert!(!format!("{:?}", sink.submitted()).contains("secret-value"));
    assert!(
        app.world()
            .get::<TextField>(password_source)
            .expect("password field after submit")
            .0
            .text()
            .is_empty()
    );

    let _ = app
        .world_mut()
        .query::<&LanAllowedIpsField>()
        .single(app.world())
        .expect("LAN allowed field");
    let _ = app
        .world_mut()
        .query::<&LanDisallowedIpsField>()
        .single(app.world())
        .expect("LAN denied field");
    let _ = app
        .world_mut()
        .query::<&LanSkipAuthPrefixesField>()
        .single(app.world())
        .expect("LAN skip-auth field");
    let _ = app
        .world_mut()
        .query::<&LanAuthenticationToggle>()
        .single(app.world())
        .expect("LAN auth toggle");
    let _ = app
        .world_mut()
        .query::<&LanAuthUsernameField>()
        .single(app.world())
        .expect("LAN username field");
}
