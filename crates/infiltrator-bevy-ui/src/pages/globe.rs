//! The Globe page (BEVY-030 / BANDROID-016): a 2.5D geo map of network
//! nodes over the business-agnostic [`globe`](infiltrator_bevy_widgets::globe)
//! widget.
//!
//! **Honesty contract.** This page never invents a coordinate. It only ever
//! carries a [`GeoLocation`]: [`GeoLocation::Known`] with a city/exact point
//! is plotted; [`GeoLocation::Unknown`] or a country-only observation renders
//! the explicit typed empty/unknown state and produces no marker and no arc.
//! The widget's own projection functions ([`project_globe_nodes`],
//! [`project_flat_nodes`], [`build_globe_arcs`]) enforce the same gate, so a
//! fabricated coordinate has no path into a rendered frame.
//!
//! **Mode seam.** [`GlobeMode`] (Globe / Flat2d / Eco) is exposed as three
//! buttons. Flat2d is the lossless 2D fallback (every known node visible,
//! antipodes included); Eco is the minimal degradation with no links or
//! animation. The selected mode is a page-local [`GlobeViewState`] resource
//! that outlives a mount, and it is restamped onto the mounted [`GlobePlate`]
//! in place (charter law: observers change components, never rebuild trees).
//!
//! **Product wiring.** The product shell has no separate Globe route (the
//! shared [`ShellPage`](infiltrator_contract::command_catalogue::ShellPage)
//! vocabulary and the capture route table are fixed), so this page is wired
//! as the Overview route's geo section: [`on_globe_page_root_added`] appends
//! it under the mounted Overview page root. The default
//! [`LastGlobeData`] is empty, so the product honestly shows the typed
//! "not observed" state until a real geo observation is composed.

use crate::route::{PageRoot, Route};
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::globe::{
    GeoLocation, GeoUnknown, GlobeLink, GlobeMode, GlobeNode, GlobePlate, GlobeSpec, globe_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Raster width of the mounted globe plate.
pub const GLOBE_WIDTH_PX: u32 = 640;
/// Raster height of the mounted globe plate.
pub const GLOBE_HEIGHT_PX: u32 = 400;

/// Root marker of the Globe page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobePageRoot;

/// Marker on one mode-toggle button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobeModeButton(pub GlobeMode);

/// Marker on the empty/unknown-state card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobeEmptyState;

/// The explicit reason line of the empty/unknown-state card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobeEmptyStateDetail;

/// The page-local render input: real nodes (each with a provenance-bearing
/// [`GeoLocation`]) and the directed links between them. It is deliberately
/// built from [`GlobeNode`]/[`GlobeLink`] so no coordinate can enter without a
/// [`GeoLocation`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlobeData {
    pub nodes: Vec<GlobeNode>,
    pub links: Vec<GlobeLink>,
}

impl GlobeData {
    pub fn new(nodes: Vec<GlobeNode>, links: Vec<GlobeLink>) -> Self {
        Self { nodes, links }
    }

    /// How many nodes carry a plottable (city/exact) coordinate.
    pub fn plottable_count(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| node.location.plottable().is_some())
            .count()
    }

    /// Whether any node can be plotted at all.
    pub fn has_plottable(&self) -> bool {
        self.plottable_count() > 0
    }
}

/// The typed event carrying a new shared geo projection into the page.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct GlobeDataUpdated(pub GlobeData);

/// The last projection the page rendered (page-local; default = not observed).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastGlobeData(pub Option<GlobeData>);

/// The selected rendering mode; a resource so it survives a route remount.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobeViewState(pub GlobeMode);

impl Default for GlobeViewState {
    fn default() -> Self {
        Self(GlobeMode::Globe)
    }
}

/// Build the widget spec for one projection and mode.
pub fn globe_spec(data: &GlobeData, mode: GlobeMode) -> GlobeSpec {
    let mut spec = GlobeSpec::new(
        data.nodes.clone(),
        data.links.clone(),
        GLOBE_WIDTH_PX,
        GLOBE_HEIGHT_PX,
    );
    spec.mode = mode;
    spec
}

/// The explicit, human-readable reason a node could not be plotted. Never a
/// coordinate.
fn unknown_reason_text(reason: GeoUnknown) -> &'static str {
    match reason {
        GeoUnknown::NotObserved => "not observed",
        GeoUnknown::SourceEmpty => "source returned no data",
        GeoUnknown::SourceUnavailable => "source unavailable",
        GeoUnknown::CountryOnly => "country-level only",
    }
}

/// The typed empty/unknown detail line for a projection. Empty when every
/// node is plottable.
pub fn globe_unknown_detail(data: &GlobeData) -> String {
    if data.nodes.is_empty() {
        return "No node geo observation has arrived yet.".to_owned();
    }
    let mut parts: Vec<&'static str> = Vec::new();
    for node in &data.nodes {
        let reason = match node.location {
            GeoLocation::Unknown(reason) => Some(reason),
            GeoLocation::Known(point) if !point.is_plottable() => Some(GeoUnknown::CountryOnly),
            _ => None,
        };
        if let Some(reason) = reason {
            let text = unknown_reason_text(reason);
            if !parts.contains(&text) {
                parts.push(text);
            }
        }
    }
    parts.join(", ")
}

