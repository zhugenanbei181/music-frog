//! The single proxy node card scene: flag, title, protocol & UDP chips,
//! feature chips, favorite pin, detail affordance and the latency trend tile.
//!
//! The same card vocabulary backs the full-mount path and the virtual-list
//! recycler, so the sized variant takes an explicit column width.
//!
//! **Who paints the card.** The shared button skin owns the hover/press wash
//! of every product control, but a full-card accent fill buries the node's own
//! text. The page therefore paints the card itself
//! ([`sync_proxy_card_visuals`]): the selected card keeps the reading surface
//! and marks itself with an accent border and an accent rail, so the
//! highlight never hides what the card says.

use crate::pages::proxies::{
    LatencySkeletonPulse, LatencyText, NodeDetailButton, NodeFlagText, NodeNameText, NodePinButton,
    NodeProtoText, NodeUdpTag, ProxyNode, ProxyNodeButton, TestProxyGroupButton, latency_color,
};
use crate::pages::proxies_card::overlays::{latency_sparkline_scene, name_fade_scene};
use crate::pages::proxies_preferences::ProxyFavoriteStar;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{BoxShadow, ShadowStyle, UiTransform, Val2};
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::gesture::{
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, SwipeToActionSpring,
};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_presentation::latency_projection::project_proxy_latency;
use infiltrator_presentation::proxies_page_model::proxy_protocol_chip;
use infiltrator_shared::country_flags::node_flag_emoji;
use lurker_core_contract::outbound::identity::OutboundId;

/// The selected proxy card's accent border width in baseline pixels.
const SELECTED_CARD_BORDER_WIDTH_PX: f32 = 1.5;

/// The resting proxy card's hairline border width in baseline pixels.
const RESTING_CARD_BORDER_WIDTH_PX: f32 = 1.0;

/// The proxy node card's minimum height in baseline pixels.
const PROXY_NODE_CARD_MIN_HEIGHT_PX: f32 = 58.0;

/// The swipe action drawer's fixed width in baseline pixels.
const SWIPE_ACTION_DRAWER_WIDTH_PX: f32 = 88.0;

/// The selected card's accent rail width in baseline pixels.
const SELECTION_RAIL_WIDTH_PX: f32 = 3.0;

/// The hover lift: how far the card rises under the pointer, in baseline pixels.
const CARD_HOVER_LIFT_PX: f32 = 2.0;

/// The hover shadow's blur radius, in baseline pixels.
const CARD_HOVER_SHADOW_BLUR_PX: f32 = 10.0;

/// The hover shadow's vertical offset, in baseline pixels.
const CARD_HOVER_SHADOW_OFFSET_PX: f32 = 3.0;

/// The node detail affordance's icon plate size, in baseline pixels.
const DETAIL_ICON_SIZE_PX: f32 = 14.0;

/// A card whose group cannot be selected from: the same washed fill the
/// shared skin paints a disabled control with.
const DISABLED_CARD_ALPHA: f32 = 0.4;

/// The card's painted state.
///
/// One typed enum, one exhaustive resolver: the owner fact (this node is the
/// group's current outbound) and the pointer fact (hover / press) never
/// collapse into a bag of booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyCardState {
    /// Resting, and not the group's current outbound.
    Resting,
    /// Resting, with the pointer over the card.
    Hovered,
    /// The pointer is pressing the card.
    Pressed,
    /// The card is the group's current outbound.
    Selected,
    /// The current outbound, with the pointer over the card.
    SelectedHovered,
    /// The current outbound, with the pointer pressing it.
    SelectedPressed,
}

/// Marker on the card's selection rail: the accent bar that marks the
/// group's current outbound without tinting the card's reading surface.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxyNodeSelectionRail;

/// Resolve the card's painted state from the two authoritative facts.
pub fn resolve_proxy_card_state(
    selected: bool,
    interaction: Option<PickingInteraction>,
) -> ProxyCardState {
    match (selected, interaction) {
        (false, Some(PickingInteraction::Pressed)) => ProxyCardState::Pressed,
        (false, Some(PickingInteraction::Hovered)) => ProxyCardState::Hovered,
        (false, _) => ProxyCardState::Resting,
        (true, Some(PickingInteraction::Pressed)) => ProxyCardState::SelectedPressed,
        (true, Some(PickingInteraction::Hovered)) => ProxyCardState::SelectedHovered,
        (true, _) => ProxyCardState::Selected,
    }
}

