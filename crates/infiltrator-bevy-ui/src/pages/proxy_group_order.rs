//! Native independent order editor; actual observers submit the whole shared draft.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::proxies::{
    ProxyGroupMoveDownButton, ProxyGroupMoveUpButton, ResetProxyGroupOrderButton,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::{ButtonInput, keyboard::KeyCode};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, GlobalZIndex, JustifyContent, Node,
    Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::proxy_group_order_editor::{GroupMove, ProxyGroupOrderEditor};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageStatus;
use std::collections::{HashMap, HashSet};

#[derive(Resource, Default)]
pub struct GroupOrderState {
    pub open: bool,
    pub editor: ProxyGroupOrderEditor,
    pub request: Option<(RequestId, u64)>,
}
#[derive(Component, Clone, Debug, Default)]
pub enum GroupOrderAction {
    Move {
        name: String,
        direction: GroupMove,
    },
    Reset,
    #[default]
    Cancel,
    Apply,
}
#[derive(Component, Clone, Copy, Default)]
pub struct GroupOrderRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct GroupOrderCard;
#[derive(Component, Clone, Copy, Default)]
pub struct GroupOrderList;
#[derive(Component, Clone, Debug, Default)]
pub struct GroupOrderRow(pub String);
#[derive(Component, Clone, Copy, Default)]
pub struct GroupOrderStatus;
#[derive(Component, Clone, Copy, Default)]
#[require(Button)]
pub struct GroupOrderApply;

