//! Behavior cases for overview topology.
//! test-intent: behavior

use super::*;

/// UI-04-05: Overview topology hover activates full-chain highlight and
/// activation penetrates with filter query down to target page.
#[test]
fn overview_topology_hover_chain_highlight_and_activation_drilldown() {
    let mut app = mounted_default();

    // 1. Verify TopologyPlate is mounted and initially has no hovered stage.
    let plate_entity = {
        let world = app.world_mut();
        let mut plates = world.query::<(Entity, &TopologyPlate)>();
        let (entity, plate) = plates.single(world).expect("mounted topology plate");
        assert_eq!(plate.0.hovered_stage, None);
        entity
    };

    // 2. Find RuleSet stage button.
    let ruleset_button_entity = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &TopologyStageButton)>();
        buttons
            .iter(world)
            .find(|(_, btn)| btn.stage == TrafficTopologyStage::RuleSet && btn.enabled)
            .expect("enabled ruleset stage button")
            .0
    };

    // 3. Hover the RuleSet button and verify plate hovered_stage is updated.
    app.world_mut()
        .entity_mut(ruleset_button_entity)
        .insert(PickingInteraction::Hovered);
    app.update();

    {
        let world = app.world_mut();
        let plate = world
            .get::<TopologyPlate>(plate_entity)
            .expect("plate survives");
        assert_eq!(plate.0.hovered_stage.as_deref(), Some("rule_set"));
    }

    // 4. Unhover and verify plate hovered_stage returns to None.
    app.world_mut()
        .entity_mut(ruleset_button_entity)
        .insert(PickingInteraction::None);
    app.update();

    {
        let world = app.world_mut();
        let plate = world
            .get::<TopologyPlate>(plate_entity)
            .expect("plate survives");
        assert_eq!(plate.0.hovered_stage, None);
    }

    // 5. Activate the RuleSet button: the drilldown opens on the Overview page.
    app.world_mut().commands().trigger(Activate {
        entity: ruleset_button_entity,
    });
    app.update();

    // The business shows its result on its own page: the route never leaves
    // the Overview page.
    assert_eq!(page_root(app.world_mut()).1, Route::Overview);

    // Verify TopologyDrilldownFilter resource contains RuleSet stage and demo detail query
    let drilldown = app.world().resource::<TopologyDrilldownFilter>();
    assert_eq!(drilldown.stage, Some(TrafficTopologyStage::RuleSet));
    assert_eq!(drilldown.filter_query.as_deref(), Some("MRS / GeoIP"));

    // The drilldown panel is visible and renders the stage's own result.
    {
        let world = app.world_mut();
        let mut panels = world.query::<(&TopologyDrilldownPanel, &Node)>();
        let (_, panel) = panels
            .single(world)
            .expect("the drilldown panel is mounted");
        assert_eq!(panel.display, Display::Flex);
    }
    {
        let world = app.world_mut();
        let mut texts = world.query::<(&TopologyDrilldownText, &Text)>();
        let (_, text) = texts.single(world).expect("the drilldown result line");
        assert!(
            text.0.contains("RuleSet"),
            "the drilldown shows the activated stage, got {}",
            text.0
        );
    }
    {
        let world = app.world_mut();
        let mut texts = world.query::<(&TopologyDrilldownKeywordText, &Text)>();
        let (_, text) = texts.single(world).expect("the drilldown keyword line");
        assert_eq!(text.0, "MRS / GeoIP");
    }
}
