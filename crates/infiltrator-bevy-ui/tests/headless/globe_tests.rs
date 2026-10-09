//! test-intent: behavior
//!
//! Headless behavior tests for the Globe page (BEVY-030 / BANDROID-016): the
//! page mounts, real known geo plots markers/arcs, unknown or country-only
//! geo renders the typed empty state, the mode toggle restamps the plate, and
//! the page never fabricates a coordinate for an unknown node.

use crate::support::{headless_plugins, subtree_has_text};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::scene::{CommandsSceneExt, Scene};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::globe::{
    GlobeData, GlobeDataUpdated, GlobeEmptyState, GlobeModeButton, GlobePagePlugin, GlobePageRoot,
    GlobeViewState, LastGlobeData, globe_page, globe_unknown_detail,
};
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route};
use infiltrator_bevy_widgets::globe::{
    GeoLocation, GeoPoint, GeoPrecision, GeoSource, GeoUnknown, GlobeLink, GlobeMode, GlobeNode,
    GlobeNodeCategory, GlobePlate, GlobeProjection, build_globe_arcs, project_flat_nodes,
    project_globe_nodes,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

fn city_point(lat: f64, lon: f64) -> GeoPoint {
    GeoPoint::new(lat, lon, GeoSource::GeoIpDatabase, GeoPrecision::City).expect("valid city point")
}

fn node(id: &str, lat: f64, lon: f64, category: GlobeNodeCategory) -> GlobeNode {
    GlobeNode::new(id, id, GeoLocation::Known(city_point(lat, lon)), category)
}

fn page_app(palette: UiPalette) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.insert_resource(palette);
    app.add_plugins(GlobePagePlugin);
    app
}

fn mount(app: &mut App, scene: impl Scene) {
    app.world_mut().commands().spawn_scene(scene);
    app.update();
    app.update();
}

fn globe_root(app: &mut App) -> Entity {
    let mut roots = app.world_mut().query::<(Entity, &GlobePageRoot)>();
    roots
        .iter(app.world())
        .next()
        .expect("globe page root mounted")
        .0
}

fn first_plate(app: &mut App) -> GlobePlate {
    let mut plates = app.world_mut().query::<&GlobePlate>();
    plates
        .iter(app.world())
        .next()
        .expect("globe plate mounted")
        .clone()
}

/// The page mounts its root and, for a plottable projection, a globe plate.
#[test]
fn globe_page_mounts_with_a_plate_for_known_geo() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = page_app(palette);
    let data = GlobeData::new(
        vec![
            node("london", 51.5, -0.1, GlobeNodeCategory::Home),
            node("paris", 48.8, 2.3, GlobeNodeCategory::Relay),
        ],
        vec![GlobeLink::new("london", "paris", 5_000_000.0)],
    );
    mount(&mut app, globe_page(&data, GlobeMode::Flat2d, &palette));

    let root = globe_root(&mut app);
    assert!(
        app.world().get_entity(root).is_ok(),
        "the page root is mounted"
    );
    let plate = first_plate(&mut app);
    assert_eq!(plate.0.nodes.len(), 2);
    assert_eq!(plate.0.links.len(), 1);
}

/// Real known geo plots markers on both the sphere and the lossless flat map,
/// and a link between two known nodes produces an arc.
#[test]
fn known_geo_renders_markers_and_arcs() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = page_app(palette);
    let data = GlobeData::new(
        vec![
            node("london", 51.5, -0.1, GlobeNodeCategory::Home),
            node("paris", 48.8, 2.3, GlobeNodeCategory::Relay),
        ],
        vec![GlobeLink::new("london", "paris", 5_000_000.0)],
    );
    mount(&mut app, globe_page(&data, GlobeMode::Flat2d, &palette));
    let plate = first_plate(&mut app);

    assert_eq!(
        project_flat_nodes(&plate.0).len(),
        2,
        "the flat fallback is lossless: every known node plots"
    );
    let projection = GlobeProjection::from_spec(&plate.0);
    assert_eq!(
        project_globe_nodes(&plate.0, &projection).len(),
        2,
        "both front-hemisphere known nodes plot on the sphere"
    );
    assert_eq!(build_globe_arcs(&plate.0).len(), 1);
}

/// Unknown and country-only locations render the explicit typed empty state,
/// with no plate and no fabricated point.
#[test]
fn unknown_geo_shows_the_typed_empty_state() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = page_app(palette);
    let country = GlobeNode::new(
        "country",
        "Country",
        GeoLocation::Known(
            GeoPoint::new(10.0, 10.0, GeoSource::GeoIpDatabase, GeoPrecision::Country)
                .expect("in-range country point"),
        ),
        GlobeNodeCategory::Exit,
    );
    let unknown = GlobeNode::new(
        "unknown",
        "Unknown",
        GeoLocation::Unknown(GeoUnknown::NotObserved),
        GlobeNodeCategory::Relay,
    );
    let data = GlobeData::new(
        vec![country, unknown],
        vec![GlobeLink::new("country", "unknown", 1_000_000.0)],
    );
    mount(&mut app, globe_page(&data, GlobeMode::Globe, &palette));

    let root = globe_root(&mut app);
    assert!(
        first_plate_opt(&mut app).is_none(),
        "no globe plate is mounted for a non-plottable projection"
    );
    assert!(subtree_has_text(
        app.world(),
        root,
        "No plottable node location"
    ));
    assert!(subtree_has_text(app.world(), root, "country-level only"));
    assert_eq!(
        globe_unknown_detail(&data),
        "country-level only, not observed"
    );
}

