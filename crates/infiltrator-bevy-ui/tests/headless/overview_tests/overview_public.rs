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

/// The hero's exit facts are another projection consumer, not a second
/// business source: the same typed public-IP refresh updates them in place.
#[test]
fn overview_hero_exit_facts_restamp_with_public_ip_snapshot() {
    let mut app = mounted_default();
    let mut projection = DemoOverviewSource::running().current();
    projection.public_ip = PublicIpProbeSnapshot::failed(1, 2, "network timeout");
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<(&PublicIpText, &Text)>();
    let values: Vec<_> = texts
        .iter(world)
        .map(|(marker, text)| (marker.0, text.0.clone()))
        .collect();
    assert!(
        values
            .iter()
            .any(|(kind, text)| { *kind == PublicIpTextKind::Ip && text == "—" })
    );
    assert!(
        values
            .iter()
            .any(|(kind, text)| { *kind == PublicIpTextKind::Location && text == "—" })
    );
    assert!(
        values
            .iter()
            .any(|(kind, text)| { *kind == PublicIpTextKind::Isp && text == "—" })
    );
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

/// The hero's exit address rides the same marker as the probe card's, so the
/// page's one refresh pass restamps the banner too. A hero that keeps the
/// address it read at mount is a second, staler copy of one fact.
#[test]
fn overview_hero_exit_address_restamps_with_the_public_ip_snapshot() {
    let mut app = mounted_default();
    let banner = {
        let world = app.world_mut();
        let mut banners = world.query::<(Entity, &OverviewStatusCard)>();
        banners.iter(world).next().expect("the hero band mounts").0
    };

    let mut projection = DemoOverviewSource::running().current();
    projection.public_ip = PublicIpProbeSnapshot::failed(1, 2, "network timeout");
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let world = app.world_mut();
    let inside = descendants(world, banner);
    let mut facts = world.query::<(Entity, &PublicIpText, &Text)>();
    let hero: Vec<(PublicIpTextKind, String)> = facts
        .iter(world)
        .filter(|(entity, ..)| inside.contains(entity))
        .map(|(_, marker, text)| (marker.0, text.0.clone()))
        .collect();
    assert!(
        hero.iter()
            .any(|(kind, text)| { *kind == PublicIpTextKind::Ip && text == "—" }),
        "the hero must follow the failed probe, got {hero:?}"
    );
}
