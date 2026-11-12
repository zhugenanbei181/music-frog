//! The Overview policy-group quick selector: a passive two-column projection
//! of the accepted outbound inventory and one typed selection intent.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::overview::LastOverviewProjection;
use crate::pages::proxies::outbound_selection_request;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::SystemParam;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut};
use bevy::feathers::theme::{SurfaceLevel, ThemeBackgroundColor, ThemeContext, ThemeTextColor};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::{LineBreak, TextColor, TextLayout};
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, GlobalZIndex,
    JustifyContent, Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{
    Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose, ScrollArea,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::smart_truncate::truncate_tail;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_contract::design_resources;
use infiltrator_presentation::dashboard_hero_projection::{
    PolicyGroupGridItem, PolicyGroupGridProjection, PolicyGroupMemberProjection,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::SelectionAvailability;

/// The dashboard keeps at most eight policy groups visible; the full count is
/// published by the authoritative inventory projection.
const POLICY_GRID_MAX_VISIBLE_GROUPS: usize = 8;
const POLICY_TILE_NAME_MAX_CHARS: usize = 12;
const POLICY_TILE_NODE_MAX_CHARS: usize = 16;
const POLICY_PICKER_MAX_WIDTH_PX: f32 = 420.0;
/// The picker row's leading group badge square, in baseline pixels.
const POLICY_PICKER_BADGE_PX: f32 = 32.0;

/// Marker on the Overview policy-group card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewPolicyGroupCard;

/// Marker on the container whose bounded subtree holds the two-column grid.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolicyGroupGridContainer;

/// Click target that opens one policy group's node picker.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub struct PolicyGroupPickerButton {
    pub collection_id: OutboundId,
}

impl Default for PolicyGroupPickerButton {
    fn default() -> Self {
        Self {
            collection_id: template_outbound_id("policy-group-template"),
        }
    }
}

/// Marker on the modal root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolicyGroupPickerRoot;

/// Marker on the modal's bounded member list.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolicyGroupPickerList;

/// Marker on the modal's dismiss control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Button)]
pub struct PolicyGroupPickerClose;

/// Click target that submits one outbound selection.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[require(Button)]
pub struct PolicyGroupMemberButton {
    pub collection_id: OutboundId,
    pub outbound_id: OutboundId,
}

impl Default for PolicyGroupMemberButton {
    fn default() -> Self {
        Self {
            collection_id: template_outbound_id("policy-group-template"),
            outbound_id: template_outbound_id("policy-member-template"),
        }
    }
}

/// Marker on the picker's dynamic title.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolicyGroupPickerTitle;

/// The exclusive picker state. Opening is keyed by the collection identity;
/// no boolean can claim "open" without naming the owner fact it displays.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum PolicyGroupPickerState {
    #[default]
    Closed,
    Open {
        collection_id: OutboundId,
    },
}

/// The policy-group card: header, bounded two-column grid and modal picker.
pub fn policy_group_card_scene(
    projection: &PolicyGroupGridProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let count = projection.total_count.to_string();
    surface_scene(
        vec![
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            @{ icon_scene(IconId::Settings, 16.0, palette.accent) }
                            --
                            LocalizedText::plain("overview_policy_groups") TextRole(design_resources::TextRole::Heading)
                        ]
                        --
                        Node {
                            padding: UiRect::axes(rem(space::S8), rem(space::S2)),
                            border_radius: BorderRadius::all(rem(radius::XS)),
                        }
                        ThemeBackgroundColor({ Token::AccentContainer })
                        Children [
                            Text({ count }) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::Accent })
                        ]
                    ]
            }),
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        min_width: px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S8),
                    }
                    PolicyGroupGridContainer
                    Children [
                        { policy_group_grid_rows(projection, palette) }
                    ]
            }),
        ],
        palette,
    )
}

