//! Behavior cases for overview public.
//! test-intent: behavior

use super::*;
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;

#[test]
fn overview_public_ip_probe_card_mounts_and_updates_in_place() {
    let mut app = mounted_default();
    let card_entity = {
        let world = app.world_mut();
        let mut cards = world.query::<(Entity, &PublicIpProbeCard)>();
        cards.single(world).expect("public ip probe card").0
    };
    assert!(card_entity != Entity::PLACEHOLDER);

    {
        let world = app.world_mut();
        let mut texts = world.query::<(&PublicIpText, &Text)>();
        assert!(
            texts.iter(world).any(|(marker, text)| {
                marker.0 == PublicIpTextKind::Ip && text.0 == "203.0.113.7"
            })
        );
    }

    let mut projection = DemoOverviewSource::running().current();
    projection.public_ip = PublicIpProbeSnapshot::failed(1, 2, "network timeout");
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<(&PublicIpText, &Text)>();
    assert!(texts.iter(world).any(|(marker, text)| {
        marker.0 == PublicIpTextKind::Status && text.0 == "network timeout"
    }));
}

#[test]
fn overview_public_ip_refresh_button_submits_refresh_command() {
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
        let mut buttons = world.query::<(Entity, &PublicIpRefreshButton)>();
        buttons
            .iter(world)
            .next()
            .expect("public ip refresh button")
            .0
    };

    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(sink.submitted().contains(&UiCommand::RefreshPublicIpProbe));
}
