//! Native independent group order editor using opaque outbound collection IDs.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::proxies::{
    ProxyGroupMoveDownButton, ProxyGroupMoveUpButton, ResetProxyGroupOrderButton,
    outbound_ui_preferences,
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
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input::{ButtonInput, keyboard::KeyCode};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, Display, FlexDirection, GlobalZIndex, JustifyContent, Node, Overflow, PositionType,
    UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::{
    identity::{OutboundId, SnapshotRevision},
    inventory::OutboundInventoryState,
    preferences::{OutboundUiPreferenceIntent, OutboundUiPreferenceRequest},
};
use lurker_core_contract::page::PageStatus;
use lurker_ui_presentation::proxy_group_order_editor::{
    GroupMove, GroupOrderFinish, GroupOrderOperation, ProxyGroupOrderEditor,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupOrderDialogState {
    #[default]
    Closed,
    Editing,
}

#[derive(Resource, Default)]
pub struct GroupOrderState {
    pub dialog: GroupOrderDialogState,
    pub editor: ProxyGroupOrderEditor,
    pub request: GroupOrderRequestState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GroupOrderRequestState {
    #[default]
    Idle,
    Submitted {
        request_id: RequestId,
        request: OutboundUiPreferenceRequest,
    },
}
#[derive(Component, Clone, Debug, Default)]
pub enum GroupOrderAction {
    Move {
        group_id: OutboundId,
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
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct GroupOrderRow {
    pub id: OutboundId,
    pub label: String,
}
impl Default for GroupOrderRow {
    fn default() -> Self {
        Self {
            id: OutboundId::new(
                EngineId::new("bevy-template").expect("static scene template engine ID is valid"),
                "group-order-template",
            )
            .expect("static scene template outbound ID is valid"),
            label: String::new(),
        }
    }
}
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
            ModalScrim ThemeBackgroundColor({ Token::Scrim }) Button GroupOrderAction::Cancel ButtonDisabled(false)
            --
            Node { width: percent(90), max_width: rem(520.0), max_height: percent(90), padding: UiRect::all(rem(16.0)), flex_direction: FlexDirection::Column, row_gap: rem(12.0) }
            ModalDialogCard GroupOrderCard ThemeBackgroundColor({ Token::Surface }) AccessibilityNode(semantic) LocalizedLabel::plain("proxies_reorder_title")
            Children [
                LocalizedText::plain("proxies_reorder_title") TextRole(Role::Heading)
                --
                LocalizedText::plain("proxies_reorder_description") TextRole(Role::Caption)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0, flex_direction: FlexDirection::Column, row_gap: rem(8.0), overflow: Overflow::scroll_y() }
                ScrollArea GroupOrderList
                --
                Text(String::new()) GroupOrderStatus TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: rem(8.0), justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0 }
                Children [
                    Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)) }
                    Button GroupOrderAction::Cancel ButtonDisabled(false)
                    Children [ LocalizedText::plain("modal_cancel") TextRole(Role::Body) ]
                    --
                    Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)) }
                    Button GroupOrderAction::Reset ButtonDisabled(false)
                    Children [ LocalizedText::plain("proxies_reorder_reset") TextRole(Role::Body) ]
                    --
                    Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)) }
                    Button GroupOrderAction::Apply GroupOrderApply ButtonDisabled(true)
                    Children [ LocalizedText::plain("proxies_reorder_apply") TextRole(Role::BodyStrong) ]
                ]
            ]
        ]
    }
}