/// The modal picker scene. The Overview page owns it and the shell mounts it
/// beside its own card. Its root is a `FixedNode` with the floating theme
/// context, so the page's scroll viewport neither lays out nor clips it, and
/// it still reads correctly from any host subtree. Only its state and display
/// change after the one mount.
pub fn policy_group_picker_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mut semantic = accesskit::Node::new(accesskit::Role::Dialog);
    semantic.set_label("policy_group_picker");
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            PolicyGroupPickerRoot
            GlobalZIndex(115)
            ModalScrim
            ThemeBackgroundColor({ Token::Scrim })
            ModalDialogBarrier
            ThemeContext(SurfaceLevel::Floating)
            FixedNode
            Children [
                Node {
                    width: percent(90),
                    max_width: rem(POLICY_PICKER_MAX_WIDTH_PX),
                    max_height: percent(80),
                    min_height: px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S12),
                    padding: UiRect::all(rem(space::S16)),
                    border_radius: BorderRadius::all(rem(palette.modal_radius_px)),
                }
                ModalDialog
                ThemeBackgroundColor({ Token::Surface })
                AccessibilityNode(semantic)
                Children [
                    Text(String::new())
                    PolicyGroupPickerTitle
                    TextRole(design_resources::TextRole::Heading)
                    --
                    LocalizedText::plain("overview_policy_group_picker_hint")
                    TextRole(design_resources::TextRole::Caption)
                    ThemeTextColor({ Token::InkDim })
                    --
                    Node {
                        width: percent(100),
                        min_height: px(0.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S8),
                        overflow: Overflow::scroll_y(),
                    }
                    ScrollArea
                    PolicyGroupPickerList
                    --
                    Node {
                        width: percent(100),
                        justify_content: JustifyContent::FlexEnd,
                    }
                    Children [
                        Node {
                            min_height: rem(palette.control_height_px),
                            padding: UiRect::axes(rem(space::S12), rem(space::S6)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        }
                        Button
                        PolicyGroupPickerClose
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        Children [
                            LocalizedText::plain("modal_cancel") TextRole(design_resources::TextRole::Body)
                        ]
                    ]
                ]
            ]
    }
}