fn first_plate_opt(app: &mut App) -> Option<GlobePlate> {
    let mut plates = app.world_mut().query::<&GlobePlate>();
    plates.iter(app.world()).next().cloned()
}

/// The mode toggle restamps the mounted plate in place and persists in the
/// page-local view state. Eco is the minimal degradation: no link geometry.
#[test]
fn mode_toggle_restamps_the_plate() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = page_app(palette);
    let data = GlobeData::new(
        vec![
            node("london", 51.5, -0.1, GlobeNodeCategory::Home),
            node("paris", 48.8, 2.3, GlobeNodeCategory::Relay),
        ],
        vec![GlobeLink::new("london", "paris", 5_000_000.0)],
    );
    mount(&mut app, globe_page(&data, GlobeMode::Globe, &palette));
    assert_eq!(first_plate(&mut app).0.mode, GlobeMode::Globe);

    let eco = {
        let mut buttons = app.world_mut().query::<(Entity, &GlobeModeButton)>();
        buttons
            .iter(app.world())
            .find(|(_, button)| button.0 == GlobeMode::Eco)
            .expect("eco mode button mounted")
            .0
    };
    app.world_mut().commands().trigger(Activate { entity: eco });
    app.update();

    let plate = first_plate(&mut app);
    assert_eq!(plate.0.mode, GlobeMode::Eco);
    assert!(!plate.0.mode.draws_links(), "Eco degrades link geometry");
    assert!(!plate.0.mode.animates(), "Eco disables animation");
    assert_eq!(
        app.world().resource::<GlobeViewState>().0,
        GlobeMode::Eco,
        "the selected mode persists in the page state"
    );
}

/// A shared projection update restamps the mounted plate in place.
#[test]
fn shared_projection_update_restamps_the_plate() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = page_app(palette);
    let first = GlobeData::new(
        vec![node("london", 51.5, -0.1, GlobeNodeCategory::Home)],
        Vec::new(),
    );
    mount(&mut app, globe_page(&first, GlobeMode::Flat2d, &palette));
    let updated = GlobeData::new(
        vec![
            node("london", 51.5, -0.1, GlobeNodeCategory::Home),
            node("paris", 48.8, 2.3, GlobeNodeCategory::Relay),
        ],
        vec![GlobeLink::new("london", "paris", 5_000_000.0)],
    );
    app.world_mut()
        .commands()
        .trigger(GlobeDataUpdated(updated.clone()));
    app.update();

    assert_eq!(first_plate(&mut app).0.nodes.len(), 2);
    assert_eq!(
        app.world().resource::<LastGlobeData>().0.as_ref(),
        Some(&updated)
    );
}

/// An unknown node never becomes a marker and never gains a coordinate; the
/// range-validating constructor is the only point entry.
#[test]
fn page_never_fabricates_a_coordinate() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = page_app(palette);
    let data = GlobeData::new(
        vec![
            node("london", 51.5, -0.1, GlobeNodeCategory::Home),
            GlobeNode::new(
                "unknown",
                "Unknown",
                GeoLocation::Unknown(GeoUnknown::SourceEmpty),
                GlobeNodeCategory::Reject,
            ),
        ],
        Vec::new(),
    );
    mount(&mut app, globe_page(&data, GlobeMode::Flat2d, &palette));
    let plate = first_plate(&mut app);

    let unknown = plate
        .0
        .nodes
        .iter()
        .find(|node| node.id == "unknown")
        .expect("unknown node carried honestly");
    assert!(matches!(
        unknown.location,
        GeoLocation::Unknown(GeoUnknown::SourceEmpty)
    ));
    assert!(unknown.location.plottable().is_none());
    assert_eq!(
        project_flat_nodes(&plate.0).len(),
        1,
        "only the known node is plotted"
    );
    let projection = GlobeProjection::from_spec(&plate.0);
    assert_eq!(project_globe_nodes(&plate.0, &projection).len(), 1);
    assert!(
        GeoPoint::new(120.0, 0.0, GeoSource::Configured, GeoPrecision::City).is_none(),
        "out-of-range coordinates cannot exist"
    );
}

/// The product shell has no standalone Globe route, so the page is wired as
/// the Overview route's geo section: it mounts under the Overview page root
/// and honestly shows the typed not-observed state by default.
#[test]
fn overview_route_mounts_the_globe_section() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.update();
    app.update();

    let globe_root = {
        let mut roots = app.world_mut().query::<(Entity, &GlobePageRoot)>();
        roots
            .iter(app.world())
            .next()
            .expect("globe section mounted")
            .0
    };
    let overview_root = {
        let mut roots = app.world_mut().query::<(Entity, &PageRoot)>();
        roots
            .iter(app.world())
            .find(|(_, root)| root.0 == Route::Overview)
            .expect("overview page root mounted")
            .0
    };
    assert_eq!(
        app.world()
            .get::<ChildOf>(globe_root)
            .expect("globe section parented")
            .0,
        overview_root,
        "the globe section is a child of the Overview page root"
    );
    assert!(
        first_plate_opt(&mut app).is_none(),
        "the default projection is not observed: no fabricated plate"
    );
    assert!(
        {
            let mut states = app.world_mut().query::<&GlobeEmptyState>();
            states.iter(app.world()).next().is_some()
        },
        "the default projection renders the typed empty state"
    );
}
