//! Connections detail inspection drawer (连接透视).
//!
//! DUAL-13-03: the Bevy detail view is a right-docked slide-out drawer over a
//! scrim built from the shared `drawer_scene`, matching the Iced side-out
//! panel, instead of an inline card. DUAL-13-06 renders the parsed route chain
//! hop by hop; DUAL-13-09 wires the reverse-rule draft through the shared
//! domain seam. The timing waterfall (DUAL-13-04) stays typed unsupported: the
//! core exposes no DNS/TCP/TLS/TTFB stages, so no fabricated bars are drawn.

#[path = "connections_drawer_query_access.rs"]
pub mod query_access;
use self::query_access::ConnectionDrawerControls;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections::{
    ConnectionsProjection, ConnectionsScrollArea, LastConnectionsProjection,
};
use crate::pages::connections_clipboard::CopyConnectionHostButton;
use bevy::ecs::change_detection::DetectChanges;
use infiltrator_presentation::connection_detail_projection::{kernel_asn_label, kernel_geo_label};
use infiltrator_shared::byte_units::format_bytes;

use crate::route::{ActiveRoute, Route};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input::{ButtonInput, keyboard::KeyCode};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, Node, Overflow, OverflowAxis,
    PositionType, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::drawer::{
    DrawerCloseButton, DrawerCloseEvent, DrawerOpenEvent, DrawerPhase, DrawerPhaseState,
    DrawerPlacement, DrawerScrim, drawer_scene_with_phase,
};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::connection::ConnectionRateObservation;
use infiltrator_contract::connection::timing_availability;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_shared::locales::{Lang, Localizer};

/// Maximum route-chain hops rendered in the drawer.
const MAX_DRAWER_CHAIN_HOPS: usize = 6;

/// Marker for the connection inspection drawer root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionDrawerRoot;

/// Marker on the absolute layer that owns drawer visibility.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionDrawerLayer;

/// Marker for action button on connection drawer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerAddRuleButton;

/// DUAL-13-14: the drawer's "disconnect this connection" action, the same
/// teardown the Iced drawer exposes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerCloseConnectionButton;

/// Mutable drawer text fields restamped when a connection is inspected.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnDrawerField(pub ConnDrawerFieldKind);

/// Which detail line a [`ConnDrawerField`] carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnDrawerFieldKind {
    #[default]
    Host,
    Process,
    Rule,
    /// DUAL-13-14: the matched rule payload, rendered by Iced as well.
    RulePayload,
    /// DUAL-13-14: local `ip:port → remote ip:port` endpoints.
    Endpoints,
    /// DUAL-13-14: transport/network label (`TCP`/`UDP`).
    Network,
    Traffic,
    /// DUAL-13-10/12: the derived instantaneous rates of the connection.
    Rate,
    /// DUAL-13-05: the kernel's `destinationIPASN` rule-evaluation result.
    KernelAsn,
    /// DUAL-13-05: the kernel's `destinationGeoIP` rule-evaluation result.
    KernelGeo,
}

/// Marker on one route-chain hop text slot in the drawer (DUAL-13-06).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnDrawerHopText(pub usize);

/// Marker on the node wrapping one hop text slot, toggled with the chain.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnDrawerHopSlot(pub usize);

/// Marker on the drafted-rule preview line (DUAL-13-09).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnDrawerRuleDraft;

/// The connections drawer is either closed or inspecting exactly one
/// connection.
///
/// The inspected connection is identified by its stable projection id, so the
/// drawer never keeps a second copy of the same selection: the row index is
/// derived from the id whenever a consumer needs it, and an open drawer
/// always carries the connection it inspects.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ConnectionsDrawerState {
    /// The drawer is hidden and inspects nothing.
    #[default]
    Closed,
    /// The drawer is open and inspects the connection with this id.
    Open(InspectedConnection),
}

/// The connection the drawer inspects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectedConnection {
    id: String,
}

/// Why one drawer stage transition was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerRefusal {
    /// The drawer is already closed; there is no inspection to dismiss.
    AlreadyClosed,
}

/// The outcome of one drawer stage transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerTransition {
    /// The stage changed; the payload is the stage that is now running.
    Applied(ConnectionsDrawerState),
    /// The transition is not legal in the running stage; the payload is that
    /// stage together with the reason it was refused.
    Refused(ConnectionsDrawerState, DrawerRefusal),
}

