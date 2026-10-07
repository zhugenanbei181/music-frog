//! Behavior cases for topology.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_widgets::chart::topology::TopologyPlate;
use infiltrator_contract::traffic_topology::{TrafficTopologySnapshot, TrafficTopologyStage};
use infiltrator_shared::locales::{Lang, Localizer};

/// The Overview page mounts the shared five-stage traffic topology card with
/// a real widget flow plate and four connecting arrows.
#[test]
fn test_topology_chain_card_mounts_with_five_stages_and_flow_plate() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut query = world.query::<(Entity, &TopologyChainCard)>();
    let (card_entity, _) = query
        .iter(world)
        .next()
        .expect("TopologyChainCard must be mounted in overview page");

    let all_descendants = descendants(world, card_entity);
    let texts: Vec<String> = all_descendants
        .iter()
        .filter_map(|e| world.get::<Text>(*e).map(|t| t.0.clone()))
        .collect();

    // Card title & badge
    assert!(
        texts
            .iter()
            .any(|t| t == Lang("zh-CN").tr("overview_topology_title").as_ref()),
        "card contains title"
    );
    assert!(
        texts.iter().any(|t| t == "12 连接 · flowing"),
        "card contains connection count badge"
    );

    // Stage 1: Client / Inbound
    assert!(texts.iter().any(|t| t == "Client / Inbound"));
    assert!(texts.iter().any(|t| t == "Mixed :7890"));
    assert!(texts.iter().any(|t| t == "12 conns"));

    // Stage 2: Sniffer
    assert!(texts.iter().any(|t| t == "Sniffer"));
    assert!(texts.iter().any(|t| t == "enabled"));
    assert!(texts.iter().any(|t| t == "On"));

    // Stage 3: RuleSet
    assert!(texts.iter().any(|t| t == "RuleSet"));
    assert!(texts.iter().any(|t| t == "MRS / GeoIP"));

    // Stage 4: Proxy Group
    assert!(texts.iter().any(|t| t == "Proxy Group"));
    assert!(texts.iter().any(|t| t == "GLOBAL / PROXIES"));

    // Stage 5: Outbound Node
    assert!(texts.iter().any(|t| t == "Outbound Node"));
    assert!(texts.iter().any(|t| t == "香港 01 · BGP 专线"));

    // Connecting arrows
    let arrow_count = texts.iter().filter(|t| t.as_str() == ">").count();
    assert_eq!(
        arrow_count, 4,
        "must have exactly 4 connecting arrows between the 5 stages"
    );

    let mut plates = world.query::<&TopologyPlate>();
    let plate = plates.single(world).expect("topology flow plate mounted");
    assert_eq!(plate.0.nodes.len(), 5);
    assert_eq!(plate.0.links.len(), 4);
    assert!(plate.0.links.iter().all(|link| link.highlighted));

    // Standalone scene creation test
    let palette = UiPalette::new(&Theme::dark());
    let _scene = topology_chain_scene(&palette);
}

#[test]
fn topology_projection_updates_text_and_flow_plate_in_place() {
    let mut app = mounted_default();
    let plate_id = {
        let world = app.world_mut();
        let mut ids = world.query::<(Entity, &TopologyPlate)>();
        ids.single(world).expect("topology plate entity").0
    };

    let mut topology = TrafficTopologySnapshot::demo_fixture();
    topology.active_connections = 2;
    topology.flow_bps = 2_048.0;
    topology.nodes[0].detail = "TUN · gvisor".to_owned();
    for link in &mut topology.links {
        link.active_connections = 2;
        link.flow_bps = 2_048.0;
    }
    let mut projection = DemoOverviewSource::running().current();
    projection.active_connections = 2;
    projection.traffic_topology = topology;
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let world = app.world_mut();
    assert!(
        world.get_entity(plate_id).is_ok(),
        "flow plate keeps its entity"
    );
    let mut details = world.query::<(&TopologyText, &Text)>();
    assert!(details.iter(world).any(|(marker, text)| {
        marker.stage == TrafficTopologyStage::Inbound
            && marker.kind == TopologyTextKind::Detail
            && text.0 == "TUN · gvisor"
    }));
    let plate = world
        .get::<TopologyPlate>(plate_id)
        .expect("topology plate survives");
    assert_eq!(plate.0.links.len(), 4);
    assert!(plate.0.links.iter().all(|link| link.active_conns == 2));
}

#[test]
fn topology_stage_activation_uses_shared_navigation_targets() {
    let targets = [
        (TrafficTopologyStage::Inbound, Route::Settings),
        (TrafficTopologyStage::Sniffer, Route::Settings),
        (TrafficTopologyStage::RuleSet, Route::Rules),
        (TrafficTopologyStage::ProxyGroup, Route::Proxies),
    ];
    for (stage, target) in targets {
        let mut app = mounted_default();
        let button = {
            let world = app.world_mut();
            let mut buttons = world.query::<(Entity, &TopologyStageButton)>();
            buttons
                .iter(world)
                .find(|(_, button)| button.stage == stage && button.enabled)
                .expect("enabled topology stage button")
                .0
        };
        app.world_mut()
            .commands()
            .trigger(Activate { entity: button });
        app.update();
        assert_eq!(page_root(app.world_mut()).1, target);
    }

    let mut unavailable = mounted_app_with(StubSource);
    let button = {
        let world = unavailable.world_mut();
        let mut buttons = world.query::<(Entity, &TopologyStageButton)>();
        buttons
            .iter(world)
            .find(|(_, button)| !button.enabled)
            .expect("unavailable topology stage is gated")
            .0
    };
    unavailable
        .world_mut()
        .commands()
        .trigger(Activate { entity: button });
    unavailable.update();
    assert_eq!(page_root(unavailable.world_mut()).1, Route::Overview);
}