/// Rebuild the bounded grid only when the authoritative projection changes.
pub(crate) fn sync_policy_group_grid(
    last: Res<LastOverviewProjection>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    containers: Query<(Entity, Option<&Children>), With<PolicyGroupGridContainer>>,
    mut rendered: Local<Option<PolicyGroupGridProjection>>,
) {
    let Some(projection) = last
        .0
        .as_ref()
        .map(|projection| projection.policy_groups.clone())
    else {
        return;
    };
    if rendered.as_ref() == Some(&projection) {
        return;
    }
    *rendered = Some(projection.clone());
    for (container, _) in &containers {
        commands.entity(container).despawn_children();
        for row in projection
            .groups
            .iter()
            .take(POLICY_GRID_MAX_VISIBLE_GROUPS)
            .collect::<Vec<_>>()
            .chunks(2)
        {
            commands
                .spawn_scene(policy_group_row_scene(row, &palette))
                .insert(ChildOf(container));
        }
        if projection.groups.is_empty() {
            commands
                .spawn_scene(policy_group_empty_scene(&projection, &palette))
                .insert(ChildOf(container));
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct PolicyGroupPickerQueries<'w, 's> {
    pub roots: Query<'w, 's, &'static mut Node, With<PolicyGroupPickerRoot>>,
    pub titles: Query<'w, 's, &'static mut Text, With<PolicyGroupPickerTitle>>,
    pub lists: Query<'w, 's, (Entity, Option<&'static Children>), With<PolicyGroupPickerList>>,
}

/// Keep the modal's exclusive state projected onto its real surface.
pub(crate) fn sync_policy_group_picker(
    mut state: ResMut<PolicyGroupPickerState>,
    last: Res<LastOverviewProjection>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    mut queries: PolicyGroupPickerQueries<'_, '_>,
    mut rendered: Local<Option<(Option<OutboundId>, usize)>>,
) {
    let projection = last.0.as_ref();
    let desired = match &*state {
        PolicyGroupPickerState::Closed => None,
        PolicyGroupPickerState::Open { collection_id } => projection.and_then(|projection| {
            projection
                .policy_groups
                .groups
                .iter()
                .find(|group| &group.collection_id == collection_id)
                .cloned()
        }),
    };
    if desired.is_none()
        && matches!(&*state, PolicyGroupPickerState::Open { .. })
        && state.is_changed()
    {
        *state = PolicyGroupPickerState::Closed;
    }
    let display = if desired.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for mut root in &mut queries.roots {
        if root.display != display {
            root.display = display;
        }
    }
    if let Some(group) = &desired
        && let Ok(mut title) = queries.titles.single_mut()
    {
        let copy = LocalizedText::new(
            "overview_policy_group_picker_title",
            vec![("name", group.name.clone())],
        )
        .render(&locale);
        if title.0 != copy {
            title.0 = copy;
        }
    }
    let signature = desired
        .as_ref()
        .map(|group| (Some(group.collection_id.clone()), group.members.len()));
    if *rendered == signature {
        return;
    }
    *rendered = signature;
    for (list, _) in &queries.lists {
        commands.entity(list).despawn_children();
        if let Some(group) = &desired {
            for member in &group.members {
                commands
                    .spawn_scene(policy_group_member_scene(group, member, &locale, &palette))
                    .insert(ChildOf(list));
            }
        }
    }
}

/// Close the picker when its page is replaced; the route is the owner fact.
pub(crate) fn close_policy_group_picker_on_route(
    route: Res<ActiveRoute>,
    mut state: ResMut<PolicyGroupPickerState>,
    mut previous: Local<Option<Route>>,
) {
    if previous.as_ref() != route.0.as_ref() {
        *state = PolicyGroupPickerState::Closed;
        *previous = route.0;
    }
}

/// Turn an activation on the card or modal into one typed state transition or
/// one shared selection intent. The UI never calls a controller directly.
pub(crate) fn on_policy_group_activated(
    activate: On<Activate>,
    tiles: Query<&PolicyGroupPickerButton>,
    members: Query<&PolicyGroupMemberButton>,
    closes: Query<(), With<PolicyGroupPickerClose>>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    sink: Option<Res<CommandSinkHandle>>,
    mut state: ResMut<PolicyGroupPickerState>,
) {
    if let Ok(tile) = tiles.get(activate.entity) {
        *state = PolicyGroupPickerState::Open {
            collection_id: tile.collection_id.clone(),
        };
        return;
    }
    if closes.contains(activate.entity) {
        *state = PolicyGroupPickerState::Closed;
        return;
    }
    let Ok(member) = members.get(activate.entity) else {
        return;
    };
    if let Some(request) = latest.as_deref().and_then(|snapshot| {
        outbound_selection_request(&snapshot.0, &member.collection_id, &member.outbound_id)
    }) && let Some(handle) = sink
    {
        handle.submit(UiCommand::SelectOutbound { request });
    }
    *state = PolicyGroupPickerState::Closed;
}

/// The official modal barrier's close request owns the dismiss transition.
pub(crate) fn on_policy_group_request_close(
    close: On<RequestClose>,
    roots: Query<(), With<PolicyGroupPickerRoot>>,
    mut state: ResMut<PolicyGroupPickerState>,
) {
    if roots.contains(close.original_event_target()) {
        *state = PolicyGroupPickerState::Closed;
    }
}

fn policy_group_grid_rows(
    projection: &PolicyGroupGridProjection,
    palette: &UiPalette,
) -> Vec<Box<dyn Scene>> {
    projection
        .groups
        .iter()
        .take(POLICY_GRID_MAX_VISIBLE_GROUPS)
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|row| Box::new(policy_group_row_scene(row, palette)) as Box<dyn Scene>)
        .collect()
}

fn policy_group_row_scene(
    groups: &[&PolicyGroupGridItem],
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut children: Vec<Box<dyn Scene>> = groups
        .iter()
        .map(|group| Box::new(policy_group_tile_scene(group, palette)) as Box<dyn Scene>)
        .collect();
    if children.len() == 1 {
        children.push(Box::new(bsn! {
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0.0),
                    min_width: px(0.0),
                }
        }));
    }
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                column_gap: rem(space::S8),
            }
            Children [
                { children }
            ]
    }
}