impl InspectedConnection {
    /// A connection identified by its stable projection id.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// The inspected connection's stable projection id.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl ConnectionsDrawerState {
    /// Whether the drawer is open.
    pub const fn is_open(&self) -> bool {
        matches!(self, Self::Open(_))
    }

    /// The inspected connection's id, if the drawer is open.
    pub fn inspected_id(&self) -> Option<&str> {
        match self {
            Self::Closed => None,
            Self::Open(open) => Some(open.id()),
        }
    }

    /// The row of the inspected connection in the given projection, if it is
    /// still listed.
    pub fn inspected_index(&self, projection: &ConnectionsProjection) -> Option<usize> {
        match self {
            Self::Closed => None,
            Self::Open(open) => projection
                .connections
                .iter()
                .position(|item| item.id == open.id()),
        }
    }

    /// Inspect the connection with this id.
    ///
    /// Legal from every stage: the drawer follows the selection, so a second
    /// inspected row replaces the first one.
    pub fn inspect(&mut self, id: String) -> DrawerTransition {
        *self = Self::Open(InspectedConnection::new(id));
        DrawerTransition::Applied(self.clone())
    }

    /// Close the drawer. Only an open drawer can be closed.
    pub fn close(&mut self) -> DrawerTransition {
        match self {
            Self::Closed => DrawerTransition::Refused(self.clone(), DrawerRefusal::AlreadyClosed),
            Self::Open(_) => {
                *self = Self::Closed;
                DrawerTransition::Applied(self.clone())
            }
        }
    }
}

/// Shared reverse-rule draft store for the Bevy connections surface
/// (DUAL-13-09). Both surfaces append through
/// [`connection_view::append_draft_rule`].
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsRuleDraft {
    pub entries: Vec<RuleEntry>,
}

/// Connection detail inspection drawer scene. Mounted closed; the inspect
/// action opens it and restamps its fields from the last projection.
///
/// The drawer is mounted at the shared drawer's own closed phase, so the
/// panel starts out of the way and the shared phase walks it in: the drawer
/// really fades in instead of appearing at once.
pub fn connection_drawer_scene(palette: &UiPalette) -> impl Scene + use<> {
    let content: Box<dyn Scene> = Box::new(connection_drawer_content(palette));

    bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                display: Display::None,
            }
            ConnectionDrawerLayer
            Children [
                @{ drawer_scene_with_phase(DrawerPlacement::Right, 420.0, DrawerPhase::Closed, content, palette) }
            ]
    }
}

