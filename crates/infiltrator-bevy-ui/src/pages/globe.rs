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
//! **Mode seam.** [`GlobeMode`] (Globe / Flat2d / Eco) is exposed through the
//! OFFICIAL headless tab strip: the shared segmented control
//! ([`segmented_control_scene`]) is the official `TabList` / `Tab` /
//! `SelectedTab` / `TabActivation` owner, so the selection state, the roving
//! tab index, the keyboard navigation and the accessibility roles are the
//! widget layer's, not a hand-rolled button group. The page reads the
//! selected tab and restamps the mounted [`GlobePlate`] in place (charter law:
//! observers change components, never rebuild trees). The selected mode is a
//! page-local [`GlobeViewState`] resource that outlives a mount.
//!
//! **Empty-state seam.** A projection without a plottable location keeps the
//! section's skeleton: the wireframe globe stays mounted with the typed
//! reason, so the card never collapses into a dead grey slab while the geo
//! observation is still missing.
//!
//! **Product wiring.** The product shell has no separate Globe route (the
//! shared [`ShellPage`](infiltrator_contract::command_catalogue::ShellPage)
//! vocabulary and the capture route table are fixed), so this page is wired
//! as the Overview route's geo section: the Overview page owns a
//! [`GlobeSectionSlot`] and the section mounts into that slot, so the section's
//! position in the page is the page's decision (it never outranks the page's
//! own hero). The default [`LastGlobeData`] is empty, so the product
//! honestly shows the typed "not observed" state until a real geo observation
//! is composed.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::math::Vec2;
use bevy::picking::events::PointerClick;
use bevy::picking::pointer::PointerButton;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ValueChange;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::globe::{
    GeoLocation, GeoUnknown, GlobeLink, GlobeMode, GlobeNode, GlobePlate, GlobeProjection,
    GlobeSpec, globe_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::tabs::{SegmentedTab, segmented_control_scene};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};

/// Raster width of the mounted globe plate.
pub const GLOBE_WIDTH_PX: u32 = 640;
/// Raster height of the mounted globe plate.
pub const GLOBE_HEIGHT_PX: u32 = 400;

/// The rendering modes, in the order the official tab strip lists them. The
/// tab index IS the mode index: one list, one order, no second mapping.
const GLOBE_MODES: [GlobeMode; 3] = [GlobeMode::Globe, GlobeMode::Flat2d, GlobeMode::Eco];

/// The tab labels, in the same order as [`GLOBE_MODES`].
const GLOBE_MODE_LABELS: [&str; 3] = ["Globe", "Flat 2D", "Eco"];

/// The section's accent tick under the title, in baseline pixels.
const GLOBE_SECTION_TICK_WIDTH_PX: f32 = 24.0;
/// The section's accent tick thickness, in baseline pixels.
const GLOBE_SECTION_TICK_HEIGHT_PX: f32 = 2.0;

/// The skeleton globe's diameter, in baseline pixels.
const GLOBE_SKELETON_SIZE_PX: f32 = 168.0;
/// The skeleton globe's vertical meridian width, in baseline pixels.
const GLOBE_SKELETON_MERIDIAN_WIDTH_PX: f32 = 56.0;
/// The skeleton globe's horizontal meridian height, in baseline pixels.
const GLOBE_SKELETON_MERIDIAN_HEIGHT_PX: f32 = 56.0;
/// The empty plate's minimum height, in baseline pixels: the skeleton keeps a
/// stable frame, so the section does not jump when the observation arrives.
const GLOBE_EMPTY_PLATE_MIN_HEIGHT_PX: f32 = 260.0;

/// Root marker of the Globe page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobePageRoot;

/// Marker on the Overview page's geo slot: the section mounts here, so the
/// page owns where the section sits.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobeSectionSlot;

/// Marker on the official tab strip that carries the globe's rendering mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobeModeStrip;

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

/// A typed record of the node the user picked on the globe. The coordinate is
/// the node's own provenance-bearing observation, never a derived guess.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobePickRecord {
    pub id: String,
    pub label: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
}

/// The typed intent that a node was picked; observers (capture, telemetry)
/// consume it, the page never inspects consumers.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct GlobeNodePicked(pub GlobePickRecord);

/// The current pick, as the single truth the detail line restamps from.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GlobePicked(pub Option<GlobePickRecord>);

/// Great-circle click tolerance in degrees; a click farther from every
/// plottable visible node picks nothing.
const NODE_PICK_TOLERANCE_DEG: f32 = 8.0;

