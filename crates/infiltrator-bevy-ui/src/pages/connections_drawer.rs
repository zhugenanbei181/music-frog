//! Connections detail inspection drawer (连接透视).
//!
//! DUAL-13-03: the Bevy detail view is a right-docked slide-out drawer over a
//! scrim built from the shared `drawer_scene`, matching the Iced side-out
//! panel, instead of an inline card. DUAL-13-06 renders the parsed route chain
//! hop by hop; DUAL-13-09 wires the reverse-rule draft through the shared
//! domain seam. The timing waterfall (DUAL-13-04) stays typed unsupported: the
//! core exposes no DNS/TCP/TLS/TTFB stages, so no fabricated bars are drawn.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::drawer::{DrawerCloseButton, DrawerPlacement, drawer_scene};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::connection_view;
use infiltrator_domain::rules::RuleEntry;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections::{ConnInspectButton, LastConnectionsProjection};

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

/// Which connection the drawer shows and whether it is open.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsDrawerState {
    pub selected: Option<usize>,
    pub open: bool,
}

/// Shared reverse-rule draft store for the Bevy connections surface
/// (DUAL-13-09). Both surfaces append through
/// [`connection_view::append_draft_rule`].
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsRuleDraft {
    pub entries: Vec<RuleEntry>,
}

/// Connection detail inspection drawer scene. Mounted hidden; the inspect
/// action opens it and restamps its fields from the last projection.
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
            ( { drawer_scene(DrawerPlacement::Right, 420.0, content, palette) } ),
        ]
    }
}

