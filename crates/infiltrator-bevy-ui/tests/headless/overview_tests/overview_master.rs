//! Behavior cases for overview master.
//! test-intent: behavior

use super::*;
use infiltrator_contract::system_toggle::SystemToggle;

#[test]
fn test_overview_master_switches_and_exit_node_cards() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut exit_query = world.query::<(Entity, &ActiveExitNodeCard)>();
    let (exit_entity, _) = exit_query
        .iter(world)
        .next()
        .expect("ActiveExitNodeCard must be mounted");
    let exit_descendants = descendants(world, exit_entity);
    let exit_texts: Vec<String> = exit_descendants
        .iter()
        .filter_map(|e| world.get::<Text>(*e).map(|t| t.0.clone()))
        .collect();
    assert!(exit_texts.iter().any(|t| t.contains("当前主出口节点")));
    assert!(exit_texts.iter().any(|t| t.contains("🇭🇰")));
    assert!(exit_texts.iter().any(|t| t.contains("香港 IPLC 01")));
    assert!(exit_texts.iter().any(|t| t.contains("VLESS · Reality")));
    assert!(exit_texts.iter().any(|t| t.contains("38 ms")));

    let mut proxy_query = world.query::<(Entity, &SystemProxyMasterCard)>();
    assert!(
        proxy_query.iter(world).next().is_some(),
        "SystemProxyMasterCard must be mounted"
    );

    let mut tun_query = world.query::<(Entity, &TunMasterCard)>();
    assert!(
        tun_query.iter(world).next().is_some(),
        "TunMasterCard must be mounted"
    );
}

#[test]
fn overview_master_switch_uses_shared_toggle_policy_and_command_sink() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewMasterSwitchButton)>();
        buttons
            .iter(world)
            .find(|(_, button)| {
                button.toggle == SystemToggle::SystemProxy && button.can_toggle && button.enabled
            })
            .expect("system proxy master switch is enabled and actionable")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    assert!(
        sink.submitted()
            .contains(&UiCommand::SetSystemProxy { enabled: false })
    );
}