/// The detail line's text before any pick has happened.
const GLOBE_PICK_DEFAULT_TEXT: &str = "Click a plottable node to inspect its location.";

/// Marker on the header's picked-node detail line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobePickedDetail;

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

/// The tab index of one rendering mode inside the official strip.
pub fn globe_mode_index(mode: GlobeMode) -> usize {
    GLOBE_MODES
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or_default()
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

/// The Globe page scene: the section card with its header, the official mode
/// tab strip and either the globe plate or the skeleton empty state.
pub fn globe_page(data: &GlobeData, mode: GlobeMode, palette: &UiPalette) -> impl Scene + use<> {
    let spec = globe_spec(data, mode);
    let body: Box<dyn Scene> = if data.has_plottable() {
        Box::new(globe_scene(spec))
    } else {
        Box::new(empty_state_scene(data, palette))
    };
    bsn! {
            @{ surface_scene(
                vec![
                    Box::new(header_scene(data)),
                    Box::new(mode_strip_scene(mode, palette)),
                    body,
                ],
                palette,
            ) }
            GlobePageRoot
    }
}

/// The section header: the title with its accent tick, then the plottable
/// summary. The summary is reading copy, not a caption — it states what the
/// map currently shows.
fn header_scene(data: &GlobeData) -> impl Scene + use<> {
    let summary = format!(
        "{} of {} node locations are plottable",
        data.plottable_count(),
        data.nodes.len()
    );
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S6),
                align_items: AlignItems::FlexStart,
            }
            Children [
                Node {
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S8),
                }
                Children [
                    Node {
                        width: rem(GLOBE_SECTION_TICK_WIDTH_PX),
                        height: rem(GLOBE_SECTION_TICK_HEIGHT_PX),
                        border_radius: BorderRadius::all(rem(radius::XS)),
                    }
                    ThemeBackgroundColor({ Token::Accent })
                    --
                    Text({ "Geo Globe".to_owned() }) TextRole(Role::Heading)
                ]
                --
                Text(summary) TextRole(Role::BodyStrong)
                --
                Text(GLOBE_PICK_DEFAULT_TEXT.to_owned()) TextRole(Role::Body) GlobePickedDetail
            ]
    }
}

/// The official mode strip: the shared segmented control over the headless
/// `TabList`, so the selection, the keyboard navigation and the accessibility
/// roles are the widget layer's.
fn mode_strip_scene(mode: GlobeMode, palette: &UiPalette) -> impl Scene + use<> {
    let labels = GLOBE_MODE_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect::<Vec<_>>();
    bsn! {
            @{ segmented_control_scene(labels, globe_mode_index(mode), palette) }
            GlobeModeStrip
    }
}

