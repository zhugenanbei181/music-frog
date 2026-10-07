//! Behavior cases for bottom.
//! test-intent: behavior

use super::*;

#[test]
fn bottom_nav_carries_named_button_semantics() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut items = world.query::<(&BottomNavItem, &AccessibilityNode)>();
    let mut labels: Vec<(String, bool)> = Vec::new();
    for (_, node) in items.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        labels.push((
            node.label().expect("bottom nav label").to_owned(),
            node.is_disabled(),
        ));
    }
    assert_eq!(
        labels,
        vec![
            (Route::Overview.label().to_owned(), false),
            (Route::Proxies.label().to_owned(), false),
            (Route::Profiles.label().to_owned(), false),
            (Route::Settings.label().to_owned(), false),
        ],
        "bottom nav carries 4 clean button semantics matching the 4 key routes"
    );
}

#[test]
fn bottom_nav_renders_four_items_and_click_activates() {
    let mut app = mounted_shell();

    // 4 items exist
    {
        let world = app.world_mut();
        let mut items = world.query::<&BottomNavItem>();
        let routes: Vec<Route> = items.iter(world).map(|i| i.0).collect();
        assert_eq!(
            routes,
            vec![
                Route::Overview,
                Route::Proxies,
                Route::Profiles,
                Route::Settings
            ]
        );
    }

    // Activate Profiles
    let profiles_entity = {
        let world = app.world_mut();
        let mut items = world.query::<(Entity, &BottomNavItem)>();
        items
            .iter(world)
            .find(|(_, item)| item.0 == Route::Profiles)
            .expect("profiles bottom nav item")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: profiles_entity,
    });
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Profiles)));
    app.update();

    // Active marker updated
    {
        let world = app.world_mut();
        let mut items = world.query::<(&BottomNavItem, &BottomNavActive)>();
        for (item, active) in items.iter(world) {
            if item.0 == Route::Profiles {
                assert!(active.0);
            } else {
                assert!(!active.0);
            }
        }
    }
}