/// The paint one card state resolves to. Pure data, so the ladder is
/// headless-testable without an app.
pub struct ProxyCardPaint {
    /// The card's own surface.
    pub fill: Color,
    /// The card's edge.
    pub border: Color,
    /// The card's edge width.
    pub border_width: Val,
    /// The card's drop shadow, if the state carries one.
    pub shadow: Option<BoxShadow>,
    /// The card's hover lift, if the state carries one.
    pub lift: Option<Val2>,
}

/// Resolve one card state into its paint. The selected card keeps the reading
/// surface: the accent rides the edge and the rail, never the whole card.
///
/// A card whose group cannot be selected from is washed out and never reacts
/// to the pointer, exactly like a disabled control.
pub fn proxy_card_paint(
    state: ProxyCardState,
    disabled: bool,
    palette: &UiPalette,
) -> ProxyCardPaint {
    let shadow = BoxShadow::new(
        palette.border,
        Val::ZERO,
        rem(CARD_HOVER_SHADOW_OFFSET_PX),
        Val::ZERO,
        rem(CARD_HOVER_SHADOW_BLUR_PX),
    );
    let lift = Val2::new(Val::ZERO, rem(-CARD_HOVER_LIFT_PX));
    let resting = ProxyCardPaint {
        fill: palette.surface_elevated,
        border: palette.border,
        border_width: rem(RESTING_CARD_BORDER_WIDTH_PX),
        shadow: None,
        lift: None,
    };
    let marked = ProxyCardPaint {
        fill: palette.surface_elevated,
        border: palette.accent,
        border_width: rem(SELECTED_CARD_BORDER_WIDTH_PX),
        shadow: None,
        lift: None,
    };
    let paint = match state {
        ProxyCardState::Resting => resting,
        ProxyCardState::Hovered => ProxyCardPaint {
            shadow: Some(shadow),
            lift: Some(lift),
            ..marked
        },
        ProxyCardState::Pressed => ProxyCardPaint {
            fill: palette.pressed_bg,
            ..marked
        },
        ProxyCardState::Selected => marked,
        ProxyCardState::SelectedHovered => ProxyCardPaint {
            shadow: Some(shadow),
            lift: Some(lift),
            ..marked
        },
        ProxyCardState::SelectedPressed => ProxyCardPaint {
            fill: palette.pressed_bg,
            ..marked
        },
    };
    if disabled {
        return ProxyCardPaint {
            fill: palette.surface_elevated.with_alpha(DISABLED_CARD_ALPHA),
            border: palette.border,
            border_width: rem(RESTING_CARD_BORDER_WIDTH_PX),
            shadow: None,
            lift: None,
        };
    }
    paint
}

type ProxyCardVisualQueryItem = (
    Entity,
    &'static ControlVisual,
    Option<&'static ButtonDisabled>,
    Option<&'static PickingInteraction>,
    &'static mut BackgroundColor,
    &'static mut BorderColor,
    &'static mut Node,
    Option<&'static mut BoxShadow>,
    &'static mut UiTransform,
);

type ProxyCardVisualQuery<'w, 's> = Query<'w, 's, ProxyCardVisualQueryItem, With<ProxyNodeButton>>;

/// Paint every mounted proxy card from the owner's selected bit and the
/// pointer's interaction.
///
/// The shared button skin paints a control's hover/press wash; the card is
/// the page's own surface, so this system runs after the skin and owns the
/// fill, the edge, the shadow and the lift.
pub fn sync_proxy_card_visuals(
    palette: Res<UiPalette>,
    mut commands: Commands,
    mut cards: ProxyCardVisualQuery,
) {
    for (
        entity,
        visual,
        disabled,
        interaction,
        mut fill,
        mut border,
        mut node,
        mut shadow,
        mut transform,
    ) in &mut cards
    {
        let paint = proxy_card_paint(
            resolve_proxy_card_state(visual.0, interaction.copied()),
            disabled.is_some_and(|disabled| disabled.0),
            &palette,
        );
        if fill.0 != paint.fill {
            fill.0 = paint.fill;
        }
        if border.top != paint.border {
            border.set_all(paint.border);
        }
        let edge = UiRect::all(paint.border_width);
        if node.border != edge {
            node.border = edge;
        }
        let shadow_styles = paint.shadow.map(|shadow| shadow.0).unwrap_or_default();
        match shadow.as_deref_mut() {
            Some(shadow) => {
                if shadow.0 != shadow_styles {
                    shadow.0 = shadow_styles;
                }
            }
            None => {
                if !shadow_styles.is_empty() {
                    commands.entity(entity).insert(BoxShadow(shadow_styles));
                }
            }
        }
        let translation = paint.lift.unwrap_or(Val2::ZERO);
        if transform.translation != translation {
            transform.translation = translation;
        }
    }
}