fn row_scene(id: OutboundId, label: String, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), column_gap: rem(8.0), align_items: AlignItems::Center, flex_shrink: 0.0 }
        GroupOrderRow { id: { id.clone() }, label: { label.clone() } }
        Children [
            Node { flex_grow: 1.0, min_width: px(0.0) }
            Children [ Text({ label.clone() }) TextRole(Role::Body) ]
            --
            Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)) }
            Button GroupOrderAction::Move { group_id: { id.clone() }, direction: GroupMove::Up } ButtonDisabled(false)
            LocalizedLabel({ LocalizedText::new("proxies_reorder_up_label", vec![("name", label.clone())]) })
            Children [ Text::new("▲") TextRole(Role::Caption) ]
            --
            Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)) }
            Button GroupOrderAction::Move { group_id: { id.clone() }, direction: GroupMove::Down } ButtonDisabled(false)
            LocalizedLabel({ LocalizedText::new("proxies_reorder_down_label", vec![("name", label.clone())]) })
            Children [ Text::new("▼") TextRole(Role::Caption) ]
        ]
    }
}

fn observed_group_order(
    latest: &LatestSurfaceSnapshot,
) -> Result<
    (
        Vec<OutboundId>,
        Vec<OutboundId>,
        SnapshotRevision,
        CoreScope,
    ),
    Failure,
> {
    let scope = latest.0.core.scope().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "active core scope is not observed",
            true,
        )
    })?;
    let inventory_page = &latest.0.outbound_inventory;
    if !matches!(inventory_page.status, PageStatus::Ready | PageStatus::Empty) {
        return Err(status_failure(&inventory_page.status, "outbound inventory"));
    }
    let inventory = inventory_page.data.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Internal,
            "outbound inventory has no observation",
            true,
        )
    })?;
    if inventory.scope() != &scope {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "outbound inventory belongs to a different core scope",
            true,
        ));
    }
    let default_order = match inventory.inventory() {
        OutboundInventoryState::Ready { collections, .. } => collections
            .iter()
            .map(|collection| collection.id.clone())
            .collect(),
        OutboundInventoryState::Empty => Vec::new(),
        OutboundInventoryState::Unknown => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "inventory is unknown",
                true,
            ));
        }
        OutboundInventoryState::Unsupported => {
            return Err(Failure::new(
                ErrorCode::Unsupported,
                "outbound inventory is unsupported",
                false,
            ));
        }
        OutboundInventoryState::Failed { failure } => return Err(failure.clone()),
    };
    let preferences = outbound_ui_preferences(&latest.0).ok_or_else(|| {
        status_failure(
            &latest.0.outbound_ui_preferences.status,
            "outbound UI preferences",
        )
    })?;
    let mut current = default_order.clone();
    if !preferences.group_order.is_empty() {
        let available: HashSet<_> = default_order.iter().collect();
        current = preferences
            .group_order
            .iter()
            .filter(|id| available.contains(id))
            .cloned()
            .collect();
        for id in &default_order {
            if !current.contains(id) {
                current.push(id.clone());
            }
        }
    }
    Ok((current, default_order, inventory.revision(), scope))
}

fn status_failure(status: &PageStatus, page: &str) -> Failure {
    match status {
        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => failure.clone(),
        PageStatus::Loading => {
            Failure::new(ErrorCode::NotReady, format!("{page} is refreshing"), true)
        }
        PageStatus::Ready | PageStatus::Empty => Failure::new(
            ErrorCode::Internal,
            format!("{page} has no accepted observation"),
            true,
        ),
    }
}

