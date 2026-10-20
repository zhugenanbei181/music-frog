//! Outbound inventory and typed selection/probe actions, plus the runtime
//! panel's outbound preferences and connection filter/sort controls.

use super::profile_apply::runtime_access;
use crate::settings_store::update;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::update::core::profile_apply::save_task;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::speedtest::SpeedtestConcurrency;
use infiltrator_domain::profile_converter::{ProfileConverter, ProfileFormat, ProxyNodeItem};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::preferences::OutboundGroupExpansion;
use lurker_core_contract::outbound::{
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionState,
        SelectionAvailability,
    },
    preferences::{
        OutboundFavoriteState, OutboundHealthFilter, OutboundLayout, OutboundUiPreferenceIntent,
    },
    probe::OutboundSelectionRequest,
};
use lurker_core_contract::page::PageData;

impl AppState {
    fn read_outbound_inventory(&self) -> Task<Message> {
        let Some(ingress) = self.command_ingress() else {
            return Task::done(Message::ShowToast(
                "Outbound inventory command is unavailable".to_owned(),
                ToastStatus::Error,
            ));
        };
        let Some(requested_scope) = ingress.observed_scope() else {
            return Task::done(Message::ShowToast(
                "Wait for an accepted Core scope before refreshing outbound inventory".to_owned(),
                ToastStatus::Info,
            ));
        };
        Task::perform(
            async move {
                let outcome = ingress
                    .execute_outcome(CommandIntent::ReadOutboundInventory)
                    .await;
                let terminal_scope = outcome.scope;
                let result = outcome
                    .result
                    .into_output()
                    .and_then(|output| match output {
                        CommandOutput::OutboundInventory(snapshot) => Ok(snapshot),
                        _ => Err(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound inventory command returned another output type",
                            false,
                        )),
                    });
                (terminal_scope, result)
            },
            move |(terminal_scope, result)| Message::OutboundInventoryReadFinished {
                requested_scope: Some(requested_scope),
                terminal_scope,
                result,
            },
        )
    }

    fn finish_outbound_inventory_read(
        &mut self,
        requested_scope: Option<CoreScope>,
        terminal_scope: Option<CoreScope>,
        result: Result<OutboundInventorySnapshot, Failure>,
    ) -> Task<Message> {
        if !self.is_product_command_ingress() {
            return Task::none();
        }
        let Some(accepted_scope) = self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope())
        else {
            return Task::none();
        };
        if requested_scope.as_ref() != Some(&accepted_scope)
            || terminal_scope.as_ref() != Some(&accepted_scope)
        {
            return Task::none();
        }
        match result {
            Ok(snapshot) if snapshot.scope() == &accepted_scope => {
                let current_revision = self
                    .runtime
                    .outbound_inventory
                    .data
                    .as_ref()
                    .filter(|current| current.scope() == &accepted_scope)
                    .map(|current| current.revision());
                if current_revision.is_some_and(|revision| snapshot.revision() < revision) {
                    return Task::none();
                }
                self.apply_outbound_inventory_observation(PageData::ready(snapshot));
                Task::none()
            }
            Ok(_) => Task::none(),
            Err(failure) => {
                self.apply_outbound_inventory_observation(PageData::failed(failure));
                Task::none()
            }
        }
    }

    fn select_outbound(
        &mut self,
        scope: CoreScope,
        request: OutboundSelectionRequest,
    ) -> Task<Message> {
        if self.shell.demo && !self.is_product_command_ingress() {
            if !self.outbound_selection_request_is_current(&scope, &request) {
                return Task::none();
            }
            let Some(current) = self.runtime.outbound_inventory.data.as_ref() else {
                return Task::none();
            };
            let OutboundInventoryState::Ready {
                entries,
                collections,
            } = current.inventory()
            else {
                return Task::none();
            };
            let mut collections = collections.clone();
            let Some(collection) = collections
                .iter_mut()
                .find(|collection| collection.id == *request.collection_id())
            else {
                return Task::none();
            };
            if matches!(
                &collection.selection,
                OutboundSelectionState::Ready { selected }
                    if selected == request.outbound_id()
            ) {
                return Task::none();
            }
            collection.selection = OutboundSelectionState::Ready {
                selected: request.outbound_id().clone(),
            };
            let Ok(snapshot) = OutboundInventorySnapshot::new(
                scope,
                SnapshotRevision::new(request.expected_revision().value().wrapping_add(1)),
                OutboundInventoryState::Ready {
                    entries: entries.clone(),
                    collections,
                },
            ) else {
                return Task::none();
            };
            self.apply_outbound_inventory_observation(PageData::ready(snapshot));
            return Task::none();
        }
        if !self.is_product_command_ingress() {
            return Task::none();
        }
        let Some(ingress) = self.command_ingress() else {
            return Task::none();
        };
        if ingress.observed_scope().as_ref() != Some(&scope) {
            return Task::none();
        }
        let Some(snapshot) = self.runtime.outbound_inventory.data.as_ref() else {
            return Task::none();
        };
        if snapshot.scope() != &scope || snapshot.revision() != request.expected_revision() {
            return Task::none();
        }
        let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
            return Task::none();
        };
        let Some(collection) = collections
            .iter()
            .find(|collection| collection.id == *request.collection_id())
        else {
            return Task::none();
        };
        if collection.availability != SelectionAvailability::Selectable
            || !collection.member_ids.contains(request.outbound_id())
            || matches!(
                &collection.selection,
                OutboundSelectionState::Ready { selected }
                    if selected == request.outbound_id()
            )
        {
            return Task::none();
        }

        let expected_revision = request.expected_revision();
        let collection_id = request.collection_id().clone();
        let outbound_id = request.outbound_id().clone();
        Task::perform(
            async move {
                let outcome = ingress
                    .execute_outcome(CommandIntent::SelectOutbound {
                        request: request.clone(),
                    })
                    .await;
                let terminal_scope = outcome.scope;
                let result = outcome
                    .result
                    .into_output()
                    .and_then(|output| match output {
                        CommandOutput::OutboundInventory(snapshot) => Ok(snapshot),
                        _ => Err(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound selection returned another output type",
                            false,
                        )),
                    });
                (terminal_scope, result)
            },
            move |(terminal_scope, result)| Message::OutboundSelectionFinished {
                scope,
                collection_id,
                outbound_id,
                expected_revision,
                terminal_scope,
                result,
            },
        )
    }

    fn finish_outbound_selection(
        &mut self,
        scope: CoreScope,
        collection_id: OutboundId,
        outbound_id: OutboundId,
        expected_revision: SnapshotRevision,
        terminal_scope: Option<CoreScope>,
        result: Result<OutboundInventorySnapshot, Failure>,
    ) -> Task<Message> {
        let current_scope = self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope());
        if self.is_product_command_ingress() && current_scope.as_ref() != Some(&scope) {
            return Task::none();
        }
        if terminal_scope.as_ref() != Some(&scope) {
            return Task::none();
        }
        let request =
            match OutboundSelectionRequest::new(collection_id, outbound_id, expected_revision) {
                Ok(request) => request,
                Err(error) => {
                    return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
                }
            };
        match result {
            Ok(snapshot)
                if snapshot.scope() == &scope
                    && snapshot.revision() > expected_revision
                    && request.is_confirmed_by(&snapshot) =>
            {
                if self
                    .runtime
                    .outbound_inventory
                    .data
                    .as_ref()
                    .filter(|current| current.scope() == &scope)
                    .is_some_and(|current| current.revision() > snapshot.revision())
                {
                    return Task::none();
                }
                self.apply_outbound_inventory_observation(PageData::ready(snapshot));
                Task::none()
            }
            Ok(_) => Task::done(Message::ShowToast(
                "Outbound selection readback did not confirm the requested member".to_owned(),
                ToastStatus::Error,
            )),
            Err(failure) => Task::done(Message::ShowToast(failure.message, ToastStatus::Error)),
        }
    }

    /// Apply a signed step to the typed concurrency range, clamped to 1..=30.
    pub(crate) fn stepped_speedtest_concurrency(current: usize, delta: i32) -> usize {
        (current as i32 + delta).clamp(1, 30) as usize
    }

    pub(super) fn persist_runtime_panel_settings_task(&self) -> Task<Message> {
        let auto_refresh = self.runtime.runtime_auto_refresh;
        let delay_sort_order = self.runtime.runtime_panel_delay_sort_order;
        let connection_filter = self.runtime.runtime_connection_filter.clone();
        let connection_sort = self.diag.connections_model.sort_key().as_str().to_string();
        Task::perform(
            async move {
                update(|settings| {
                    settings.runtime_panel.auto_refresh = auto_refresh;
                    settings.runtime_panel.delay_sort_order = delay_sort_order;
                    settings.runtime_panel.connection_filter = connection_filter;
                    settings.runtime_panel.connection_sort = connection_sort;
                })
                .await
            },
            Message::RuntimePanelSettingsSaved,
        )
    }

    pub fn recompute_filtered_groups(&mut self) {
        if !self.is_product_command_ingress() && !self.shell.demo {
            return;
        }
        self.refresh_outbound_inventory_projection();
    }

    /// Proxy list/selection, delay testing and runtime panel preferences.
    /// Unmatched messages fall through to the next domain in the
    /// `update_core` chain.
    pub(super) fn update_core_proxies(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshOutboundInventory => {
                if self.is_product_command_ingress() {
                    self.read_outbound_inventory()
                } else {
                    Task::none()
                }
            }
            Message::SelectOutbound { scope, request } => self.select_outbound(scope, request),
            Message::PrepareRuntimeOutboundSelection { scope, request } => {
                if !self.outbound_selection_request_is_current(&scope, &request) {
                    return Task::none();
                }
                self.runtime.runtime_selected_group = request.collection_id().value().to_owned();
                self.runtime.runtime_selected_proxy = request.outbound_id().value().to_owned();
                self.runtime.runtime_outbound_selection = Some((scope, request));
                Task::none()
            }
            Message::OutboundInventoryReadFinished {
                requested_scope,
                terminal_scope,
                result,
            } => self.finish_outbound_inventory_read(requested_scope, terminal_scope, result),
            Message::OutboundSelectionFinished {
                scope,
                collection_id,
                outbound_id,
                expected_revision,
                terminal_scope,
                result,
            } => self.finish_outbound_selection(
                scope,
                collection_id,
                outbound_id,
                expected_revision,
                terminal_scope,
                result,
            ),
            Message::OutboundProbeFinished {
                token,
                scope,
                request,
                terminal_scope,
                result,
            } => self.finish_outbound_probe(token, scope, request, terminal_scope, result),
            Message::StartOutboundProbeBatch {
                scope,
                target_scope,
                expected_inventory_revision,
            } => self.start_outbound_probe_batch(scope, target_scope, expected_inventory_revision),
            Message::OutboundProbeBatchFinished {
                token,
                scope,
                request,
                terminal_scope,
                result,
            } => self.finish_outbound_probe_batch(token, scope, request, terminal_scope, result),
            Message::DismissOutboundProbeBatch => {
                self.dismiss_outbound_probe_batch();
                Task::none()
            }
            Message::OutboundUiPreferenceFinished {
                token,
                scope,
                request,
                terminal_scope,
                result,
            } => self.finish_outbound_ui_preference(token, scope, request, terminal_scope, result),
            Message::FilterProxies(filter) => self.update_proxy_search(filter),
            Message::ToggleFilterAlive(enabled) => {
                if self.is_product_command_ingress() {
                    return self.submit_current_outbound_ui_preference(
                        OutboundUiPreferenceIntent::SetHealthFilter(if enabled {
                            OutboundHealthFilter::HealthyOnly
                        } else {
                            OutboundHealthFilter::All
                        }),
                    );
                }
                if !self.shell.demo {
                    return Task::none();
                }
                self.apply_demo_outbound_ui_preference(
                    OutboundUiPreferenceIntent::SetHealthFilter(if enabled {
                        OutboundHealthFilter::HealthyOnly
                    } else {
                        OutboundHealthFilter::All
                    }),
                );
                Task::none()
            }
            Message::ToggleFavoriteOutbound(outbound_id) => {
                if !self.is_product_command_ingress() && !self.shell.demo {
                    return Task::none();
                }
                let favorite = self
                    .runtime
                    .proxy_groups
                    .iter()
                    .flat_map(|group| &group.proxies)
                    .find(|member| member.id.as_ref() == Some(&outbound_id))
                    .is_some_and(|member| member.favorite);
                let intent = OutboundUiPreferenceIntent::SetFavorite {
                    outbound_id,
                    state: if favorite {
                        OutboundFavoriteState::Unpinned
                    } else {
                        OutboundFavoriteState::Pinned
                    },
                };
                if self.is_product_command_ingress() {
                    self.submit_current_outbound_ui_preference(intent)
                } else {
                    self.apply_demo_outbound_ui_preference(intent);
                    Task::none()
                }
            }
            Message::ToggleOutboundGroupExpanded(group_id) => {
                if !self.is_product_command_ingress() && !self.shell.demo {
                    return Task::none();
                }
                let expanded = self
                    .runtime
                    .proxy_groups
                    .iter()
                    .find(|group| group.id.as_ref() == Some(&group_id))
                    .is_none_or(|group| !group.expanded);
                let intent = OutboundUiPreferenceIntent::SetGroupExpansion {
                    group_id,
                    expansion: if expanded {
                        OutboundGroupExpansion::Expanded
                    } else {
                        OutboundGroupExpansion::Collapsed
                    },
                };
                if self.is_product_command_ingress() {
                    self.submit_current_outbound_ui_preference(intent)
                } else {
                    self.apply_demo_outbound_ui_preference(intent);
                    Task::none()
                }
            }
            Message::ToggleProxyCompactView => {
                if self.is_product_command_ingress() {
                    return self.submit_current_outbound_ui_preference(
                        OutboundUiPreferenceIntent::SetLayout(
                            if self.runtime.proxies_model.layout() == OutboundLayout::CompactList {
                                OutboundLayout::Grid
                            } else {
                                OutboundLayout::CompactList
                            },
                        ),
                    );
                }
                if !self.shell.demo {
                    return Task::none();
                }
                self.apply_demo_outbound_ui_preference(OutboundUiPreferenceIntent::SetLayout(
                    if self.runtime.proxies_model.layout() == OutboundLayout::CompactList {
                        OutboundLayout::Grid
                    } else {
                        OutboundLayout::CompactList
                    },
                ));
                Task::none()
            }
            Message::OpenAddCustomNodeModal(open) => {
                self.runtime.is_adding_custom_node = open;
                Task::none()
            }
            Message::UpdateNewNodeType(t) => {
                self.runtime.new_node_type = t;
                Task::none()
            }
            Message::UpdateNewNodeName(n) => {
                self.runtime.new_node_name = n;
                Task::none()
            }
            Message::UpdateNewNodeServer(s) => {
                self.runtime.new_node_server = s;
                Task::none()
            }
            Message::UpdateNewNodePort(p) => {
                self.runtime.new_node_port = p;
                Task::none()
            }
            Message::UpdateNewNodeCredential(c) => {
                self.runtime.new_node_credential = c;
                Task::none()
            }
            Message::UpdateNewNodeCipher(c) => {
                self.runtime.new_node_cipher = c;
                Task::none()
            }
            Message::UpdateNewNodeTls(tls) => {
                self.runtime.new_node_tls = tls;
                Task::none()
            }
            Message::SubmitAddCustomNode => {
                let name = self.runtime.new_node_name.trim().to_string();
                let server = self.runtime.new_node_server.trim().to_string();
                let port = self
                    .runtime
                    .new_node_port
                    .trim()
                    .parse::<u16>()
                    .unwrap_or(443);
                let node_type = self.runtime.new_node_type.clone();
                let cred = self.runtime.new_node_credential.trim().to_string();
                let cipher = self.runtime.new_node_cipher.trim().to_string();
                let tls = self.runtime.new_node_tls;

                if name.is_empty() || server.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Node name and server are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                let node_item = ProxyNodeItem {
                    name: name.clone(),
                    server,
                    port,
                    node_type: node_type.clone(),
                    password: if matches!(node_type.as_str(), "ss" | "trojan" | "hysteria2")
                        && !cred.is_empty()
                    {
                        Some(cred.clone())
                    } else {
                        None
                    },
                    uuid: if matches!(node_type.as_str(), "vmess" | "vless") && !cred.is_empty() {
                        Some(cred)
                    } else {
                        None
                    },
                    cipher: if node_type == "ss" && !cipher.is_empty() {
                        Some(cipher)
                    } else {
                        None
                    },
                    tls,
                    ..Default::default()
                };

                let runtime = runtime_access(self.runtime.runtime.clone());
                save_task(
                    runtime,
                    move |content| {
                        let mut nodes =
                            ProfileConverter::parse_nodes(content, ProfileFormat::ClashYaml)
                                .unwrap_or_default();
                        nodes.insert(0, node_item);
                        ProfileConverter::export_nodes(&nodes, ProfileFormat::ClashYaml)
                    },
                    Message::CustomNodeAdded,
                )
            }
            Message::CustomNodeAdded(result) => {
                self.runtime.is_adding_custom_node = false;
                match result {
                    Ok(_) => {
                        self.runtime.new_node_name.clear();
                        self.runtime.new_node_server.clear();
                        Task::batch(vec![
                            Task::done(Message::RefreshOutboundInventory),
                            Task::done(Message::ShowToast(
                                "Proxy node added to active profile".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::InspectOutbound(outbound_id) => {
                let next = outbound_id.filter(|id| self.outbound_is_observed(id));
                if self.runtime.outbound_inspecting != next {
                    self.runtime.reset_outbound_probe();
                    self.runtime.outbound_inspecting = next;
                }
                Task::none()
            }
            Message::ProbeInspectedOutbound => self.probe_inspected_outbound(),
            Message::StartOutboundProbe { scope, request } => {
                self.start_outbound_probe(scope, request)
            }
            Message::SetOutboundSortOrder(sort_order) => {
                if self.is_product_command_ingress() {
                    return self.submit_current_outbound_ui_preference(
                        OutboundUiPreferenceIntent::SetSortOrder(sort_order),
                    );
                }
                if self.shell.demo {
                    self.apply_demo_outbound_ui_preference(
                        OutboundUiPreferenceIntent::SetSortOrder(sort_order),
                    );
                }
                Task::none()
            }
            Message::SetRuntimePanelDelaySort(sort_order) => {
                self.runtime.runtime_panel_delay_sort_order = sort_order;
                if self.shell.demo {
                    Task::none()
                } else {
                    self.persist_runtime_panel_settings_task()
                }
            }
            Message::UpdateRuntimeProbeUrlDraft(url) => {
                self.runtime.runtime_probe_settings_editor.edit_url(url);
                self.sync_probe_draft_fields();
                Task::none()
            }
            Message::UpdateRuntimeProbeTimeoutDraft(timeout) => {
                self.runtime
                    .runtime_probe_settings_editor
                    .edit_timeout(timeout);
                self.sync_probe_draft_fields();
                Task::none()
            }
            Message::UpdateRuntimeSelectedGroup(group) => {
                self.runtime.runtime_selected_group = group;
                self.runtime.runtime_outbound_selection = None;
                if self.is_product_command_ingress() || self.shell.demo {
                    self.sync_runtime_outbound_selection();
                }
                Task::none()
            }
            Message::ApplyRuntimeSelectedProxy => {
                if self.is_product_command_ingress() {
                    let Some((scope, request)) = self.runtime.runtime_outbound_selection.clone()
                    else {
                        return Task::none();
                    };
                    if !self.outbound_selection_request_is_current(&scope, &request) {
                        self.runtime.runtime_outbound_selection = None;
                        self.sync_runtime_outbound_selection();
                        return Task::none();
                    }
                    return Task::done(Message::SelectOutbound { scope, request });
                }
                if self.shell.demo {
                    let Some((scope, request)) = self.runtime.runtime_outbound_selection.clone()
                    else {
                        return Task::none();
                    };
                    if !self.outbound_selection_request_is_current(&scope, &request) {
                        self.runtime.runtime_outbound_selection = None;
                        self.sync_runtime_outbound_selection();
                        return Task::none();
                    }
                    return Task::done(Message::SelectOutbound { scope, request });
                }
                let group = self.runtime.runtime_selected_group.trim().to_string();
                let proxy = self.runtime.runtime_selected_proxy.trim().to_string();
                if group.is_empty() || proxy.is_empty() {
                    return Task::none();
                }
                self.outbound_selection_message(&group, &proxy)
                    .map(Task::done)
                    .unwrap_or_else(|| {
                        Task::done(Message::ShowToast(
                            "Outbound selection is unavailable for the current observation"
                                .to_owned(),
                            ToastStatus::Info,
                        ))
                    })
            }
            Message::UpdateRuntimeConnectionFilter(filter) => {
                self.diag.connections_model.edit_query(
                    filter
                        .trim()
                        .strip_prefix("tab:closed")
                        .unwrap_or(&filter)
                        .trim(),
                );
                self.runtime.runtime_connection_filter = filter;
                self.diag.connections_page = 0;
                self.persist_runtime_panel_settings_task()
            }
            Message::UpdateRuntimeConnectionSort(sort_key) => {
                self.diag.connections_model.select_sort(sort_key);
                self.diag.connections_page = 0;
                self.persist_runtime_panel_settings_task()
            }
            Message::RuntimePanelSettingsSaved(_) => Task::none(),
            Message::UpdateFilteredGroups => {
                self.recompute_filtered_groups();
                Task::none()
            }
            Message::CancelSpeedtestRun(run_id) => {
                let Some(ingress) = self.command_ingress() else {
                    return Task::done(Message::ShowToast(
                        "Speedtest is not configured for this Core owner".to_string(),
                        ToastStatus::Error,
                    ));
                };
                let Some(requested_scope) = ingress.observed_scope() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let outcome = ingress
                            .execute_outcome(CommandIntent::CancelSpeedtestRun { run_id })
                            .await;
                        if outcome.scope.as_ref() != Some(&requested_scope) {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "speedtest cancellation completed for a stale Core scope",
                                false,
                            ));
                        }
                        let cancellation =
                            outcome
                                .result
                                .into_output()
                                .and_then(|output| match output {
                                    CommandOutput::SpeedtestCancelled(outcome) => Ok(outcome),
                                    _ => Err(Failure::new(
                                        ErrorCode::InvalidState,
                                        "speedtest cancellation returned another output type",
                                        false,
                                    )),
                                })?;
                        Ok((cancellation, ingress.speedtest_snapshot()))
                    },
                    Message::SpeedtestCancelled,
                )
            }
            Message::AdjustSpeedtestConcurrency(delta) => {
                let Some(ingress) = self.command_ingress() else {
                    return Task::done(Message::ShowToast(
                        "Speedtest is not configured for this Core owner".to_string(),
                        ToastStatus::Error,
                    ));
                };
                let current = ingress.speedtest_snapshot().concurrency.get();
                let next = Self::stepped_speedtest_concurrency(usize::from(current), delta);
                let Ok(concurrency) = SpeedtestConcurrency::new(next as u8) else {
                    return Task::done(Message::ShowToast(
                        "Speedtest concurrency is outside its supported range".to_owned(),
                        ToastStatus::Error,
                    ));
                };
                let Some(requested_scope) = ingress.observed_scope() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        let outcome = ingress
                            .execute_outcome(CommandIntent::SetSpeedtestConcurrency { concurrency })
                            .await;
                        if outcome.scope.as_ref() != Some(&requested_scope) {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "speedtest concurrency update completed for a stale Core scope",
                                false,
                            ));
                        }
                        outcome.result.into_unit()?;
                        Ok(ingress.speedtest_snapshot())
                    },
                    Message::SpeedtestConcurrencySet,
                )
            }
            other => self.update_core_runtime_config(other),
        }
    }
}
