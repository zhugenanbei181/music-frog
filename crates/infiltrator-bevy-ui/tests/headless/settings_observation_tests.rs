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
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_application::runtime_control_projection::RuntimeControlApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::settings::settings_core::{
    SettingsLine, SettingsLineKind, TunEnableToggle,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_ports::runtime_configuration::{
    ConfigurationObservation, ConfigurationRevision, DnsRuntimeConfiguration, Ipv6Configuration,
    LanAuthenticationState, LanSecurityConfiguration, LanSharingConfiguration, ObservedValue,
    ProxyPortsConfiguration, SnifferConfiguration, TunConfiguration,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::session::SessionToken;
use std::sync::Arc;

fn tun_readout() -> RuntimeConfigurationReadout {
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("bevy-settings-observation").expect("instance"),
            Some(CoreVersion::new("test").expect("version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    };
    let revision = ConfigurationRevision::new(1);
    RuntimeConfigurationReadout {
        tun: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: TunConfiguration {
                enabled: ObservedValue::Reported(false),
                stack: ObservedValue::Reported("system".to_owned()),
                ..Default::default()
            },
        },
        lan_sharing: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSharingConfiguration {
                enabled: ObservedValue::Unreported,
                mixed_port: ObservedValue::Unreported,
                bind_address: ObservedValue::Unreported,
            },
        },
        lan_security: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSecurityConfiguration {
                allowed_ips: ObservedValue::Unreported,
                disallowed_ips: ObservedValue::Unreported,
                skip_auth_prefixes: ObservedValue::Unreported,
                authentication: LanAuthenticationState::Unreported,
            },
        },
        ipv6: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: Ipv6Configuration {
                enabled: ObservedValue::Unreported,
                tun_enabled: ObservedValue::Reported(false),
            },
        },
        sniffer: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: SnifferConfiguration {
                enabled: ObservedValue::Unreported,
            },
        },
        dns: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: DnsRuntimeConfiguration::default(),
        },
        core_log_level: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: ObservedValue::Unreported,
        },
        proxy_ports: ConfigurationObservation {
            scope,
            revision,
            value: ProxyPortsConfiguration::default(),
        },
    }
}

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
    let config = tun_readout();
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
    snapshot.runtime_control =
        owner.observe(snapshot.generation, snapshot.revision, Some(&Err(failure)));
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
