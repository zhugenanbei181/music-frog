//! TEA adapter for the typed group-order draft and correlated command result.
use crate::state::AppState;
use crate::state::runtime::ProxyGroupOrderDialogState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::proxy_group_order_editor::{GroupMove, GroupOrderOperation};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::outbound::inventory::OutboundInventoryState;
use infiltrator_contract::outbound::preferences::OutboundUiPreferenceIntent;
use infiltrator_contract::surface_snapshot::PageStatus;
use std::collections::HashSet;

impl AppState {
    pub(crate) fn open_group_order(
        &mut self,
        movement: Option<(OutboundId, GroupMove)>,
    ) -> Task<Message> {
        if self.runtime.group_order_dialog != ProxyGroupOrderDialogState::Editing {
            if !matches!(
                &self.runtime.outbound_inventory.status,
                PageStatus::Ready | PageStatus::Empty
            ) {
                self.runtime.group_order_editor.failure = Some(page_failure(
                    &self.runtime.outbound_inventory.status,
                    "outbound inventory",
                ));
                return Task::none();
            }
            let Some(inventory) = self.runtime.outbound_inventory.data.as_ref() else {
                self.runtime.group_order_editor.failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "outbound inventory is not observed",
                    true,
                ));
                return Task::none();
            };
            if !matches!(
                &self.runtime.outbound_ui_preferences.status,
                PageStatus::Ready | PageStatus::Empty
            ) {
                self.runtime.group_order_editor.failure = Some(page_failure(
                    &self.runtime.outbound_ui_preferences.status,
                    "outbound preferences",
                ));
                return Task::none();
            }
            let default_order = match inventory.inventory() {
                OutboundInventoryState::Ready { collections, .. } => collections
                    .iter()
                    .map(|collection| collection.id.clone())
                    .collect::<Vec<_>>(),
                OutboundInventoryState::Empty => Vec::new(),
                OutboundInventoryState::Unknown => {
                    self.runtime.group_order_editor.failure = Some(Failure::new(
                        ErrorCode::NotReady,
                        "outbound inventory is unknown",
                        true,
                    ));
                    return Task::none();
                }
                OutboundInventoryState::Unsupported => {
                    self.runtime.group_order_editor.failure = Some(Failure::new(
                        ErrorCode::Unsupported,
                        "outbound inventory is unsupported",
                        false,
                    ));
                    return Task::none();
                }
                OutboundInventoryState::Failed { failure } => {
                    self.runtime.group_order_editor.failure = Some(failure.clone());
                    return Task::none();
                }
            };
            let Some(preferences) = self
                .runtime
                .outbound_ui_preferences
                .data
                .as_ref()
                .filter(|preferences| preferences.scope() == inventory.scope())
            else {
                self.runtime.group_order_editor.failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "outbound preferences do not match the accepted inventory scope",
                    true,
                ));
                return Task::none();
            };
            let current = effective_group_order(&default_order, &preferences.group_order);
            if let Err(failure) =
                self.runtime
                    .group_order_editor
                    .open(current, default_order, inventory.revision())
            {
                self.runtime.group_order_editor.failure = Some(failure);
                return Task::none();
            }
            self.runtime.group_order_dialog = ProxyGroupOrderDialogState::Editing;
        }
        self.observe_group_order();
        if let Some((id, direction)) = movement {
            self.runtime.group_order_editor.move_group(&id, direction);
        }
        Task::none()
    }

    pub(crate) fn observe_group_order(&mut self) {
        if self.runtime.group_order_dialog != ProxyGroupOrderDialogState::Editing {
            return;
        }
        if !matches!(
            &self.runtime.outbound_inventory.status,
            PageStatus::Ready | PageStatus::Empty
        ) {
            let failure = page_failure(
                &self.runtime.outbound_inventory.status,
                "outbound inventory",
            );
            self.runtime.group_order_editor.observe(Err(failure));
            return;
        }
        let Some(inventory) = self.runtime.outbound_inventory.data.as_ref() else {
            self.runtime.group_order_editor.observe(Err(Failure::new(
                ErrorCode::NotReady,
                "outbound inventory is not observed",
                true,
            )));
            return;
        };
        if !matches!(
            &self.runtime.outbound_ui_preferences.status,
            PageStatus::Ready | PageStatus::Empty
        ) {
            let failure = page_failure(
                &self.runtime.outbound_ui_preferences.status,
                "outbound preferences",
            );
            self.runtime.group_order_editor.observe(Err(failure));
            return;
        }
        let Some(preferences) = self
            .runtime
            .outbound_ui_preferences
            .data
            .as_ref()
            .filter(|preferences| preferences.scope() == inventory.scope())
        else {
            self.runtime.group_order_editor.observe(Err(Failure::new(
                ErrorCode::NotReady,
                "outbound preferences are not observed for the inventory scope",
                true,
            )));
            return;
        };
        let default_order = match inventory.inventory() {
            OutboundInventoryState::Ready { collections, .. } => collections
                .iter()
                .map(|collection| collection.id.clone())
                .collect::<Vec<_>>(),
            OutboundInventoryState::Empty => Vec::new(),
            OutboundInventoryState::Unknown
            | OutboundInventoryState::Unsupported
            | OutboundInventoryState::Failed { .. } => {
                self.runtime.group_order_editor.observe(Err(Failure::new(
                    ErrorCode::NotReady,
                    "outbound inventory is no longer available",
                    true,
                )));
                return;
            }
        };
        let current = effective_group_order(&default_order, &preferences.group_order);
        self.runtime
            .group_order_editor
            .observe(Ok((current, inventory.revision())));
    }

    pub(crate) fn apply_group_order(&mut self) -> Task<Message> {
        if !self.is_product_command_ingress() {
            if self.shell.demo {
                let Some(inventory_revision) = self.runtime.group_order_editor.inventory_revision
                else {
                    self.runtime.group_order_editor.failure = Some(Failure::new(
                        ErrorCode::NotReady,
                        "group order has no accepted inventory revision",
                        true,
                    ));
                    return Task::none();
                };
                let intent = if self.runtime.group_order_editor.operation
                    == GroupOrderOperation::ResetToInventory
                {
                    OutboundUiPreferenceIntent::ResetGroupOrder { inventory_revision }
                } else {
                    OutboundUiPreferenceIntent::ReorderGroups {
                        group_ids: self.runtime.group_order_editor.draft.clone(),
                        inventory_revision,
                    }
                };
                if !self.apply_demo_outbound_ui_preference(intent) {
                    self.runtime.group_order_editor.failure = Some(Failure::new(
                        ErrorCode::NotReady,
                        "demo outbound preferences are unavailable",
                        true,
                    ));
                    return Task::none();
                }
                self.runtime.group_order_dialog = ProxyGroupOrderDialogState::Closed;
                return Task::none();
            }
            self.runtime.group_order_editor.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "group order command service is not composed",
                true,
            ));
            return Task::none();
        }
        let pending = match self.runtime.group_order_editor.begin() {
            Ok(pending) => pending,
            Err(failure) => {
                self.runtime.group_order_editor.failure = Some(failure);
                return Task::none();
            }
        };
        let Some(scope) = self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope())
        else {
            let failure = Failure::new(
                ErrorCode::NotReady,
                "group order has no accepted core scope",
                true,
            );
            self.runtime
                .group_order_editor
                .finish(pending.token, Err(failure.clone()));
            return Task::none();
        };
        let intent = if pending.operation == GroupOrderOperation::ResetToInventory {
            OutboundUiPreferenceIntent::ResetGroupOrder {
                inventory_revision: pending.inventory_revision,
            }
        } else {
            OutboundUiPreferenceIntent::ReorderGroups {
                group_ids: pending.groups,
                inventory_revision: pending.inventory_revision,
            }
        };
        self.submit_outbound_ui_preference(scope, intent)
    }
}

fn effective_group_order(
    inventory_order: &[OutboundId],
    preference_order: &[OutboundId],
) -> Vec<OutboundId> {
    if preference_order.is_empty() {
        return inventory_order.to_vec();
    }
    let available: HashSet<_> = inventory_order.iter().collect();
    let mut effective = preference_order
        .iter()
        .filter(|id| available.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    for id in inventory_order {
        if !effective.contains(id) {
            effective.push(id.clone());
        }
    }
    effective
}

fn page_failure(status: &PageStatus, name: &str) -> Failure {
    match status {
        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => failure.clone(),
        PageStatus::Loading => {
            Failure::new(ErrorCode::NotReady, format!("{name} is refreshing"), true)
        }
        PageStatus::Ready | PageStatus::Empty => Failure::new(
            ErrorCode::Internal,
            format!("{name} has no accepted observation"),
            false,
        ),
    }
}
