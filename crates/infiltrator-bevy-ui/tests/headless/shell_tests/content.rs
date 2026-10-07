//! Behavior cases for content.
//! test-intent: behavior

use super::*;

#[test]
fn content_slot_is_a_component_marker() {
    fn assert_component<T: Component>() {}
    assert_component::<ContentSlot>();
}

#[test]
fn content_title_syncs_with_active_route() {
    let mut app = mounted_shell();

    // Default title is "核心概览"
    {
        let world = app.world_mut();
        let mut titles = world.query::<(&Text, &ContentTitleLabel)>();
        let (text, _) = titles.single(world).expect("title text");
        assert_eq!(text.0, "核心概览");
    }

    // Set active route to Proxies
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Proxies)));
    app.update();

    {
        let world = app.world_mut();
        let mut titles = world.query::<(&Text, &ContentTitleLabel)>();
        let (text, _) = titles.single(world).expect("title text");
        assert_eq!(text.0, "代理节点");
    }

    // Set active route to Settings
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Settings)));
    app.update();

    {
        let world = app.world_mut();
        let mut titles = world.query::<(&Text, &ContentTitleLabel)>();
        let (text, _) = titles.single(world).expect("title text");
        assert_eq!(text.0, "系统设置");
    }
}