pub fn modal(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None, align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        GroupOrderRoot GlobalZIndex(115)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim BackgroundColor({ palette.scrim }) Button GroupOrderAction::Cancel ButtonDisabled(false)
            --
            Node { width: percent(90), max_width: px(520.0), max_height: percent(90), padding: UiRect::all(px(16.0)), flex_direction: FlexDirection::Column, row_gap: px(12.0) }
            ModalDialogCard GroupOrderCard BackgroundColor({ palette.surface }) AccessibilityNode(semantic) LocalizedLabel::plain("proxies_reorder_title")
            Children [
                LocalizedText::plain("proxies_reorder_title") TextRole(Role::Heading)
                --
                LocalizedText::plain("proxies_reorder_description") TextRole(Role::Caption)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0, flex_direction: FlexDirection::Column, row_gap: px(8.0), overflow: Overflow::scroll_y() }
                ScrollArea GroupOrderList
                --
                Text(String::new()) GroupOrderStatus TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: px(8.0), justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0 }
                Children [
                    Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                    Button GroupOrderAction::Cancel ButtonDisabled(false)
                    Children [ LocalizedText::plain("modal_cancel") TextRole(Role::Body) ]
                    --
                    Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                    Button GroupOrderAction::Reset ButtonDisabled(false)
                    Children [ LocalizedText::plain("proxies_reorder_reset") TextRole(Role::Body) ]
                    --
                    Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                    Button GroupOrderAction::Apply GroupOrderApply ButtonDisabled(true)
                    Children [ LocalizedText::plain("proxies_reorder_apply") TextRole(Role::BodyStrong) ]
                ]
            ]
        ]
    }
}
fn row_scene(name: String, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), column_gap: px(8.0), align_items: AlignItems::Center, flex_shrink: 0.0 }
        GroupOrderRow({ name.clone() })
        Children [
            Node { flex_grow: 1.0, min_width: px(0.0) }
            Children [ Text({ name.clone() }) TextRole(Role::Body) ]
            --
            Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
            Button GroupOrderAction::Move { name: { name.clone() }, direction: GroupMove::Up } ButtonDisabled(false)
            LocalizedLabel({ LocalizedText::new("proxies_reorder_up_label", vec![("name", name.clone())]) })
            Children [ Text::new("▲") TextRole(Role::Caption) ]
            --
            Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
            Button GroupOrderAction::Move { name: { name.clone() }, direction: GroupMove::Down } ButtonDisabled(false)
            LocalizedLabel({ LocalizedText::new("proxies_reorder_down_label", vec![("name", name.clone())]) })
            Children [ Text::new("▼") TextRole(Role::Caption) ]
        ]
    }
}
#[derive(SystemParam)]
pub struct OrderControls<'w, 's> {
    up: Query<'w, 's, &'static ProxyGroupMoveUpButton>,
    down: Query<'w, 's, &'static ProxyGroupMoveDownButton>,
    reset: Query<'w, 's, (), With<ResetProxyGroupOrderButton>>,
    actions: Query<'w, 's, &'static GroupOrderAction>,
}
fn observed_groups(latest: &LatestSurfaceSnapshot) -> Result<Vec<String>, Failure> {
    let page = &latest.0.pages.proxies;
    match &page.status {
        PageStatus::Ready | PageStatus::Empty => page
            .data
            .as_ref()
            .map(|page| page.groups.iter().map(|group| group.name.clone()).collect())
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::Internal,
                    "proxy page has no group identities",
                    true,
                )
            }),
        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
            Err(failure.clone())
        }
        PageStatus::Loading => Err(Failure::new(
            ErrorCode::NotReady,
            "group observations are refreshing",
            true,
        )),
    }
}
pub fn activate_order(
    event: On<Activate>,
    controls: OrderControls,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<GroupOrderState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    let external = if let Ok(button) = controls.up.get(event.entity) {
        Some(GroupOrderAction::Move {
            name: button.group_name.clone(),
            direction: GroupMove::Up,
        })
    } else if let Ok(button) = controls.down.get(event.entity) {
        Some(GroupOrderAction::Move {
            name: button.group_name.clone(),
            direction: GroupMove::Down,
        })
    } else if controls.reset.contains(event.entity) {
        Some(GroupOrderAction::Reset)
    } else {
        None
    };
    if external.is_some() && !state.open {
        let groups = match observed_groups(&latest) {
            Ok(groups) => groups,
            Err(failure) => {
                state.editor.failure = Some(failure);
                return;
            }
        };
        if let Err(failure) = state.editor.open(groups) {
            state.editor.failure = Some(failure);
            return;
        }
        state.open = true;
    }
    let Some(action) = external.or_else(|| controls.actions.get(event.entity).ok().cloned()) else {
        return;
    };
    if !state.open {
        return;
    }
    match action {
        GroupOrderAction::Move { name, direction } => {
            state.editor.move_group(&name, direction);
        }
        GroupOrderAction::Reset => state.editor.reset_draft(),
        GroupOrderAction::Cancel => {
            if state.editor.pending.is_none() {
                state.editor.cancel();
                state.open = false;
            }
        }
        GroupOrderAction::Apply => {
            let pending = match state.editor.begin() {
                Ok(pending) => pending,
                Err(_) => return,
            };
            if let Some(request) = sink.and_then(|sink| {
                sink.submit_tracked(UiCommand::ReorderProxyGroups {
                    group_names: pending.groups,
                })
            }) {
                state.request = Some((request, pending.token));
            } else {
                state.editor.finish(
                    pending.token,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "group order has no terminal command service",
                        true,
                    )),
                );
            }
        }
    }
}
pub fn finish_order(event: On<CommandExecutedEvent>, mut state: ResMut<GroupOrderState>) {
    let Some((request, token)) = state.request else {
        return;
    };
    let UiCommand::ReorderProxyGroups { group_names } = &event.command else {
        return;
    };
    if event.request_id != request
        || !state
            .editor
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token && pending.groups == *group_names)
    {
        return;
    }
    let success = event.unit_result().is_ok();
    if state.editor.finish(token, event.unit_result()) {
        state.request = None;
        if success {
            state.open = false;
        }
    }
}
pub fn sync_order_editor(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    route: Res<ActiveRoute>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<GroupOrderState>,
) {
    if !state.open {
        return;
    }
    if route.0 != Some(Route::Proxies) {
        if state.editor.pending.is_none() {
            state.editor.cancel();
        }
        state.open = false;
        return;
    }
    if keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)) && state.editor.pending.is_none()
    {
        state.editor.cancel();
        state.open = false;
        return;
    }
    state.editor.observe(observed_groups(&latest));
}
pub fn reconcile_order_rows(
    mut commands: Commands,
    state: Res<GroupOrderState>,
    palette: Res<UiPalette>,
    roots: Query<Entity, With<GroupOrderList>>,
    rows: Query<(Entity, &GroupOrderRow)>,
) {
    if !state.open {
        return;
    }
    let Ok(root) = roots.single() else { return };
    let existing: HashSet<_> = rows.iter().map(|(_, row)| row.0.as_str()).collect();
    for name in &state.editor.draft {
        if !existing.contains(name.as_str()) {
            commands
                .spawn_scene(row_scene(name.clone(), &palette))
                .insert(ChildOf(root));
        }
    }
    for (entity, row) in &rows {
        if !state.editor.draft.contains(&row.0) {
            commands.entity(entity).despawn();
        }
    }
}
pub fn sort_order_rows(
    state: Res<GroupOrderState>,
    mut roots: Query<&mut Children, With<GroupOrderList>>,
    rows: Query<&GroupOrderRow>,
) {
    let ranks: HashMap<_, _> = state
        .editor
        .draft
        .iter()
        .enumerate()
        .map(|(rank, name)| (name.as_str(), rank))
        .collect();
    for mut children in &mut roots {
        let sorted = children.iter().enumerate().all(|(rank, entity)| {
            rows.get(*entity)
                .ok()
                .and_then(|row| ranks.get(row.0.as_str()).copied())
                == Some(rank)
        });
        if !sorted {
            children.sort_by_key(|entity| {
                rows.get(*entity)
                    .ok()
                    .and_then(|row| ranks.get(row.0.as_str()).copied())
                    .unwrap_or(usize::MAX)
            });
        }
    }
}
pub fn sync_order_surface(
    state: Res<GroupOrderState>,
    locale: Res<UiLocale>,
    mut roots: Query<&mut Node, With<GroupOrderRoot>>,
    mut buttons: Query<(&GroupOrderAction, &mut ButtonDisabled)>,
    mut status: Query<&mut Text, With<GroupOrderStatus>>,
) {
    for mut root in &mut roots {
        let display = if state.open {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != display {
            root.display = display;
        }
    }
    for (action, mut button) in &mut buttons {
        let disabled = match action {
            GroupOrderAction::Apply => !state.editor.can_apply(),
            GroupOrderAction::Move { name, direction } => {
                state.editor.pending.is_some()
                    || state
                        .editor
                        .draft
                        .iter()
                        .position(|item| item == name)
                        .is_none_or(|index| match direction {
                            GroupMove::Up => index == 0,
                            GroupMove::Down => index + 1 == state.editor.draft.len(),
                        })
            }
            _ => state.editor.pending.is_some(),
        };
        if button.0 != disabled {
            button.0 = disabled;
        }
    }
    let message = state
        .editor
        .failure
        .as_ref()
        .or(state.editor.observation_failure.as_ref())
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if state.editor.pending.is_some() {
                LocalizedText::plain("core_control_pending").render(&locale)
            } else {
                String::new()
            }
        });
    for mut text in &mut status {
        if text.0 != message {
            text.0 = message.clone();
        }
    }
}