#[derive(SystemParam)]
pub struct OrderControls<'w, 's> {
    up: Query<'w, 's, &'static ProxyGroupMoveUpButton>,
    down: Query<'w, 's, &'static ProxyGroupMoveDownButton>,
    reset: Query<'w, 's, (), With<ResetProxyGroupOrderButton>>,
    actions: Query<'w, 's, &'static GroupOrderAction>,
}
pub fn activate_order(
    event: On<Activate>,
    controls: OrderControls,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<GroupOrderState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    let external = if let Ok(button) = controls.up.get(event.entity) {
        button
            .collection_id
            .clone()
            .map(|group_id| GroupOrderAction::Move {
                group_id,
                direction: GroupMove::Up,
            })
    } else if let Ok(button) = controls.down.get(event.entity) {
        button
            .collection_id
            .clone()
            .map(|group_id| GroupOrderAction::Move {
                group_id,
                direction: GroupMove::Down,
            })
    } else if controls.reset.contains(event.entity) {
        Some(GroupOrderAction::Reset)
    } else {
        None
    };
    if external.is_none() && controls.actions.get(event.entity).is_err() {
        return;
    }
    if external.is_some() && state.dialog != GroupOrderDialogState::Editing {
        match observed_group_order(&latest) {
            Ok((current, default_order, revision, _)) => {
                match state.editor.open(current, default_order, revision) {
                    Ok(()) => state.dialog = GroupOrderDialogState::Editing,
                    Err(failure) => state.editor.reject(failure),
                }
            }
            Err(failure) => state.editor.reject(failure),
        }
    }
    let Some(action) = external.or_else(|| controls.actions.get(event.entity).ok().cloned()) else {
        return;
    };
    if state.dialog != GroupOrderDialogState::Editing {
        return;
    }
    match action {
        GroupOrderAction::Move {
            group_id,
            direction,
        } => {
            state.editor.move_group(&group_id, direction);
        }
        GroupOrderAction::Reset => state.editor.reset_draft(),
        GroupOrderAction::Cancel => {
            if !state.editor.is_pending() {
                state.editor = ProxyGroupOrderEditor::default();
                state.request = GroupOrderRequestState::Idle;
                state.dialog = GroupOrderDialogState::Closed;
            }
        }
        GroupOrderAction::Apply => {
            let pending = match state.editor.begin() {
                Ok(pending) => pending,
                Err(_) => return,
            };
            let Some(preferences) = outbound_ui_preferences(&latest.0) else {
                state.editor.finish(
                    pending.token,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "outbound preferences are not currently observed",
                        true,
                    )),
                );
                return;
            };
            let intent = if pending.operation == GroupOrderOperation::ResetToInventory {
                OutboundUiPreferenceIntent::ResetGroupOrder {
                    inventory_revision: pending.inventory_revision,
                }
            } else {
                OutboundUiPreferenceIntent::ReorderGroups {
                    group_ids: pending.groups.clone(),
                    inventory_revision: pending.inventory_revision,
                }
            };
            let request = match OutboundUiPreferenceRequest::new(preferences.revision(), intent) {
                Ok(request) if request.validate_scope(preferences.scope()).is_ok() => request,
                Ok(_) => {
                    state.editor.finish(
                        pending.token,
                        Err(Failure::new(
                            ErrorCode::InvalidInput,
                            "group order contains an outbound from another core",
                            false,
                        )),
                    );
                    return;
                }
                Err(error) => {
                    state.editor.finish(
                        pending.token,
                        Err(Failure::new(
                            ErrorCode::InvalidInput,
                            error.to_string(),
                            false,
                        )),
                    );
                    return;
                }
            };
            if let Some(request_id) = sink.and_then(|sink| {
                sink.submit_tracked(UiCommand::ApplyOutboundUiPreference {
                    request: request.clone(),
                })
            }) {
                state.request = GroupOrderRequestState::Submitted {
                    request_id,
                    request,
                };
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
    let GroupOrderRequestState::Submitted {
        request_id,
        request,
    } = state.request.clone()
    else {
        return;
    };
    if event.request_id != request_id
        || !matches!(
            &event.command,
            UiCommand::ApplyOutboundUiPreference { request: submitted }
                if submitted == &request
        )
    {
        return;
    }
    let result = match &event.result {
        Err(failure) => Err(failure.clone()),
        Ok(CommandOutput::OutboundUiPreferencesApplied(observed))
            if request.is_confirmed_by(observed) =>
        {
            Ok(())
        }
        Ok(_) => Err(Failure::new(
            ErrorCode::InvalidState,
            "group order returned an unrelated preference observation",
            false,
        )),
    };
    let Some(token) = state.editor.pending().map(|pending| pending.token) else {
        state.request = GroupOrderRequestState::Idle;
        return;
    };
    match state.editor.finish(token, result) {
        GroupOrderFinish::Applied => {
            state.request = GroupOrderRequestState::Idle;
            state.dialog = GroupOrderDialogState::Closed;
        }
        GroupOrderFinish::Failed | GroupOrderFinish::Stale => {
            state.request = GroupOrderRequestState::Idle;
        }
    }
}

pub fn sync_order_editor(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    route: Res<ActiveRoute>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<GroupOrderState>,
) {
    if state.dialog != GroupOrderDialogState::Editing {
        return;
    }
    if route.0 != Some(Route::Proxies) {
        if !state.editor.is_pending() {
            state.editor = ProxyGroupOrderEditor::default();
            state.request = GroupOrderRequestState::Idle;
        }
        state.dialog = GroupOrderDialogState::Closed;
        return;
    }
    if keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)) && !state.editor.is_pending() {
        state.editor = ProxyGroupOrderEditor::default();
        state.request = GroupOrderRequestState::Idle;
        state.dialog = GroupOrderDialogState::Closed;
        return;
    }
    state.editor.observe(
        observed_group_order(&latest)
            .map(|(current, _, inventory_revision, _)| (current, inventory_revision)),
    );
}

