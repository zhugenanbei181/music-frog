//! test-intent: behavior
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, InteractionDisabled, Node, Val};
use bevy::ui_widgets::{Activate, ButtonPlugin};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::pages::settings::settings_core::SettingsProjection;
use infiltrator_bevy_ui::pages::settings::settings_preferences::{
    PreferenceKind, PreferenceKnob, PreferenceStatus,
};
use infiltrator_bevy_ui::pages::settings::{
    CloseToTrayToggle, SettingsProjectionUpdated, SystemNotificationsToggle,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{ThemeSkin, TokenColor, contrast};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageStatus;
use std::sync::Arc;

#[test]
fn native_preferences_replay_real_flags_stale_gates_locales_and_explicit_boolean_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
        CommandPumpPlugin::new(sink.clone()),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    let tray = app
        .world_mut()
        .query::<(Entity, &CloseToTrayToggle)>()
        .single(app.world())
        .unwrap()
        .0;
    let notifications = app
        .world_mut()
        .query::<(Entity, &SystemNotificationsToggle)>()
        .single(app.world())
        .unwrap()
        .0;
    let label = app
        .world_mut()
        .query::<(Entity, &PreferenceStatus)>()
        .iter(app.world())
        .find(|(_, marker)| matches!(marker.0, PreferenceKind::Tray))
        .unwrap()
        .0;
    let knob = app
        .world_mut()
        .query::<(Entity, &PreferenceKnob)>()
        .iter(app.world())
        .find(|(_, marker)| matches!(marker.0, PreferenceKind::Tray))
        .unwrap()
        .0;
    let mut facts = SettingsProjection::demo();
    facts.close_to_tray = Some(false);
    facts.notifications_enabled = Some(true);
    app.world_mut()
        .trigger(SettingsProjectionUpdated(facts.clone()));
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "已关闭");
    assert_eq!(app.world().get::<Node>(knob).unwrap().left, Val::Px(2.0));
    assert!(!app.world().get::<ButtonDisabled>(tray).unwrap().0);
    click_entity(&mut app, tray);
    click_entity(&mut app, notifications);
    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::UpdateSetting {
                key: "close_to_tray".into(),
                value: "true".into()
            },
            UiCommand::UpdateSetting {
                key: "notifications_enabled".into(),
                value: "false".into()
            },
        ]
    );
    assert_eq!(
        app.world().get::<Text>(label).unwrap().0,
        "已关闭",
        "queue admission cannot change observed facts"
    );
    facts.preference_status = PageStatus::Failed {
        failure: Failure::new(ErrorCode::Storage, "denied {state}/中文🙂", true),
    };
    app.world_mut()
        .trigger(SettingsProjectionUpdated(facts.clone()));
    app.update();
    assert!(app.world().get::<ButtonDisabled>(tray).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(tray).is_some());
    click_entity(&mut app, tray);
    app.world_mut().trigger(Activate { entity: tray });
    app.update();
    assert_eq!(sink.submitted().len(), 2);
    assert!(
        app.world()
            .get::<Text>(label)
            .unwrap()
            .0
            .contains("denied {state}/中文🙂")
    );
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(
        app.world().get::<Text>(label).unwrap().0,
        "Disabled (stale: denied {state}/中文🙂)"
    );
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(tray)
            .unwrap()
            .0
            .label(),
        Some("Minimize to Tray on Close")
    );
    facts.preference_status = PageStatus::Ready;
    facts.close_to_tray = None;
    app.world_mut()
        .trigger(SettingsProjectionUpdated(facts.clone()));
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Unknown");
    assert_eq!(
        *app.world().get::<Visibility>(knob).unwrap(),
        Visibility::Hidden
    );
    assert!(app.world().get::<ButtonDisabled>(tray).unwrap().0);
    facts.close_to_tray = Some(true);
    app.world_mut().trigger(SettingsProjectionUpdated(facts));
    app.update();
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "Enabled");
    assert_eq!(app.world().get::<Node>(knob).unwrap().left, Val::Px(18.0));
    assert_eq!(
        *app.world().get::<Visibility>(knob).unwrap(),
        Visibility::Inherited
    );
    assert!(app.world().get::<InteractionDisabled>(tray).is_none());
    for skin in [ThemeSkin::Light, ThemeSkin::Dark] {
        app.world_mut().trigger(ThemeSwitch(skin));
        app.update();
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "Enabled");
        let ink = app.world().get::<TextColor>(label).unwrap().0.to_srgba();
        let mut layers = Vec::new();
        let mut entity = label;
        loop {
            if let Some(fill) = app.world().get::<BackgroundColor>(entity) {
                layers.push(fill.0.to_srgba());
            }
            let Some(parent) = app.world().get::<ChildOf>(entity) else {
                break;
            };
            entity = parent.parent();
        }
        let mut fill = [0.0; 3];
        for layer in layers.into_iter().rev() {
            for (channel, front) in fill.iter_mut().zip([layer.red, layer.green, layer.blue]) {
                *channel = front * layer.alpha + *channel * (1.0 - layer.alpha);
            }
        }
        let ratio = contrast::contrast_ratio(
            TokenColor::rgba(ink.red, ink.green, ink.blue, ink.alpha),
            TokenColor::rgb(fill[0], fill[1], fill[2]),
        );
        assert!(ratio >= 4.5, "preference caption is unreadable: {ratio}");
        assert_eq!(app.world().get::<Node>(knob).unwrap().left, Val::Px(18.0));
    }
}