/// The Globe page scene: header, mode toggle and either the globe plate or the
/// explicit empty/unknown card.
pub fn globe_page(data: &GlobeData, mode: GlobeMode, palette: &UiPalette) -> impl Scene + use<> {
    let spec = globe_spec(data, mode);
    let mut body: Vec<Box<dyn Scene>> = Vec::new();
    if data.has_plottable() {
        body.push(Box::new(globe_scene(spec)));
    } else {
        body.push(Box::new(empty_state_scene(data, palette)));
    }
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S12),
                padding: UiRect::all(Val::Px(space::S16)),
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
            }
            BackgroundColor({ palette.surface })
            GlobePageRoot
            Children [
                @{ header_scene(data, palette) }
                --
                @{ mode_toggle_scene(mode, palette) }
                --
                { body }
            ]
    }
}

fn header_scene(data: &GlobeData, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "{} of {} node locations are plottable",
        data.plottable_count(),
        data.nodes.len()
    );
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S4),
            }
            Children [
                Text({ "Geo Globe".to_owned() }) TextRole(Role::Heading)
                --
                Text(summary) TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    height: px(1.0),
                }
                BackgroundColor({ palette.border })
            ]
    }
}

fn mode_toggle_scene(mode: GlobeMode, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
            }
            Children [
                @{ mode_button_scene(GlobeMode::Globe, mode, "Globe", palette) }
                --
                @{ mode_button_scene(GlobeMode::Flat2d, mode, "Flat 2D", palette) }
                --
                @{ mode_button_scene(GlobeMode::Eco, mode, "Eco", palette) }
            ]
    }
}

fn mode_button_scene(
    target: GlobeMode,
    current: GlobeMode,
    label: &'static str,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let selected = target == current;
    let fill = if selected {
        palette.accent_container
    } else {
        palette.surface_elevated
    };
    let role = if selected {
        Role::BodyStrong
    } else {
        Role::Body
    };
    bsn! {
            Node {
                min_height: px(palette.control_height_px),
                padding: UiRect::horizontal(Val::Px(space::S12)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ fill })
            Button
            GlobeModeButton(target)
            Children [
                Text({ label.to_owned() }) TextRole(role)
            ]
    }
}

fn empty_state_scene(data: &GlobeData, palette: &UiPalette) -> impl Scene + use<> {
    let detail = globe_unknown_detail(data);
    bsn! {
            Node {
                width: percent(100),
                height: px(GLOBE_HEIGHT_PX as f32),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(space::S16)),
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            GlobeEmptyState
            Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    Text({ "No plottable node location".to_owned() })
                    TextRole(Role::BodyStrong)
                    --
                    Text(detail)
                    GlobeEmptyStateDetail
                    TextRole(Role::Caption)
                ]
            ]
    }
}

/// Registers the Globe page once at product assembly. Resources outlive route
/// entities, so the selected mode survives a remount.
pub struct GlobePagePlugin;

impl Plugin for GlobePagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobeViewState>();
        app.init_resource::<LastGlobeData>();
        app.add_observer(on_globe_page_root_added);
        app.add_observer(on_globe_mode_activated);
        app.add_observer(on_globe_data_updated);
    }
}

/// Append the Globe section to the mounted Overview page root. The product
/// shell has no standalone Globe route, so Overview hosts it; the page root
/// marker keeps the section addressable and the exactly-one-`PageRoot`
/// invariant intact.
fn on_globe_page_root_added(
    trigger: On<Add<PageRoot>>,
    roots: Query<&PageRoot>,
    data: Res<LastGlobeData>,
    view: Res<GlobeViewState>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    let entity = trigger.event().entity;
    let Ok(root) = roots.get(entity) else {
        return;
    };
    if root.0 != Route::Overview {
        return;
    }
    let data = data.0.clone().unwrap_or_default();
    commands
        .spawn_scene(globe_page(&data, view.0, &palette))
        .insert(ChildOf(entity));
}

/// The mode toggle: restamp the selected mode onto the view state and every
/// mounted plate, then repaint the buttons in place.
fn on_globe_mode_activated(
    activate: On<Activate>,
    buttons: Query<&GlobeModeButton>,
    mut view: ResMut<GlobeViewState>,
    mut plates: Query<&mut GlobePlate>,
    mut visuals: Query<(&GlobeModeButton, &mut BackgroundColor, &Children)>,
    mut roles: Query<&mut TextRole>,
    palette: Res<UiPalette>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let mode = button.0;
    if view.0 != mode {
        view.0 = mode;
    }
    for mut plate in &mut plates {
        if plate.0.mode != mode {
            plate.0.mode = mode;
        }
    }
    for (marker, mut background, children) in &mut visuals {
        let selected = marker.0 == mode;
        let fill = if selected {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
        if background.0 != fill {
            background.0 = fill;
        }
        let role = if selected {
            Role::BodyStrong
        } else {
            Role::Body
        };
        for child in children.iter() {
            if let Ok(mut current) = roles.get_mut(*child)
                && current.0 != role
            {
                current.0 = role;
            }
        }
    }
}

/// Adopt a new shared projection: remember it and restamp every mounted plate.
fn on_globe_data_updated(
    trigger: On<GlobeDataUpdated>,
    mut last: ResMut<LastGlobeData>,
    view: Res<GlobeViewState>,
    mut plates: Query<&mut GlobePlate>,
) {
    let data = trigger.0.clone();
    last.0 = Some(data.clone());
    let spec = globe_spec(&data, view.0);
    for mut plate in &mut plates {
        if plate.0 != spec {
            plate.0 = spec.clone();
        }
    }
}
