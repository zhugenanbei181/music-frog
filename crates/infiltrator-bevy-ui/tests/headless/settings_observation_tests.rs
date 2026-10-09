//! test-intent: behavior
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ui::InteractionDisabled;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{ButtonPlugin, Checkbox, CheckboxPlugin};
use infiltrator_application::runtime_control_projection::RuntimeControlApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::settings::settings_core::{
    SettingsLine, SettingsLineKind, TunEnableToggle,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::runtime::{ConfigSnapshot, TunSnapshot};
use infiltrator_ports::error::PortError;
use std::sync::Arc;

fn line(app: &mut App, kind: SettingsLineKind) -> String {
    app.world_mut()
        .query::<(&SettingsLine, &Text)>()
        .iter(app.world())
        .find(|(part, _)| part.0 == kind)
        .unwrap()
        .1
        .0
        .clone()
}

#[test]
fn native_settings_unknown_false_failed_and_recovered_controls_keep_entities_and_only_submit_observed_intents()
 {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        CheckboxPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
    ));
    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(
        sink.clone() as Arc<dyn UiCommandSink>
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    let owner = RuntimeControlApplication::default();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.runtime_control = owner.observe(snapshot.generation, snapshot.revision, None);
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    let parent = app
        .world_mut()
        .query_filtered::<Entity, With<TunEnableToggle>>()
        .single(app.world())
        .unwrap();
    let checkbox = app
        .world()
        .get::<Children>(parent)
        .unwrap()
        .iter()
        .find(|entity| app.world().get::<Checkbox>(**entity).is_some())
        .copied()
        .unwrap();
    assert_eq!(line(&mut app, SettingsLineKind::TunStack), "未观测");
    assert_eq!(line(&mut app, SettingsLineKind::LogLevel), "当前: 未观测");
    assert!(app.world().get::<ButtonDisabled>(checkbox).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(checkbox).is_some());
    click_entity(&mut app, checkbox);
    assert!(sink.submitted().is_empty());
    let config = ConfigSnapshot {
        mode: "direct".into(),
        log_level: "warning".into(),
        tun: Some(TunSnapshot {
            enable: Some(false),
            stack: Some("system".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    snapshot.revision += 1;
    snapshot.runtime_control = owner.observe(
        snapshot.generation,
        snapshot.revision,
        Some(&Ok(config.clone())),
    );
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    assert_eq!(line(&mut app, SettingsLineKind::TunStack), "system");
    assert!(app.world().get::<InteractionDisabled>(checkbox).is_none());
    click_entity(&mut app, checkbox);
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ToggleTun { enabled: true }]
    );
    sink.clear();
    snapshot.revision += 1;
    let failure = Failure::new(ErrorCode::Authentication, "read denied", false);
    snapshot.runtime_control = owner.observe(
        snapshot.generation,
        snapshot.revision,
        Some(&Err(PortError::Rejected(failure))),
    );
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    assert_eq!(line(&mut app, SettingsLineKind::TunStack), "system");
    assert!(line(&mut app, SettingsLineKind::RuntimeStatus).contains("保留值已失效"));
    assert!(app.world().get::<InteractionDisabled>(checkbox).is_some());
    click_entity(&mut app, checkbox);
    assert!(sink.submitted().is_empty());
    snapshot.revision += 1;
    snapshot.runtime_control =
        owner.observe(snapshot.generation, snapshot.revision, Some(&Ok(config)));
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    assert!(app.world().get::<InteractionDisabled>(checkbox).is_none());
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<TunEnableToggle>>()
            .single(app.world())
            .unwrap(),
        parent
    );
    assert!(
        app.world()
            .get::<Children>(parent)
            .unwrap()
            .contains(&checkbox)
    );
    click_entity(&mut app, checkbox);
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ToggleTun { enabled: true }]
    );
}

#[test]
fn render_strategy_setting_selects_persists_and_falls_back_typed() {
    use bevy::ui_widgets::Activate;
    use infiltrator_bevy_ui::pages::settings::settings_render_strategy::{
        RENDER_STRATEGY_SETTING_KEY, RenderStrategyButton, parse_render_strategy,
    };
    use infiltrator_bevy_widgets::shader_fx::CardRenderStrategy;

    assert_eq!(
        parse_render_strategy("gpu_instanced"),
        CardRenderStrategy::GpuInstanced
    );
    assert_eq!(
        parse_render_strategy("cpu_fallback"),
        CardRenderStrategy::CpuFallback
    );
    assert_eq!(parse_render_strategy("flat"), CardRenderStrategy::Flat);
    assert_eq!(
        parse_render_strategy("unsupported-token"),
        CardRenderStrategy::default()
    );

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        CheckboxPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
    ));
    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(
        sink.clone() as Arc<dyn UiCommandSink>
    ));
    app.init_resource::<CardRenderStrategy>();
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();

    let target = app
        .world_mut()
        .query::<(Entity, &RenderStrategyButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == CardRenderStrategy::CpuFallback)
        .map(|(entity, _)| entity)
        .expect("the CPU strategy pill is mounted");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: target });
    app.update();

    assert_eq!(
        *app.world().resource::<CardRenderStrategy>(),
        CardRenderStrategy::CpuFallback
    );
    assert!(sink.submitted().contains(&UiCommand::UpdateSetting {
        key: RENDER_STRATEGY_SETTING_KEY.to_owned(),
        value: "cpu_fallback".to_owned(),
    }));
}