fn connection_drawer_content(palette: &UiPalette) -> impl Scene + use<> {
    let language = UiLocale::default().code().to_string();
    let timing_notice = match timing_availability() {
        Availability::Unsupported { .. } => Lang(&language)
            .tr("conn_drawer_timing_unsupported")
            .into_owned(),
        _ => unreachable!("connection timing capability must be explicit"),
    };
    let lang = Lang(&language);
    let title = lang.tr("conn_drawer_title").into_owned();
    let close = lang.tr("conn_drawer_close").into_owned();
    let chain_label = lang.tr("conn_drawer_proxy_chain").into_owned();
    let add_rule = lang.tr("quick_rule_btn").into_owned();
    let disconnect = lang.tr("conn_drawer_close_conn_btn").into_owned();
    let draft_empty = lang.tr("conn_drawer_draft_empty").into_owned();
    let copy_label = Lang(&language).tr("conn_drawer_copy_host_btn").into_owned();
    let hop_slots: Vec<Box<dyn Scene>> = (0..MAX_DRAWER_CHAIN_HOPS)
        .map(|hop| {
            Box::new(bsn! {
                Node {
                    display: { if hop == 0 { Display::Flex } else { Display::None } },
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S4),
                }
                ConnDrawerHopSlot(hop)
                Children [
                    Text({ String::new() }) ConnDrawerHopText(hop) TextRole(Role::Caption)
                ]
}) as Box<dyn Scene>
        })
        .collect();

    bsn! {
            Node {
                width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
            }
            ConnectionDrawerRoot ScrollArea
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        @{ icon_tile_scene(IconId::Activity, 24.0, palette) }
                        --
                        Text(title) TextRole(Role::BodyStrong)
                    ]
                    --
                    Node {
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    Button
                    DrawerCloseButton
                    Children [
                        Text(close) TextRole(Role::Caption)
                    ]
                ]
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Host) TextRole(Role::BodyStrong)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Process) TextRole(Role::Caption)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Rule) TextRole(Role::Caption)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::RulePayload) TextRole(Role::Caption)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Endpoints) TextRole(Role::Mono)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Network) TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S4),
                }
                Children [
                    Text(chain_label) TextRole(Role::Caption)
                    --
                    { hop_slots }
                ]
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Traffic) TextRole(Role::Mono)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Rate) TextRole(Role::Mono)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::KernelAsn) TextRole(Role::Mono)
                --
                Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::KernelGeo) TextRole(Role::Mono)
                --
                Text(timing_notice) TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexStart,
                    column_gap: rem(space::S8),
                }
                Children [
                    Node {
                        min_height: rem(palette.control_height_px),
                        padding: UiRect::horizontal(rem(space::S12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    }
                    ThemeBackgroundColor({ Token::Accent })
                    Button
                    DrawerAddRuleButton
                    Children [
                        Text(add_rule) TextRole(Role::BodyStrong)
                    ]
                    --
                    Node {
                        min_height: rem(palette.control_height_px),
                        padding: UiRect::horizontal(rem(space::S12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    }
                    ThemeBackgroundColor({ Token::Danger })
                    Button
                    DrawerCloseConnectionButton
                    Children [
                        Text(disconnect) TextRole(Role::BodyStrong)
                    ]
                ]
                --
                Node { min_height: rem(palette.control_height_px), padding: UiRect::horizontal(rem(space::S12)), align_items: AlignItems::Center }
                Button CopyConnectionHostButton
                Children [ Text(copy_label) TextRole(Role::BodyStrong) ]
                --
                Text(draft_empty) ConnDrawerRuleDraft TextRole(Role::Caption)
            ]
    }
}

/// Drive the shared drawer's typed phase from the drawer's own state.
///
/// The phase is the drawer's only state: the request is the drawer's own
/// open/close transition, and the shared layer walks the slide from it.
pub(crate) fn sync_connections_drawer_phase(
    state: Option<Res<ConnectionsDrawerState>>,
    scrims: Query<(Entity, &DrawerPhaseState), With<DrawerScrim>>,
    mut opens: MessageWriter<DrawerOpenEvent>,
    mut closes: MessageWriter<DrawerCloseEvent>,
) {
    let Some(state) = state else {
        return;
    };
    let Ok((entity, phase)) = scrims.single() else {
        return;
    };
    match (state.is_open(), phase.0) {
        (true, DrawerPhase::Closed) => {
            opens.write(DrawerOpenEvent(entity));
        }
        (false, DrawerPhase::Open | DrawerPhase::Opening { .. }) => {
            closes.write(DrawerCloseEvent(entity));
        }
        _ => {}
    }
}

/// Toggle the drawer layer's visibility from the shared drawer's phase.
///
/// The layer stays mounted while the drawer slides, so the panel is really
/// seen sliding in; it is hidden only once the phase has settled closed.
pub(crate) fn sync_connections_drawer(
    state: Option<Res<ConnectionsDrawerState>>,
    phases: Query<&DrawerPhaseState, With<DrawerScrim>>,
    mut layers: Query<&mut Node, (With<ConnectionDrawerLayer>, Without<ConnectionsScrollArea>)>,
    mut scrolls: Query<&mut Node, (With<ConnectionsScrollArea>, Without<ConnectionDrawerLayer>)>,
) {
    let phase = phases
        .single()
        .ok()
        .map(|phase| phase.0)
        .unwrap_or(DrawerPhase::Closed);
    let open = state.map(|state| state.is_open()).unwrap_or(false);
    for mut node in &mut layers {
        // The layer stays flex while the drawer slides: the panel is only
        // hidden once the shared phase has settled closed.
        let want = if !open {
            if matches!(phase, DrawerPhase::Closing { .. }) {
                Display::Flex
            } else {
                Display::None
            }
        } else {
            Display::Flex
        };
        if node.display != want {
            node.display = want;
        }
    }
    // The open drawer owns the viewport: the layer below stops scrolling, so
    // neither the wheel nor the touch drag can move the connection list under
    // the panel. The official scroll container honours the same rule — it
    // scrolls only while its own overflow axis is `scroll`.
    for mut node in &mut scrolls {
        let want = if open {
            OverflowAxis::Clip
        } else {
            OverflowAxis::Scroll
        };
        if node.overflow.y != want {
            node.overflow.y = want;
        }
    }
}

/// Inspect / close / add-rule activation for the connections drawer.
pub(crate) fn on_connections_drawer_activated(
    activate: On<Activate>,
    last: Option<Res<LastConnectionsProjection>>,
    mut drawer: Option<ResMut<ConnectionsDrawerState>>,
    mut draft: Option<ResMut<ConnectionsRuleDraft>>,
    handle: Option<Res<CommandSinkHandle>>,
    targets: ConnectionDrawerControls,
) {
    let ConnectionDrawerControls {
        inspect_buttons,
        add_rule_buttons,
        close_connection_buttons,
        close_buttons,
        mut fields,
        mut hops,
        mut hop_slots,
        mut draft_lines,
    } = targets;

    if let Ok(inspect) = inspect_buttons.get(activate.entity) {
        let Some(item) = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .and_then(|projection| projection.connections.get(inspect.0))
        else {
            return;
        };
        if let Some(state) = drawer.as_deref_mut() {
            state.inspect(item.id.clone());
        }
        restamp_drawer(item, &mut fields, &mut hops, &mut hop_slots);
        return;
    }

    if close_buttons.contains(activate.entity) {
        if let Some(state) = drawer.as_deref_mut() {
            state.close();
        }
        return;
    }

    if add_rule_buttons.contains(activate.entity) {
        let Some(id) = drawer
            .as_ref()
            .filter(|state| state.is_open())
            .and_then(|state| state.inspected_id())
        else {
            return;
        };
        let Some(item) = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .and_then(|projection| projection.connections.iter().find(|item| item.id == id))
        else {
            return;
        };
        let spec =
            connection_view::quick_rule_spec(item, connection_view::DEFAULT_QUICK_RULE_TARGET);
        let Some(draft) = draft.as_deref_mut() else {
            return;
        };
        let added = connection_view::append_draft_rule(&mut draft.entries, &spec);
        if let Some(entry) = added {
            for mut text in &mut draft_lines {
                let language = UiLocale::default().code().to_string();
                text.0 = format!(
                    "{}: {}",
                    Lang(&language).tr("conn_drawer_draft"),
                    entry.rule
                );
            }
        }
        return;
    }

    if close_connection_buttons.contains(activate.entity) {
        // DUAL-13-14: the drawer tears down the same connection id the Iced
        // drawer closes, then hides itself.
        let Some(handle) = handle else {
            return;
        };
        let Some(id) = drawer
            .as_ref()
            .filter(|state| state.is_open())
            .and_then(|state| state.inspected_id())
        else {
            return;
        };
        let Some(item) = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .and_then(|projection| projection.connections.iter().find(|item| item.id == id))
        else {
            return;
        };
        handle.submit(UiCommand::CloseConnection {
            id: item.id.clone(),
        });
        if let Some(state) = drawer.as_deref_mut() {
            state.close();
        }
    }
}

fn restamp_drawer<F, H, S>(
    item: &ConnectionSnapshot,
    fields: &mut Query<(&mut Text, &ConnDrawerField), F>,
    hops: &mut Query<(&mut Text, &ConnDrawerHopText), H>,
    hop_slots: &mut Query<(&mut Node, &ConnDrawerHopSlot), S>,
) where
    F: QueryFilter,
    H: QueryFilter,
    S: QueryFilter,
{
    let chain = connection_view::route_chain(item);
    let language = UiLocale::default().code().to_string();
    let lang = Lang(&language);
    let translate = |key: &str| lang.tr(key).into_owned();
    for (mut text, field) in fields.iter_mut() {
        text.0 = match field.0 {
            ConnDrawerFieldKind::Host => item.host.clone(),
            ConnDrawerFieldKind::Process => {
                if item.process.is_empty() {
                    "—".to_owned()
                } else {
                    item.process.clone()
                }
            }
            ConnDrawerFieldKind::Rule => item.rule.clone(),
            ConnDrawerFieldKind::RulePayload => {
                if item.rule_payload.is_empty() {
                    "—".to_owned()
                } else {
                    item.rule_payload.clone()
                }
            }
            ConnDrawerFieldKind::Endpoints => format!(
                "{}:{} → {}:{}",
                item.source_ip, item.source_port, item.destination_ip, item.destination_port
            ),
            ConnDrawerFieldKind::Network => item.network.to_uppercase(),
            ConnDrawerFieldKind::Traffic => format!(
                "↑ {}  ↓ {}",
                format_bytes(item.upload_total),
                format_bytes(item.download_total)
            ),
            // DUAL-13-10/12: the drawer shows the derived instantaneous rates
            // only once a real window exists; a fresh connection stays honest.
            ConnDrawerFieldKind::Rate => match item.rate_observation {
                ConnectionRateObservation::Measured => format!(
                    "{} ↑ {}/s  ↓ {}/s",
                    lang.tr("conn_drawer_rate_prefix"),
                    format_bytes(item.upload_bps.max(0.0) as u64),
                    format_bytes(item.download_bps.max(0.0) as u64)
                ),
                ConnectionRateObservation::Unobserved => {
                    lang.tr("conn_drawer_rate_pending").into_owned()
                }
            },
            // DUAL-13-05: the kernel's own GEOIP/IP-ASN rule-evaluation results.
            // The client reads no MMDB and never guesses a location or ASN.
            ConnDrawerFieldKind::KernelAsn => format!(
                "{}: {}",
                lang.tr("conn_drawer_kernel_asn"),
                kernel_asn_label(&item.destination_ip_asn, &translate)
            ),
            ConnDrawerFieldKind::KernelGeo => format!(
                "{}: {}",
                lang.tr("conn_drawer_kernel_geo"),
                kernel_geo_label(item.destination_geo_ip.as_deref(), &translate)
            ),
        };
    }
    for (mut text, marker) in hops.iter_mut() {
        text.0 = chain.hops().get(marker.0).cloned().unwrap_or_default();
    }
    for (mut node, slot) in hop_slots.iter_mut() {
        // Hide slots past the parsed chain so a shorter chain cannot show a
        // stale hop. The first slot always shows, so the drawer is never blank.
        node.display = if slot.0 <= chain.len() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

type DrawerFieldFilter = (Without<ConnDrawerHopText>, Without<ConnDrawerRuleDraft>);
type DrawerHopFilter = (
    With<ConnDrawerHopText>,
    Without<ConnDrawerField>,
    Without<ConnDrawerRuleDraft>,
);
type DrawerSlotFilter = (
    With<ConnDrawerHopSlot>,
    Without<ConnDrawerField>,
    Without<ConnDrawerRuleDraft>,
);

/// Live refresh follows identity through reorder and dismisses a vanished connection.
pub fn reconcile_connection_inspection(
    last: Option<Res<LastConnectionsProjection>>,
    mut state: ResMut<ConnectionsDrawerState>,
    mut fields: Query<(&mut Text, &ConnDrawerField), DrawerFieldFilter>,
    mut hops: Query<(&mut Text, &ConnDrawerHopText), DrawerHopFilter>,
    mut slots: Query<(&mut Node, &ConnDrawerHopSlot), DrawerSlotFilter>,
) {
    let Some(last) = last else {
        return;
    };
    if !state.is_open() || (!last.is_changed() && !state.is_changed()) {
        return;
    }
    let Some(projection) = last.0.as_ref() else {
        *state = ConnectionsDrawerState::default();
        return;
    };
    // The inspected identity is the only source: a connection that vanished
    // from the projection closes the drawer instead of leaving a stale row.
    let Some(index) = state.inspected_index(projection) else {
        *state = ConnectionsDrawerState::default();
        return;
    };
    restamp_drawer(
        &projection.connections[index],
        &mut fields,
        &mut hops,
        &mut slots,
    );
}

/// Closing an inspector discards its selection and never submits a command.
pub fn dismiss_connection_inspection(
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<ConnectionsDrawerState>,
) {
    if state.is_open()
        && (route.0 != Some(Route::Connections)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
    {
        state.close();
    }
}