/// The empty/unknown state: the skeleton globe the section keeps while no
/// plottable location has been observed, plus the typed reason.
fn empty_state_scene(data: &GlobeData, palette: &UiPalette) -> impl Scene + use<> {
    let detail = globe_unknown_detail(data);
    bsn! {
            Node {
                width: percent(100),
                min_height: rem(GLOBE_EMPTY_PLATE_MIN_HEIGHT_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            GlobeEmptyState
            Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: rem(space::S12),
                }
                Children [
                    @{ skeleton_scene(palette) }
                    --
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

/// The skeleton globe: the wireframe the section keeps while the geo
/// observation is missing. A sphere reads as "the map is on its way"; a flat
/// slab reads as a dead card.
fn skeleton_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                width: rem(GLOBE_SKELETON_SIZE_PX),
                height: rem(GLOBE_SKELETON_SIZE_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(palette.hairline_px)),
                border_radius: BorderRadius::MAX,
            }
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Children [
                Node {
                    width: rem(GLOBE_SKELETON_MERIDIAN_WIDTH_PX),
                    height: percent(100),
                    border: UiRect::all(px(palette.hairline_px)),
                    border_radius: BorderRadius::MAX,
                }
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                --
                Node {
                    width: percent(100),
                    height: rem(GLOBE_SKELETON_MERIDIAN_HEIGHT_PX),
                    border: UiRect::all(px(palette.hairline_px)),
                    border_radius: BorderRadius::MAX,
                }
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
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
        app.init_resource::<GlobePicked>();
        app.add_observer(on_globe_section_slot_added);
        app.add_observer(on_globe_plate_clicked);
        app.add_observer(on_globe_mode_selected);
        app.add_observer(on_globe_data_updated);
        app.add_systems(Update, restamp_globe_picked_detail);
    }
}

/// Mount the Globe section into the Overview page's geo slot. The page owns
/// the slot's position, so the section never outranks the page's own hero.
fn on_globe_section_slot_added(
    trigger: On<Add<GlobeSectionSlot>>,
    data: Res<LastGlobeData>,
    view: Res<GlobeViewState>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    let entity = trigger.event().entity;
    let data = data.0.clone().unwrap_or_default();
    commands
        .spawn_scene(globe_page(&data, view.0, &palette))
        .insert(ChildOf(entity));
}

/// The mode switch: the official tab strip owns the selection, so the page
/// only reads the selected tab and restamps the mounted plate in place.
fn on_globe_mode_selected(
    change: On<ValueChange<Option<Entity>>>,
    strips: Query<(), With<GlobeModeStrip>>,
    tabs: Query<&SegmentedTab>,
    mut view: ResMut<GlobeViewState>,
    mut plates: Query<&mut GlobePlate>,
) {
    if !strips.contains(change.source) {
        return;
    }
    let Some(selected) = change.value else {
        return;
    };
    let Ok(tab) = tabs.get(selected) else {
        return;
    };
    let Some(mode) = GLOBE_MODES.get(tab.0).copied() else {
        return;
    };
    if view.0 != mode {
        view.0 = mode;
    }
    for mut plate in &mut plates {
        if plate.0.mode != mode {
            plate.0.mode = mode;
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

/// A primary click on the globe: resolve the clicked UI node up to its
/// plate, turn the UI picking backend's node-normalized hit position into
/// raster pixels, invert the projection onto the sphere, and pick the
/// nearest plottable visible node (#99). The pick becomes the plate's
/// hovered node — the raster already paints that highlight — plus the typed
/// [`GlobeNodePicked`] intent and the [`GlobePicked`] record.
fn on_globe_plate_clicked(
    trigger: On<PointerClick>,
    parents: Query<&ChildOf>,
    mut plates: Query<&mut GlobePlate>,
    mut picked: ResMut<GlobePicked>,
    mut commands: Commands,
) {
    let event = trigger.event();
    if event.button != PointerButton::Primary {
        return;
    }
    let Some(hit_position) = event.hit.position else {
        return;
    };
    // The charter renders the raster on a child node of the plate, so the
    // clicked entity walks up the hierarchy until it carries the plate.
    let mut entity = event.entity;
    let mut plate = plates.get_mut(entity);
    while plate.is_err() {
        let Ok(parent) = parents.get(entity) else {
            return;
        };
        entity = parent.parent();
        plate = plates.get_mut(entity);
    }
    let Ok(mut plate) = plate else {
        return;
    };
    let spec = &plate.0;
    // The UI backend reports (-0.5, -0.5) top-left .. (0.5, 0.5) bottom-right
    // across the whole node, so the fraction maps straight into raster px.
    let fraction = hit_position.xy() + Vec2::splat(0.5);
    let raster_px = Vec2::new(
        fraction.x * spec.width as f32,
        fraction.y * spec.height as f32,
    );
    let projection = GlobeProjection::from_spec(spec);
    let Some(hit) = projection.unproject(raster_px) else {
        return;
    };
    let Some(node) = spec.nearest_node_to(&hit, NODE_PICK_TOLERANCE_DEG) else {
        return;
    };
    let Some(point) = node.location.plottable() else {
        return;
    };
    let record = GlobePickRecord {
        id: node.id.clone(),
        label: node.label.clone(),
        lat_deg: point.lat_deg(),
        lon_deg: point.lon_deg(),
    };
    plate.hovered_node = Some(record.id.clone());
    if picked.0.as_ref() != Some(&record) {
        picked.0 = Some(record.clone());
    }
    commands.trigger(GlobeNodePicked(record));
}

/// Restamp the section's picked-node detail line from the single pick
/// record. The line states the node's own observed coordinate — never a
/// derived or rounded reconstruction of the click.
fn restamp_globe_picked_detail(
    picked: Res<GlobePicked>,
    mut detail: Query<&mut Text, With<GlobePickedDetail>>,
) {
    let Some(record) = picked.0.as_ref() else {
        return;
    };
    let line = format!(
        "{} — {:.2}°, {:.2}°",
        record.label, record.lat_deg, record.lon_deg
    );
    for mut text in &mut detail {
        if text.0 != line {
            text.0.clone_from(&line);
        }
    }
}