fn connection_drawer_content(palette: &UiPalette) -> impl Scene + use<> {
    let hop_slots: Vec<Box<dyn Scene>> = (0..MAX_DRAWER_CHAIN_HOPS)
        .map(|hop| {
            Box::new(bsn! {
                Node {
                    display: { if hop == 0 { Display::Flex } else { Display::None } },
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S4),
                }
                ConnDrawerHopSlot(hop)
                Children [
                    ( Text({ String::new() }) ConnDrawerHopText(hop) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S12),
        }
        ConnectionDrawerRoot
        Children [
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            ( Text({ "单连接深度透视 (Deep Telemetry)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                        }
                        Button
                        DrawerCloseButton
                        Children [
                            ( Text({ "关闭".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Host) TextRole(Role::BodyStrong) ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Process) TextRole(Role::Caption) ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Rule) TextRole(Role::Caption) ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::RulePayload) TextRole(Role::Caption) ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Endpoints) TextRole(Role::Mono) ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Network) TextRole(Role::Caption) ),
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text({ "路由链: ".to_owned() }) TextRole(Role::Caption) ),
                    { hop_slots },
                ]
            ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Traffic) TextRole(Role::Mono) ),
            ( Text({ "—".to_owned() }) ConnDrawerField(ConnDrawerFieldKind::Rate) TextRole(Role::Mono) ),
            ( Text({ "内核未提供该连接的 DNS/TCP/TLS/TTFB 耗时明细".to_owned() }) TextRole(Role::Caption) ),
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexStart,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.accent })
                        Button
                        DrawerAddRuleButton
                        Children [
                            ( Text({ "一键添加为规则".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.danger })
                        Button
                        DrawerCloseConnectionButton
                        Children [
                            ( Text({ "断开此连接".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            ),
            ( Text({ "规则草稿: 尚未生成".to_owned() }) ConnDrawerRuleDraft TextRole(Role::Caption) ),
        ]
    }
}

/// Toggle the drawer layer's visibility from the shared drawer state.
pub(crate) fn sync_connections_drawer(
    state: Option<Res<ConnectionsDrawerState>>,
    mut layers: Query<&mut Node, With<ConnectionDrawerLayer>>,
) {
    let open = state.map(|state| state.open).unwrap_or(false);
    for mut node in &mut layers {
        node.display = if open { Display::Flex } else { Display::None };
    }
}

/// Inspect / close / add-rule activation for the connections drawer.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn on_connections_drawer_activated(
    activate: On<Activate>,
    inspect_buttons: Query<&ConnInspectButton>,
    add_rule_buttons: Query<(), With<DrawerAddRuleButton>>,
    close_connection_buttons: Query<(), With<DrawerCloseConnectionButton>>,
    close_buttons: Query<(), With<DrawerCloseButton>>,
    last: Option<Res<LastConnectionsProjection>>,
    mut drawer: Option<ResMut<ConnectionsDrawerState>>,
    mut draft: Option<ResMut<ConnectionsRuleDraft>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut fields: Query<
        (&mut Text, &ConnDrawerField),
        (Without<ConnDrawerHopText>, Without<ConnDrawerRuleDraft>),
    >,
    mut hops: Query<
        (&mut Text, &ConnDrawerHopText),
        (
            With<ConnDrawerHopText>,
            Without<ConnDrawerField>,
            Without<ConnDrawerRuleDraft>,
        ),
    >,
    mut hop_slots: Query<
        (&mut Node, &ConnDrawerHopSlot),
        (
            With<ConnDrawerHopSlot>,
            Without<ConnDrawerField>,
            Without<ConnDrawerRuleDraft>,
        ),
    >,
    mut draft_lines: Query<
        &mut Text,
        (
            With<ConnDrawerRuleDraft>,
            Without<ConnDrawerField>,
            Without<ConnDrawerHopText>,
        ),
    >,
) {
    if let Ok(inspect) = inspect_buttons.get(activate.entity) {
        if let Some(state) = drawer.as_deref_mut() {
            state.selected = Some(inspect.0);
            state.open = true;
        }
        if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
            restamp_drawer(
                projection,
                inspect.0,
                &mut fields,
                &mut hops,
                &mut hop_slots,
            );
        }
        return;
    }

    if close_buttons.contains(activate.entity) {
        if let Some(state) = drawer.as_deref_mut() {
            state.open = false;
        }
        return;
    }

    if add_rule_buttons.contains(activate.entity) {
        let Some(index) = drawer.as_ref().and_then(|state| state.selected) else {
            return;
        };
        let Some(item) = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .and_then(|projection| projection.connections.get(index))
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
                text.0 = format!("规则草稿: {}", entry.rule);
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
        let Some(index) = drawer.as_ref().and_then(|state| state.selected) else {
            return;
        };
        let Some(item) = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .and_then(|projection| projection.connections.get(index))
        else {
            return;
        };
        handle.submit(UiCommand::CloseConnection {
            id: item.id.clone(),
        });
        if let Some(state) = drawer.as_deref_mut() {
            state.open = false;
        }
    }
}

#[allow(clippy::type_complexity)]
fn restamp_drawer<F, H, S>(
    projection: &crate::pages::connections::ConnectionsProjection,
    index: usize,
    fields: &mut Query<(&mut Text, &ConnDrawerField), F>,
    hops: &mut Query<(&mut Text, &ConnDrawerHopText), H>,
    hop_slots: &mut Query<(&mut Node, &ConnDrawerHopSlot), S>,
) where
    F: bevy::ecs::query::QueryFilter,
    H: bevy::ecs::query::QueryFilter,
    S: bevy::ecs::query::QueryFilter,
{
    let Some(item) = projection.connections.get(index) else {
        return;
    };
    let chain = connection_view::route_chain(item);
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
                crate::pages::overview::format_byte_count(item.upload_total),
                crate::pages::overview::format_byte_count(item.download_total)
            ),
            // DUAL-13-10/12: the drawer shows the derived instantaneous rates
            // only once a real window exists; a fresh connection stays honest.
            ConnDrawerFieldKind::Rate => {
                if item.upload_bps > 0.0 || item.download_bps > 0.0 {
                    format!(
                        "瞬时 ↑ {}/s  ↓ {}/s",
                        crate::pages::overview::format_byte_count(item.upload_bps.max(0.0) as u64),
                        crate::pages::overview::format_byte_count(item.download_bps.max(0.0) as u64)
                    )
                } else {
                    "瞬时速率: 等待第二次采样".to_owned()
                }
            }
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
