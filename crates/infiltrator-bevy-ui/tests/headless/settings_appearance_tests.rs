//! test-intent: behavior
//! The Settings appearance card is the shell's only live appearance control.
//!
//! The rail used to carry a cycling theme pill and a density pill in its
//! footer while the settings page's own theme strip was decorative (a
//! hardcoded four-skin list, a fixed selected index, nothing dispatched). Both
//! halves are adjudicated here: the card selects from the contract's
//! `ThemePreference::ALL` and the widget layer's `Density::ALL`, it writes the
//! owner, and the official `TabList` selection follows the owner back rather
//! than the other way round.

use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::world::World;
use bevy::ui_widgets::{Activate, SelectedTab, ValueChange};
use infiltrator_bevy_ui::app::{ShellPlugin, SidebarPanel};
use infiltrator_bevy_ui::appearance::ThemeMode;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::pages::settings::settings_appearance::{
    AppearanceChip, AppearanceStrip, DensityChip, DensityStrip,
};
use infiltrator_bevy_ui::pages::settings::settings_index::SettingsIndexButton;
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::localization::LocalizedLabel;
use infiltrator_bevy_widgets::responsive::{Density, ResponsiveContext};
use infiltrator_contract::shell_navigation::pages_folded_under;
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use std::sync::Arc;

fn mounted_settings(sink: Arc<DemoCommandSink>) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ShellPlugin::default(),
        PagesPlugin::demo(),
        CommandPumpPlugin::new(sink),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    app.update();
    app
}

/// The chip of one theme option, found through the strip the card mounts.
fn theme_chip(app: &mut App, preference: ThemePreference) -> Entity {
    app.world_mut()
        .query::<(Entity, &AppearanceChip)>()
        .iter(app.world())
        .find(|(_, chip)| chip.0 == preference)
        .unwrap_or_else(|| panic!("the appearance card offers {preference:?}"))
        .0
}

fn density_chip(app: &mut App, density: Density) -> Entity {
    app.world_mut()
        .query::<(Entity, &DensityChip)>()
        .iter(app.world())
        .find(|(_, chip)| chip.0 == density)
        .unwrap_or_else(|| panic!("the appearance card offers {density:?}"))
        .0
}

fn selected(app: &mut App, strip: Entity) -> Option<Entity> {
    app.world().get::<SelectedTab>(strip).unwrap().0
}

/// The theme strip's own entity: the official `TabList` the card mounts.
fn theme_strip(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<AppearanceStrip>>()
        .single(app.world())
        .expect("one appearance strip")
}

fn density_strip(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<DensityStrip>>()
        .single(app.world())
        .expect("one density strip")
}

/// The options one strip offers, read in the order the strip paints them.
///
/// The strip's child list *is* the option table the user reads left to right;
/// an ECS query iterates archetypes and answers in storage order, which is how
/// this case once compared `[System, Amoled, Dark, Light, Forest]` against the
/// contract's `[System, Dark, Light, Forest, Amoled]`. The claim is about the
/// mounted sequence, so the read has to descend the tree.
fn offered_themes(app: &mut App, strip: Entity) -> Vec<ThemePreference> {
    let world = app.world();
    children_of(world, strip)
        .into_iter()
        .filter_map(|child| world.get::<AppearanceChip>(child).map(|chip| chip.0))
        .collect()
}

fn offered_densities(app: &mut App, strip: Entity) -> Vec<Density> {
    let world = app.world();
    children_of(world, strip)
        .into_iter()
        .filter_map(|child| world.get::<DensityChip>(child).map(|chip| chip.0))
        .collect()
}

fn children_of(world: &World, strip: Entity) -> Vec<Entity> {
    world
        .get::<Children>(strip)
        .expect("the strip mounts its chips as children")
        .iter()
        .copied()
        .collect()
}

#[test]
fn the_appearance_card_offers_every_preference_and_names_it_once() {
    let mut app = mounted_settings(Arc::new(DemoCommandSink::accepting()));

    let strip = theme_strip(&mut app);
    let offered = offered_themes(&mut app, strip);
    assert_eq!(
        offered,
        ThemePreference::ALL.to_vec(),
        "the card renders the contract's option table, in its order, not a surface-side list"
    );

    let strip = density_strip(&mut app);
    assert_eq!(
        offered_densities(&mut app, strip),
        Density::ALL.to_vec(),
        "the density strip renders the widget layer's table, in its order"
    );

    // Each chip is labelled by the same key it dispatches, so the strip never
    // says one thing while writing another.
    let world = app.world_mut();
    let mut labels = world.query::<(&AppearanceChip, &LocalizedLabel)>();
    for (chip, label) in labels.iter(world) {
        assert_eq!(
            label.0.key,
            chip.0.label_key(),
            "{:?} is named by its own contract key",
            chip.0
        );
    }
    let mut density_labels = world.query::<(&DensityChip, &LocalizedLabel)>();
    for (chip, label) in density_labels.iter(world) {
        assert_eq!(label.0.key, chip.0.label_key());
    }
}