/// Paint the selection rail: the accent bar belongs to the card whose node is
/// the group's current outbound, and no other card carries one.
pub fn sync_proxy_selection_rails(
    palette: Res<UiPalette>,
    parents: Query<&ChildOf>,
    cards: Query<&ControlVisual, With<ProxyNodeButton>>,
    mut rails: Query<(Entity, &ProxyNodeSelectionRail, &mut BackgroundColor)>,
) {
    for (entity, _rail, mut fill) in &mut rails {
        // The rail sits inside the card's content row, so the owning card is
        // the nearest ancestor that carries the node's identity.
        let mut current = entity;
        let mut selected = false;
        loop {
            if cards.get(current).is_ok_and(|card| card.0) {
                selected = true;
                break;
            }
            let Ok(parent) = parents.get(current) else {
                break;
            };
            current = parent.parent();
        }
        let color = if selected {
            palette.accent
        } else {
            Color::NONE
        };
        if fill.0 != color {
            fill.0 = color;
        }
    }
}

/// Renders a single proxy node card with flag, title, protocol, features, and latency.
pub fn proxy_node_scene(
    g_idx: usize,
    n_idx: usize,
    group_name: &str,
    collection_id: Option<OutboundId>,
    node: &ProxyNode,
    selection_available: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    proxy_node_scene_sized(
        ProxyNodeSceneContext {
            group_idx: g_idx,
            node_idx: n_idx,
            group_name,
            collection_id,
            selection_available,
        },
        node,
        percent(31),
        palette,
    )
}

pub struct ProxyNodeSceneContext<'a> {
    pub group_idx: usize,
    pub node_idx: usize,
    pub group_name: &'a str,
    pub collection_id: Option<OutboundId>,
    pub selection_available: bool,
}

