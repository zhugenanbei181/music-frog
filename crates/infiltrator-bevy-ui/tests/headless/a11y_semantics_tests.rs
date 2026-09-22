//! Headless tests for the shared accessibility grammar on the Bevy surface
//! (DUAL-15-10): the real `AccessibilityNode` roles/labels mounted in the
//! shell, the overlays, and the switch states.

use bevy::MinimalPlugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::world::World;
use bevy::scene::ScenePlugin;
use infiltrator_bevy_ui::a11y::{accesskit_role, semantic_node, switch_node, value_node};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command_palette::ToggleCommandPalette;
use infiltrator_bevy_ui::mini_hud::ToggleMiniHud;
use infiltrator_bevy_ui::toast::ShellToast;
use infiltrator_contract::a11y::{A11yRole, ShellA11yNode};

/// Everything the mounted shell publishes: role, label and (for switches) the
/// announced state.
fn published_nodes(
    world: &mut World,
) -> Vec<(accesskit::Role, String, Option<accesskit::Toggled>)> {
    let mut query = world.query::<&AccessibilityNode>();
    query
        .iter(world)
        .map(|node| {
            (
                node.0.role(),
                node.0.label().unwrap_or_default().to_string(),
                node.0.toggled(),
            )
        })
        .collect()
}

fn mounted_shell() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(
        infiltrator_contract::theme::ThemePreference::Fixed(
            infiltrator_contract::theme::ThemeSkin::Dark,
        ),
    ));
    app.update();
    app
}

/// Every shared grammar row, mounted: the shell itself plus the three overlays
/// (Mini HUD, command palette, toast region).
fn mounted_everything() -> App {
    let mut app = mounted_shell();
    app.world_mut().commands().trigger(ToggleMiniHud);
    app.world_mut().commands().trigger(ToggleCommandPalette);
    app.world_mut()
        .write_message(ShellToast::info("shell mounted for a11y coverage"));
    app.update();
    app.update();
    app
}

#[test]
fn the_mounted_shell_publishes_every_shared_semantic_row() {
    use infiltrator_contract::a11y::shell_a11y_specs;

    let mut app = mounted_everything();
    let nodes = published_nodes(app.world_mut());
    for spec in shell_a11y_specs() {
        let role = accesskit_role(spec.role);
        let found = nodes
            .iter()
            .find(|(node_role, label, _)| *node_role == role && label == spec.label_zh);
        assert!(
            found.is_some(),
            "{:?} must mount as {role:?} with the shared label {:?}; mounted labels: {:?}",
            spec.node,
            spec.label_zh,
            nodes.iter().map(|(_, label, _)| label).collect::<Vec<_>>()
        );
        if spec.node.is_switch() {
            let (_, _, toggled) = found.expect("switch node found above");
            assert!(
                toggled.is_some(),
                "{:?} is a switch and must announce its state",
                spec.node
            );
        }
    }
}

#[test]
fn a_switch_node_announces_its_live_state_and_a_status_node_its_value() {
    let on = switch_node(ShellA11yNode::SystemProxySwitch, true);
    assert_eq!(on.0.role(), accesskit::Role::Switch);
    assert_eq!(on.0.label(), Some("系统代理开关"));
    assert_eq!(on.0.toggled(), Some(accesskit::Toggled::True));

    let off = switch_node(ShellA11yNode::TunSwitch, false);
    assert_eq!(off.0.role(), accesskit::Role::Switch);
    assert_eq!(off.0.toggled(), Some(accesskit::Toggled::False));

    let readout = value_node(ShellA11yNode::TrafficReadout, "↑ 1.2 MB/s");
    assert_eq!(readout.0.role(), accesskit::Role::Label);
    assert_eq!(readout.0.label(), Some("上下行实时速率: ↑ 1.2 MB/s"));

    let dialog = semantic_node(ShellA11yNode::CommandPaletteDialog);
    assert_eq!(dialog.0.role(), accesskit::Role::Dialog);
    assert_eq!(dialog.0.label(), Some("命令面板"));
}

#[test]
fn every_shared_role_maps_onto_a_real_accesskit_role() {
    // The shared inventory is authoritative: adding a role there must be
    // translated by the Bevy mapping or this test fails.
    for role in A11yRole::ALL {
        let mapped = accesskit_role(role);
        assert_ne!(mapped, accesskit::Role::Unknown, "{role:?} must map");
    }
    assert_eq!(accesskit_role(A11yRole::Switch), accesskit::Role::Switch);
    assert_eq!(accesskit_role(A11yRole::Text), accesskit::Role::Label);
}

/// The palette's live query line is a real `Text` grammar row mounted on the
/// node that shows the query, so a screen reader hears the search phrase
/// (DUAL-15-10/11).
#[test]
fn the_command_palette_query_line_publishes_the_shared_text_row() {
    use bevy::ecs::query::With;
    use infiltrator_bevy_ui::command_palette::CommandPaletteQueryLabel;

    let mut app = mounted_everything();
    let world = app.world_mut();
    let mut query = world.query_filtered::<&AccessibilityNode, With<CommandPaletteQueryLabel>>();
    let nodes: Vec<&AccessibilityNode> = query.iter(world).collect();
    assert_eq!(nodes.len(), 1, "one query line per mounted palette");
    assert_eq!(nodes[0].0.role(), accesskit::Role::Label);
    assert_eq!(nodes[0].0.label(), Some("命令面板查询词"));
}