#[test]
fn choosing_a_preference_paints_it_writes_the_owner_and_persists() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_settings(Arc::clone(&sink));

    let strip = theme_strip(&mut app);
    // Cold start follows the OS, and the strip mirrors the owner from frame one.
    assert_eq!(
        app.world().resource::<ThemeMode>().0,
        ThemePreference::System
    );
    let seeded = selected(&mut app, strip).expect("the strip carries a selection");
    assert_eq!(
        app.world().get::<AppearanceChip>(seeded).unwrap().0,
        ThemePreference::System
    );

    let forest = theme_chip(&mut app, ThemePreference::Fixed(ThemeSkin::Forest));
    app.world_mut().commands().trigger(ValueChange {
        source: strip,
        value: Some(forest),
        is_final: true,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ThemeMode>().0,
        ThemePreference::Fixed(ThemeSkin::Forest),
        "the card is the owner's writer, not a second reader"
    );
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateSetting {
            key: "theme".to_owned(),
            value: "forest".to_owned(),
        }],
        "the choice is written through the settings owner, so it survives restart"
    );
    assert_eq!(
        selected(&mut app, strip),
        Some(forest),
        "the official selection follows the owner's answer"
    );

    // Choosing what is already chosen restates nothing.
    app.world_mut().commands().trigger(ValueChange {
        source: strip,
        value: Some(forest),
        is_final: true,
    });
    app.update();
    assert_eq!(
        sink.submitted().len(),
        1,
        "a no-op choice dispatches no write"
    );
}

#[test]
fn a_chip_activated_directly_selects_the_same_preference() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_settings(Arc::clone(&sink));
    let amoled = theme_chip(&mut app, ThemePreference::Fixed(ThemeSkin::Amoled));
    app.world_mut()
        .commands()
        .trigger(Activate { entity: amoled });
    app.update();
    assert_eq!(
        app.world().resource::<ThemeMode>().0,
        ThemePreference::Fixed(ThemeSkin::Amoled)
    );
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateSetting {
            key: "theme".to_owned(),
            value: "amoled".to_owned(),
        }]
    );
}

#[test]
fn choosing_a_density_writes_the_responsive_owner_only() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_settings(Arc::clone(&sink));

    let strip = density_strip(&mut app);
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Comfortable
    );

    let compact = density_chip(&mut app, Density::Compact);
    app.world_mut().commands().trigger(ValueChange {
        source: strip,
        value: Some(compact),
        is_final: true,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Compact,
        "the shared widget observer is the one writer of density"
    );
    assert_eq!(
        selected(&mut app, strip),
        Some(compact),
        "the strip follows the owner, so a refused request cannot leave a stale chip lit"
    );
    assert!(
        sink.submitted().is_empty(),
        "density is a presentation preference; it writes no settings value twice"
    );
}

#[test]
fn the_settings_index_reaches_every_folded_page() {
    let mut app = mounted_settings(Arc::new(DemoCommandSink::accepting()));
    let world = app.world_mut();
    let folded = pages_folded_under(infiltrator_contract::command_catalogue::ShellPage::Settings);
    let mut chips = world.query::<&SettingsIndexButton>();
    let offered: Vec<Route> = chips.iter(world).map(|chip| chip.0).collect();
    assert_eq!(offered.len(), folded.len(), "one chip per folded page");
    for page in &folded {
        assert!(
            offered.contains(&Route::from_shell_page(*page)),
            "{page:?} must be reachable from the settings index"
        );
    }
}

#[test]
fn an_index_chip_navigates_and_the_rail_keeps_settings_current() {
    let mut app = mounted_settings(Arc::new(DemoCommandSink::accepting()));
    let dns = app
        .world_mut()
        .query::<(Entity, &SettingsIndexButton)>()
        .iter(app.world())
        .find(|(_, chip)| chip.0 == Route::Dns)
        .expect("the index offers DNS")
        .0;
    app.world_mut().commands().trigger(Activate { entity: dns });
    app.update();
    app.update();

    assert_eq!(app.world().resource::<ActiveRoute>().0, Some(Route::Dns));
    // Folding is the contract's answer, so the rail lights Settings rather
    // than going silent on a page that has no row of its own.
    let mut rows = app.world_mut().query::<(
        &infiltrator_bevy_ui::app::SidebarNavItem,
        &infiltrator_bevy_widgets::nav::NavActive,
    )>();
    let lit: Vec<Route> = rows
        .iter(app.world())
        .filter(|(_, active)| active.0)
        .map(|(item, _)| item.0)
        .collect();
    assert_eq!(lit, vec![Route::Settings]);
}

#[test]
fn the_rail_mounts_no_appearance_or_density_control() {
    let mut app = mounted_settings(Arc::new(DemoCommandSink::accepting()));
    let world = app.world_mut();
    let rail = world
        .query::<(Entity, &SidebarPanel)>()
        .single(world)
        .expect("one rail")
        .0;

    let mut subtree: Vec<Entity> = Vec::new();
    let mut stack = vec![rail];
    while let Some(entity) = stack.pop() {
        subtree.push(entity);
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }
    let selectors = subtree
        .iter()
        .filter(|entity| {
            world.get::<AppearanceChip>(**entity).is_some()
                || world.get::<DensityChip>(**entity).is_some()
        })
        .count();
    assert_eq!(
        selectors, 0,
        "appearance is a settings preference; the navigation column carries no selector"
    );
}