/// Renders a single proxy node card at an explicit column width. The
/// virtual-list node rows pass the shared wrapped-item percent so a recycled
/// row reuses the exact same card vocabulary as the full-mount path.
pub fn proxy_node_scene_sized(
    context: ProxyNodeSceneContext<'_>,
    node: &ProxyNode,
    width: Val,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let ProxyNodeSceneContext {
        group_idx: g_idx,
        node_idx: n_idx,
        group_name,
        collection_id,
        selection_available,
    } = context;
    let name = node.name.clone();
    let flag = if node.node_type.is_empty() {
        String::new()
    } else {
        node_flag_emoji(&node.name).into()
    };
    let latency = project_proxy_latency(node.delay_ms);
    let delay_color_val = latency_color(latency.band, palette);
    let delay_copy = LocalizedText::new(latency.key, latency.params);
    let protocol_chip: Vec<Box<dyn Scene>> = if node.node_type.is_empty() {
        Vec::new()
    } else {
        let proto_tag = proxy_protocol_chip(&node.node_type);
        vec![Box::new(bsn! {
            Node {
                padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                border_radius: BorderRadius::all(rem(radius::XS)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            ThemeBackgroundColor({ Token::AccentContainer })
            Children [ Text(proto_tag) NodeProtoText { group_idx: g_idx, node_idx: n_idx } TextRole(Role::Caption) ThemeTextColor({ Token::Accent }) ]
        })]
    };

    let paint = proxy_card_paint(
        resolve_proxy_card_state(node.selected, None),
        !selection_available,
        palette,
    );
    let card_shadow: Vec<ShadowStyle> = paint.shadow.map(|shadow| shadow.0).unwrap_or_default();

    let star = if node.favorite { "★" } else { "☆" };
    let star_color = if node.favorite {
        palette.warning
    } else {
        palette.ink_dim
    };

    let has_udp = node.features.iter().any(|f| f.eq_ignore_ascii_case("udp"));

    // The feature row lists every advertised tag once: the UDP capability has
    // its own chip, so the raw tag is not repeated beside it.
    let feature_chips: Vec<_> = node
        .features
        .iter()
        .filter(|feature| !feature.eq_ignore_ascii_case("udp"))
        .map(|f| {
            let f_str = f.clone();
            Box::new(bsn! {
                Node {
                    padding: UiRect::axes(rem(space::S4), rem(space::S2)),
                    // 3px chip corner: no radius token carries 3px.
                    border_radius: BorderRadius::MAX,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                ThemeBackgroundColor({ Token::Surface })
                Children [
                    Text({ f_str }) TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                ]
}) as Box<dyn Scene>
        })
        .collect();

    let udp_chip: Vec<Box<dyn Scene>> = if has_udp {
        vec![Box::new(bsn! {
                    Node {
                        padding: UiRect::axes(rem(space::S4), rem(space::S2)),
                        // 3px chip corner: no radius token carries 3px.
                        border_radius: BorderRadius::MAX,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Surface })
                    Children [
                        Text({ "UDP".to_owned() })
                        NodeUdpTag { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::Caption)
                        ThemeTextColor({ Token::InkDim })
                    ]
        }) as Box<dyn Scene>]
    } else {
        Vec::new()
    };

    bsn! {
            Node {
                width: { width },
                min_height: rem(PROXY_NODE_CARD_MIN_HEIGHT_PX),
                border: UiRect::all(paint.border_width),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                overflow: Overflow::clip(),
            }
            BackgroundColor({ paint.fill })
            BorderColor {
                top: { paint.border },
                right: { paint.border },
                bottom: { paint.border },
                left: { paint.border },
            }
            BoxShadow({ card_shadow })
            ControlVisual({ node.selected })
            ProxyNodeButton {
                group_idx: { g_idx },
                node_idx: { n_idx },
                group_name: { group_name.to_owned() },
                node_name: { node.name.clone() },
                collection_id: { collection_id.clone() },
                outbound_id: { node.outbound_id.clone() },
            }
            Button
            ButtonDisabled({ !selection_available })
            SwipeToActionItem {
                offset_x: 0.0,
                max_action_width: SWIPE_ACTION_DRAWER_WIDTH_PX,
            }
            SwipeToActionSpring::default()
            Children [
                Node {
                    width: percent(100),
                    min_height: rem(PROXY_NODE_CARD_MIN_HEIGHT_PX),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                    padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                }
                SwipeContentContainer
                Children [
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        Node {
                            position_type: PositionType::Relative,
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                            flex_grow: 1.0,
                            min_width: px(0.0),
                            overflow: Overflow::clip(),
                        }
                        Children [
                            Node {
                                width: rem(SELECTION_RAIL_WIDTH_PX),
                                height: rem(space::S16),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                                flex_shrink: 0.0,
                            }
                            ProxyNodeSelectionRail
                            --
                            Text({ flag.to_owned() })
                            NodeFlagText { group_idx: g_idx, node_idx: n_idx }
                            TextRole(Role::BodyStrong)
                            --
                            Text({ name.clone() })
                            NodeNameText { group_idx: g_idx, node_idx: n_idx }
                            TextRole(Role::BodyStrong)
                            --
                            @{ name_fade_scene(palette.over_surface(paint.fill)) }
                        ]
                        --
                        Node {
                            padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                            border_radius: BorderRadius::all(rem(radius::SM)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            column_gap: rem(space::S4),
                            flex_shrink: 0.0,
                        }
                        ThemeBackgroundColor({ Token::Surface })
                        LatencySkeletonPulse { group_idx: g_idx, node_idx: n_idx }
                        Children [
                            @{ latency_sparkline_scene(g_idx, n_idx) }
                            --
                            LocalizedText::new(delay_copy.key,delay_copy.params)
                            LatencyText { group_idx: g_idx, node_idx: n_idx }
                            TextRole(Role::Mono)
                            TextColor(delay_color_val)
                        ]
                    ]
                    --
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S4),
                            flex_wrap: FlexWrap::Wrap,
                            flex_grow: 1.0,
                            min_width: px(0.0),
                        }
                        Children [
                            { protocol_chip }
                            --
                            { udp_chip }
                            --
                            { feature_chips }
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                            flex_shrink: 0.0,
                        }
                        Children [
                            Text({ star.to_owned() })
                            ProxyFavoriteStar
                            TextRole(Role::BodyStrong)
                            TextColor({ star_color })
                            --
                            Text({ "📌".to_owned() })
                            NodePinButton {
                                group_idx: g_idx,
                                node_idx: n_idx,
                                node_name: { name.clone() },
                                outbound_id: { node.outbound_id.clone() },
                            }
                            TextRole(Role::Caption)
                            ThemeTextColor({ if node.favorite { Token::Warning } else { Token::InkDim } })
                        --
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            flex_shrink: 0.0,
                        }
                        NodeDetailButton {
                            group_idx: g_idx,
                            node_idx: n_idx,
                            node_name: { name.clone() },
                            outbound_id: { node.outbound_id.clone() },
                        }
                        Button
                        Children [
                            @{ icon_scene(IconId::FileText, DETAIL_ICON_SIZE_PX, palette.ink_dim) }
                        ]
                    ]
                ]
                ]
                --
                Node {
                    position_type: PositionType::Absolute,
                    right: px(0.0),
                    top: px(0.0),
                    bottom: px(0.0),
                    width: rem(SWIPE_ACTION_DRAWER_WIDTH_PX),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: rem(space::S4),
                    display: Display::None,
                }
                SwipeActionDrawer
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Children [
                    Node {
                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Warning })
                    Button
                    NodePinButton {
                        group_idx: { g_idx },
                        node_idx: { n_idx },
                        node_name: { name },
                        outbound_id: { node.outbound_id.clone() },
                    }
                    Children [
                        Text({ "★".to_owned() }) TextRole(Role::BodyStrong) ThemeTextColor({ Token::Surface })
                    ]
                    --
                    Node {
                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Accent })
                    Button
                    TestProxyGroupButton {
                        group_idx: { g_idx },
                        collection_id: { collection_id.clone() },
                    }
                    Children [
                        Text({ "⚡".to_owned() }) TextRole(Role::BodyStrong) ThemeTextColor({ Token::Surface })
                    ]
                ]
            ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_bevy_widgets::theme::Theme;

    #[test]
    fn the_card_state_resolves_the_owner_fact_and_the_pointer_fact() {
        assert_eq!(
            resolve_proxy_card_state(false, None),
            ProxyCardState::Resting
        );
        assert_eq!(
            resolve_proxy_card_state(false, Some(PickingInteraction::Hovered)),
            ProxyCardState::Hovered
        );
        assert_eq!(
            resolve_proxy_card_state(false, Some(PickingInteraction::Pressed)),
            ProxyCardState::Pressed
        );
        assert_eq!(
            resolve_proxy_card_state(true, None),
            ProxyCardState::Selected
        );
        assert_eq!(
            resolve_proxy_card_state(true, Some(PickingInteraction::Hovered)),
            ProxyCardState::SelectedHovered
        );
        assert_eq!(
            resolve_proxy_card_state(true, Some(PickingInteraction::Pressed)),
            ProxyCardState::SelectedPressed
        );
    }

    #[test]
    fn the_selected_card_keeps_the_reading_surface() {
        let palette = UiPalette::new(&Theme::dark());
        let selected = proxy_card_paint(ProxyCardState::Selected, false, &palette);
        let resting = proxy_card_paint(ProxyCardState::Resting, false, &palette);
        // The accent rides the edge, never the whole card: the fill stays the
        // reading surface, so the node's own text keeps its contrast.
        assert_eq!(selected.fill, resting.fill);
        assert_eq!(selected.border, palette.accent);
        assert_ne!(resting.border, palette.accent);
    }

    #[test]
    fn the_hover_lifts_and_shadows_the_card() {
        let palette = UiPalette::new(&Theme::dark());
        let hovered = proxy_card_paint(ProxyCardState::Hovered, false, &palette);
        let resting = proxy_card_paint(ProxyCardState::Resting, false, &palette);
        assert!(hovered.shadow.is_some());
        assert!(resting.shadow.is_none());
        assert!(hovered.lift.is_some());
        assert!(resting.lift.is_none());
    }

    #[test]
    fn a_card_whose_group_cannot_be_selected_is_washed_out() {
        let palette = UiPalette::new(&Theme::dark());
        let disabled = proxy_card_paint(ProxyCardState::Selected, true, &palette);
        let selected = proxy_card_paint(ProxyCardState::Selected, false, &palette);
        assert_ne!(disabled.fill, selected.fill);
        assert!(disabled.shadow.is_none());
        assert!(disabled.lift.is_none());
    }
}
