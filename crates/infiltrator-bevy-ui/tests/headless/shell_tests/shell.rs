//! Behavior cases for shell.
//! test-intent: behavior

use super::*;
use infiltrator_contract::a11y::ShellA11yNode;

#[test]
fn shell_mounts_camera_content_slot_and_stamped_header() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();

    let world = app.world_mut();
    let mut cameras = world.query::<&Camera2d>();
    assert_eq!(cameras.iter(world).count(), 1, "ui camera mounted");

    let world = app.world_mut();
    let mut slots = world.query::<&ContentSlot>();
    assert_eq!(slots.iter(world).count(), 1, "exactly one content slot");

    let world = app.world_mut();
    let mut rails = world.query::<&SidebarPanel>();
    assert_eq!(rails.iter(world).count(), 1, "exactly one sidebar rail");

    let world = app.world_mut();
    let mut headers = world.query::<(&Text, &TextRole, &TextColor, &TextFont)>();
    let (_, role, ink, font) = headers
        .iter(world)
        .find(|(_, role, _, _)| role.0 == Role::Heading)
        .expect("title row carries the heading role");
    assert_eq!(role.0, Role::Heading);
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(ink.0, palette.ink, "heading ink stamped from tokens");
    assert!(
        matches!(font.font_size, FontSize::Px(size) if size == palette.heading_font_px),
        "heading size stamped from the type scale"
    );
}

#[test]
fn shell_reruns_do_not_stack_slots() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app.update();
    let world = app.world_mut();
    let mut slots = world.query::<&ContentSlot>();
    assert_eq!(slots.iter(world).count(), 1);
}

#[test]
fn shell_exposes_named_semantic_nodes_on_root_header_and_pill() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut roots = world.query::<(&AccessibilityNode, &ShellRoot)>();
    let (root, _) = roots.single(world).expect("shell root semantic node");
    // DUAL-15-10: the window/header/theme labels are the shared grammar rows.
    assert_eq!(root.role(), accesskit::Role::Window);
    assert_eq!(root.label(), Some(ShellA11yNode::Window.label_zh()));

    let mut headers = world.query::<(&AccessibilityNode, &ShellHeader)>();
    let (header, _) = headers.single(world).expect("header semantic node");
    assert_eq!(header.role(), accesskit::Role::Header);
    assert_eq!(header.label(), Some(ShellA11yNode::ShellHeader.label_zh()));

    let mut pills = world.query::<(&AccessibilityNode, &ThemeToggle)>();
    let (pill, _) = pills.single(world).expect("pill semantic node");
    assert_eq!(pill.role(), accesskit::Role::Button);
    assert_eq!(pill.label(), Some(ShellA11yNode::ThemeToggle.label_zh()));
}

#[test]
fn test_shell_header_history_and_status_indicators() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut back_query = world.query::<(Entity, &HistoryBackButton)>();
    let (back_entity, _) = back_query
        .iter(world)
        .next()
        .expect("HistoryBackButton must be mounted in header");

    let mut forward_query = world.query::<(Entity, &HistoryForwardButton)>();
    let (forward_entity, _) = forward_query
        .iter(world)
        .next()
        .expect("HistoryForwardButton must be mounted in header");

    let mut dot_query = world.query::<(Entity, &GlobalStatusDot)>();
    assert!(
        dot_query.iter(world).next().is_some(),
        "GlobalStatusDot must be mounted"
    );

    let mut mode_capsule_query = world.query::<(Entity, &GlobalModeCapsule)>();
    assert!(
        mode_capsule_query.iter(world).next().is_some(),
        "GlobalModeCapsule must be mounted"
    );

    // Trigger back button activation
    world.commands().trigger(Activate {
        entity: back_entity,
    });
    app.update();

    // Trigger forward button activation
    app.world_mut().commands().trigger(Activate {
        entity: forward_entity,
    });
    app.update();
}