fn policy_group_tile_scene(group: &PolicyGroupGridItem, palette: &UiPalette) -> impl Scene + use<> {
    let selectable = group.availability == SelectionAvailability::Selectable;
    let locale = UiLocale::default();
    let unselected = locale.text("overview_policy_group_unselected");
    let selected = group.selected_node.as_ref().map_or_else(
        || unselected.clone(),
        |node| {
            format!(
                "{} {}",
                group.selected_flag_code.as_deref().unwrap_or("—"),
                truncate_tail(node, POLICY_TILE_NODE_MAX_CHARS)
            )
        },
    );
    let mut semantic = accesskit::Node::new(accesskit::Role::Button);
    semantic.set_label(format!("{} {}", group.name, selected));
    bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: px(0.0),
                min_width: px(0.0),
                min_height: rem(palette.control_height_px),
                padding: UiRect::all(rem(space::S8)),
                align_items: AlignItems::Center,
                column_gap: rem(space::S8),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            Button
            ButtonDisabled({ !selectable })
            PolicyGroupPickerButton { collection_id: { group.collection_id.clone() } }
            AccessibilityNode(semantic)
            Children [
                Node {
                    width: rem(POLICY_PICKER_BADGE_PX),
                    height: rem(POLICY_PICKER_BADGE_PX),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor({ palette.accent_container })
                Children [
                    Text({ group.avatar_char.clone() }) TextRole(design_resources::TextRole::BodyStrong) TextColor({ palette.accent })
                ]
                --
                Node {
                    flex_grow: 1.0,
                    min_width: px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S2),
                    overflow: Overflow::clip_x(),
                }
                Children [
                    Text({ truncate_tail(&group.name, POLICY_TILE_NAME_MAX_CHARS) })
                    TextRole(design_resources::TextRole::BodyStrong)
                    TextLayout { linebreak: LineBreak::NoWrap, ..TextLayout::default() }
                    --
                    Text({ selected })
                    TextRole(design_resources::TextRole::Caption)
                    ThemeTextColor({ Token::InkDim })
                    TextLayout { linebreak: LineBreak::NoWrap, ..TextLayout::default() }
                ]
            ]
    }
}

fn policy_group_empty_scene(
    projection: &PolicyGroupGridProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let copy = if projection.observed {
        "overview_policy_group_unselected"
    } else {
        "overview_policy_group_unavailable"
    };
    bsn! {
            Node {
                width: percent(100),
                padding: UiRect::all(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            Children [
                LocalizedText::plain(copy) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::InkDim })
            ]
    }
}

fn policy_group_member_scene(
    group: &PolicyGroupGridItem,
    member: &PolicyGroupMemberProjection,
    locale: &UiLocale,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let selected = group.selected_outbound_id.as_ref() == Some(&member.outbound_id);
    let label = format!("{} {}", member.flag_code, member.display_name);
    let mut semantic = accesskit::Node::new(accesskit::Role::Button);
    semantic.set_label(if selected {
        format!("{label} · {}", locale.text("overview_active_exit_label"))
    } else {
        label.clone()
    });
    let selected_copy = if selected {
        LocalizedText::plain("overview_active_exit_label").render(locale)
    } else {
        String::new()
    };
    bsn! {
            Node {
                width: percent(100),
                min_height: rem(palette.control_height_px),
                padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: rem(space::S8),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({
                if selected {
                    palette.accent_container
                } else {
                    palette.surface_elevated
                }
            })
            Button
            PolicyGroupMemberButton {
                collection_id: { group.collection_id.clone() },
                outbound_id: { member.outbound_id.clone() },
            }
            AccessibilityNode(semantic)
            Children [
                Node {
                    flex_grow: 1.0,
                    min_width: px(0.0),
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S8),
                    overflow: Overflow::clip_x(),
                }
                Children [
                    Text({ member.flag_code.clone() }) TextRole(design_resources::TextRole::Body)
                    --
                    Text({ member.display_name.clone() })
                    TextRole(design_resources::TextRole::Body)
                    TextLayout { linebreak: LineBreak::NoWrap, ..TextLayout::default() }
                ]
                --
                Text({ selected_copy }) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::Accent })
            ]
    }
}

fn template_outbound_id(value: &str) -> OutboundId {
    OutboundId::new(
        EngineId::new("bevy-template").expect("template engine ID is valid"),
        value,
    )
    .expect("template outbound ID is valid")
}