pub fn reconcile_order_rows(
    mut commands: Commands,
    state: Res<GroupOrderState>,
    palette: Res<UiPalette>,
    latest: Res<LatestSurfaceSnapshot>,
    roots: Query<Entity, With<GroupOrderList>>,
    rows: Query<(Entity, &GroupOrderRow)>,
) {
    if state.dialog != GroupOrderDialogState::Editing {
        return;
    }
    let Ok(root) = roots.single() else { return };
    let labels: HashMap<_, _> = latest
        .0
        .outbound_inventory
        .data
        .as_ref()
        .and_then(|inventory| match inventory.inventory() {
            OutboundInventoryState::Ready { collections, .. } => Some(
                collections
                    .iter()
                    .map(|collection| (collection.id.clone(), collection.id.value().to_owned()))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    let existing: HashSet<_> = rows.iter().map(|(_, row)| row.id.clone()).collect();
    for id in state.editor.draft() {
        if !existing.contains(id) {
            let label = labels
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.value().to_owned());
            commands
                .spawn_scene(row_scene(id.clone(), label, &palette))
                .insert(ChildOf(root));
        }
    }
    for (entity, row) in &rows {
        if !state.editor.draft().contains(&row.id) {
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
        .draft()
        .iter()
        .enumerate()
        .map(|(rank, id)| (id, rank))
        .collect();
    for mut children in &mut roots {
        let sorted = children.iter().enumerate().all(|(rank, entity)| {
            rows.get(*entity)
                .ok()
                .and_then(|row| ranks.get(&row.id).copied())
                == Some(rank)
        });
        if !sorted {
            children.sort_by_key(|entity| {
                rows.get(*entity)
                    .ok()
                    .and_then(|row| ranks.get(&row.id).copied())
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
        let display = if state.dialog == GroupOrderDialogState::Editing {
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
            GroupOrderAction::Move {
                group_id,
                direction,
            } => {
                state.editor.is_pending()
                    || state
                        .editor
                        .draft()
                        .iter()
                        .position(|item| item == group_id)
                        .is_none_or(|index| match direction {
                            GroupMove::Up => index == 0,
                            GroupMove::Down => index + 1 == state.editor.draft().len(),
                        })
            }
            _ => state.editor.is_pending(),
        };
        if button.0 != disabled {
            button.0 = disabled;
        }
    }
    let message = state
        .editor
        .failure()
        .or_else(|| state.editor.observation_failure())
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if state.editor.is_pending() {
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
