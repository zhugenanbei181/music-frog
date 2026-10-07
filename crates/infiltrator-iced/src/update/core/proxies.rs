//! Proxies domain: proxy list loading and filtering, group/proxy selection,
//! delay testing and the persisted runtime-panel preferences (sort keys,
//! delay test URL/timeout, connection filter/sort).

use crate::settings_store::update;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::update::core::profile_apply::save_task;
use iced::Task;
use infiltrator_application::proxy_projection::project_groups_snapshot;
use infiltrator_application::proxy_search_projection::project_name_runs;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::proxies::ProxySortOrder;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::speedtest::SpeedtestScope;
use infiltrator_domain::profile_converter::{ProfileConverter, ProfileFormat, ProxyNodeItem};
use infiltrator_domain::proxy::Proxy;
use infiltrator_ports::error::PortError;
use std::collections::HashMap;

const DEFAULT_RUNTIME_CONNECTION_SORT: &str = "download_desc";

impl AppState {
    fn apply_proxies_loaded(
        &mut self,
        result: Result<HashMap<String, Proxy>, InfiltratorError>,
    ) -> Task<Message> {
        self.runtime.is_loading_proxies = false;
        match result {
            Ok(proxies) => {
                self.runtime.proxies = proxies;
                self.reconcile_proxy_inspection();
                self.refresh_tray();
                self.recompute_filtered_groups();
                self.sync_runtime_proxy_selection();
            }
            Err(error) => self.set_error(&error),
        }
        Task::none()
    }

    /// Drive a scope-wide speedtest through the shared engine port. The host's
    /// `SpeedtestApplication` is the single fact source both surfaces read; no
    /// UI-local metric or legacy proxy path is used.
    fn run_speedtest_scope(&mut self, scope: SpeedtestScope) -> Task<Message> {
        let Some(port) = self
            .runtime
            .runtime
            .clone()
            .and_then(|runtime| runtime.speedtest_port())
        else {
            return Task::done(Message::ShowToast(
                "Speedtest is not available on this host".to_string(),
                ToastStatus::Error,
            ));
        };
        if let Some(application) = self.commands.clone() {
            let group = match scope {
                SpeedtestScope::SingleGroup(group) => Some(group),
                SpeedtestScope::AllGroups => None,
                _ => return Task::none(),
            };
            let (url, timeout_ms) = match self.applied_probe_options() {
                Ok(options) => (Some(options.test_url), Some(options.timeout_ms)),
                Err(_) => (None, None),
            };
            self.runtime.runtime_testing_all_delays = true;
            return Task::perform(
                async move {
                    match (application
                        .execute(CommandIntent::TestDelay {
                            group,
                            url,
                            timeout_ms,
                        })
                        .await)
                        .into_unit()
                    {
                        Ok(()) => Ok(port.snapshot()),
                        Err(failure) => Err(PortError::Rejected(failure)),
                    }
                },
                Message::SpeedtestScopeUpdated,
            );
        }
        let options = match self.applied_probe_options() {
            Ok(options) => options,
            Err(failure) => {
                return Task::done(Message::ShowToast(failure.message, ToastStatus::Error));
            }
        };
        let test_url = Some(options.test_url);
        let timeout_ms = options.timeout_ms;
        self.runtime.runtime_testing_all_delays = true;
        Task::perform(
            async move { port.run_scope(scope, test_url, Some(timeout_ms)).await },
            Message::SpeedtestScopeUpdated,
        )
    }

    pub(super) fn normalize_delay_sort_key(value: &str) -> &'static str {
        match value.trim().to_ascii_lowercase().as_str() {
            "delay_asc" => "delay_asc",
            "delay_desc" => "delay_desc",
            "name_asc" => "name_asc",
            "name_desc" => "name_desc",
            _ => "delay_asc",
        }
    }

    pub(super) fn normalize_connection_sort_key(value: &str) -> &'static str {
        match value.trim().to_ascii_lowercase().as_str() {
            "download_desc" => "download_desc",
            "upload_desc" => "upload_desc",
            "latest_desc" => "latest_desc",
            "host_asc" => "host_asc",
            _ => DEFAULT_RUNTIME_CONNECTION_SORT,
        }
    }

    /// DUAL-06-03: the user-typed speedtest target, or `None` when blank so the
    /// shared engine applies its own default. The engine owns the effective
    /// target fact; the UI only carries the raw input into the port call.
    pub(crate) fn speedtest_target_url(&self) -> Option<String> {
        let trimmed = self.runtime.runtime_speedtest_url.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// DUAL-06-01: apply a signed step to a concurrency bound, clamped to the
    /// supported 1..=64 window. Pure so the handler and tests share one rule.
    pub(crate) fn stepped_speedtest_concurrency(current: usize, delta: i32) -> usize {
        (current as i32 + delta).clamp(1, 64) as usize
    }

    pub(super) fn persist_runtime_panel_settings_task(&self) -> Task<Message> {
        let auto_refresh = self.runtime.runtime_auto_refresh;
        let delay_sort = Self::normalize_delay_sort_key(&self.runtime.proxy_delay_sort).to_string();
        let connection_filter = self.runtime.runtime_connection_filter.clone();
        let connection_sort =
            Self::normalize_connection_sort_key(&self.runtime.runtime_connection_sort).to_string();
        Task::perform(
            async move {
                update(|settings| {
                    settings.runtime_panel.auto_refresh = auto_refresh;
                    settings.runtime_panel.delay_sort = delay_sort;
                    settings.runtime_panel.connection_filter = connection_filter;
                    settings.runtime_panel.connection_sort = connection_sort;
                })
                .await
            },
            Message::RuntimePanelSettingsSaved,
        )
    }

    pub fn recompute_filtered_groups(&mut self) {
        if self.commands.is_some() && !self.shell.demo {
            return;
        }
        let mut preferences = self.runtime.proxy_ui_preferences.clone();
        preferences.search_query = self.runtime.proxy_filter.clone();
        preferences.filter_alive = self.runtime.filter_alive_only;
        preferences.sort_order =
            ProxySortOrder::from_str_loose(&self.runtime.proxy_delay_sort).unwrap_or_default();
        preferences.favorite_proxies = self.runtime.favorite_proxies.iter().cloned().collect();
        let (groups, _) = project_groups_snapshot(&self.runtime.proxies, &preferences);
        self.runtime.proxy_name_runs = project_name_runs(&groups, &preferences.search_query);
        self.runtime.proxy_groups = groups;
        self.runtime.filtered_groups = self
            .runtime
            .proxy_groups
            .iter()
            .map(|group| {
                (
                    group.name.clone(),
                    group.proxies.iter().map(|node| node.name.clone()).collect(),
                )
            })
            .collect();
    }

    fn sync_runtime_proxy_selection(&mut self) {
        let mut groups: Vec<String> = self
            .runtime
            .proxies
            .iter()
            .filter_map(|(name, proxy)| {
                if proxy.is_group() {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        if groups.is_empty() {
            self.runtime.runtime_selected_group.clear();
            self.runtime.runtime_selected_proxy.clear();
            return;
        }

        groups.sort();
        if let Some(index) = groups.iter().position(|name| name == "GLOBAL") {
            let global = groups.remove(index);
            groups.insert(0, global);
        }

        if !groups
            .iter()
            .any(|name| name == &self.runtime.runtime_selected_group)
        {
            self.runtime.runtime_selected_group = groups[0].clone();
        }

        let members: Vec<String> = self
            .runtime
            .proxies
            .get(&self.runtime.runtime_selected_group)
            .and_then(|proxy| proxy.all())
            .map(|all| all.to_vec())
            .unwrap_or_default();
        if members.is_empty() {
            self.runtime.runtime_selected_proxy.clear();
            return;
        }

        if !members
            .iter()
            .any(|name| name == &self.runtime.runtime_selected_proxy)
        {
            let current = self
                .runtime
                .proxies
                .get(&self.runtime.runtime_selected_group)
                .and_then(|proxy| proxy.now())
                .map(|name| name.to_string());
            self.runtime.runtime_selected_proxy = current
                .filter(|name| members.iter().any(|member| member == name))
                .unwrap_or_else(|| members[0].clone());
        }
    }

    /// Proxy list/selection, delay testing and runtime panel preferences.
    /// Unmatched messages fall through to the next domain in the
    /// `update_core` chain.
    pub(super) fn update_core_proxies(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LoadProxies => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    let scope = self.commands.as_ref().map(|application| {
                        let snapshot = application.snapshot();
                        (snapshot.generation, snapshot.session_token)
                    });
                    self.runtime.is_loading_proxies = true;
                    Task::perform(
                        async move {
                            rt.get_proxies()
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        move |result| match scope {
                            Some((generation, session_token)) => Message::ProxiesLoadedForSession {
                                generation,
                                session_token,
                                result,
                            },
                            None => Message::ProxiesLoaded(result),
                        },
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProxiesLoadedForSession {
                generation,
                session_token,
                result,
            } => {
                let current = self
                    .commands
                    .as_ref()
                    .map(|application| application.snapshot());
                if current.is_some_and(|current| {
                    current.generation == generation
                        && current.session_token == session_token
                        && matches!(
                            current.lifecycle,
                            CoreLifecycle::Ready | CoreLifecycle::Running
                        )
                }) {
                    self.apply_proxies_loaded(result)
                } else {
                    Task::none()
                }
            }
            Message::ProxiesLoaded(result) => {
                if self.commands.is_some() {
                    Task::none()
                } else {
                    self.apply_proxies_loaded(result)
                }
            }
            Message::SelectProxy(group, name) => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.switch_proxy(&group, &name)
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        |_| Message::LoadProxies,
                    )
                } else {
                    Task::none()
                }
            }
            Message::FilterProxies(filter) => self.update_proxy_search(filter),
            Message::ToggleFilterAlive(enabled) => {
                if self.commands.is_some() {
                    return self
                        .submit_proxy_preference(CommandIntent::ToggleFilterAlive { enabled });
                }
                self.runtime.filter_alive_only = enabled;
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::ToggleFavoriteProxy(proxy) => {
                if self.commands.is_some() {
                    return self
                        .submit_proxy_preference(CommandIntent::ToggleFavoriteProxy { proxy });
                }
                if self.runtime.favorite_proxies.contains(&proxy) {
                    self.runtime.favorite_proxies.remove(&proxy);
                } else {
                    self.runtime.favorite_proxies.insert(proxy);
                }
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::ToggleProxyCompactView => {
                if self.commands.is_some() {
                    return self.submit_proxy_preference(CommandIntent::SetProxyCompactView {
                        compact: !self.runtime.proxy_compact_view,
                    });
                }
                self.runtime.proxy_compact_view = !self.runtime.proxy_compact_view;
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

                let runtime = self.runtime.runtime.clone();
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
                            Task::done(Message::LoadProxies),
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
            Message::InspectProxy(proxy) => {
                let next = proxy.filter(|name| self.proxy_inspection(name).is_some());
                if self.runtime.inspecting_proxy != next {
                    self.runtime.inspection_probe.dismiss();
                    self.runtime.inspecting_proxy = next;
                }
                self.reconcile_proxy_inspection();
                Task::none()
            }
            Message::TestInspectedProxy => self.probe_inspected_proxy(),
            Message::ProxyInspectionProbed {
                name,
                token,
                result,
            } => self.finish_inspection_probe(name, token, result),
            Message::ToggleProxySort => {
                self.runtime.proxy_sort_by_delay = !self.runtime.proxy_sort_by_delay;
                self.runtime.proxy_delay_sort = if self.runtime.proxy_sort_by_delay {
                    "delay_asc".to_string()
                } else {
                    "name_asc".to_string()
                };
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::UpdateProxyDelaySort(sort_key) => {
                if self.commands.is_some() {
                    return self.submit_proxy_preference(CommandIntent::SetProxySortOrder {
                        order: ProxySortOrder::from_str_loose(&sort_key).unwrap_or_default(),
                    });
                }
                let normalized = Self::normalize_delay_sort_key(&sort_key).to_string();
                self.runtime.proxy_delay_sort = normalized.clone();
                self.runtime.proxy_sort_by_delay = normalized.starts_with("delay_");
                Task::batch(vec![
                    Task::done(Message::UpdateFilteredGroups),
                    self.persist_runtime_panel_settings_task(),
                ])
            }
            Message::UpdateDelayTestUrl(url) => {
                self.runtime.probe_options_editor.edit_url(url);
                self.sync_probe_draft_fields();
                Task::none()
            }
            Message::UpdateDelayTimeoutMs(timeout) => {
                self.runtime.probe_options_editor.edit_timeout(timeout);
                self.sync_probe_draft_fields();
                Task::none()
            }
            Message::UpdateRuntimeSelectedGroup(group) => {
                self.runtime.runtime_selected_group = group;
                self.sync_runtime_proxy_selection();
                Task::none()
            }
            Message::UpdateRuntimeSelectedProxy(proxy) => {
                self.runtime.runtime_selected_proxy = proxy;
                Task::none()
            }
            Message::ApplyRuntimeSelectedProxy => {
                let group = self.runtime.runtime_selected_group.trim().to_string();
                let proxy = self.runtime.runtime_selected_proxy.trim().to_string();
                if group.is_empty() || proxy.is_empty() {
                    return Task::none();
                }
                Task::done(Message::SelectProxy(group, proxy))
            }
            Message::UpdateRuntimeConnectionFilter(filter) => {
                self.diag.connection_groups.edit_query(
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
                self.runtime.runtime_connection_sort =
                    Self::normalize_connection_sort_key(&sort_key).to_string();
                self.diag.connections_page = 0;
                self.persist_runtime_panel_settings_task()
            }
            Message::RuntimePanelSettingsSaved(_) => Task::none(),
            Message::UpdateFilteredGroups => {
                self.recompute_filtered_groups();
                Task::none()
            }
            Message::SpeedtestScopeUpdated(result) => {
                self.runtime.runtime_testing_all_delays = false;
                match result {
                    Ok(snapshot) => {
                        let alive = snapshot.alive_nodes_count();
                        let total = snapshot.node_count();
                        self.diag.speedtest = snapshot;
                        Task::batch(vec![
                            Task::done(Message::LoadProxies),
                            Task::done(Message::ShowToast(
                                format!("Delay test complete: {alive} alive / {total} tested"),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(error) => {
                        let e = InfiltratorError::Internal(error.to_string());
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::CancelSpeedtest => {
                let Some(port) = self
                    .runtime
                    .runtime
                    .clone()
                    .and_then(|runtime| runtime.speedtest_port())
                else {
                    return Task::done(Message::ShowToast(
                        "Speedtest is not available on this host".to_string(),
                        ToastStatus::Error,
                    ));
                };
                let was_running = port.cancel();
                self.runtime.runtime_testing_all_delays = false;
                Task::done(Message::ShowToast(
                    if was_running {
                        "Speedtest cancelled".to_string()
                    } else {
                        "No speedtest is running".to_string()
                    },
                    if was_running {
                        ToastStatus::Warning
                    } else {
                        ToastStatus::Error
                    },
                ))
            }
            Message::UpdateSpeedtestTestUrl(url) => {
                self.runtime.runtime_speedtest_url = url;
                Task::none()
            }
            Message::AdjustSpeedtestConcurrency(delta) => {
                let Some(port) = self
                    .runtime
                    .runtime
                    .clone()
                    .and_then(|runtime| runtime.speedtest_port())
                else {
                    return Task::done(Message::ShowToast(
                        "Speedtest is not available on this host".to_string(),
                        ToastStatus::Error,
                    ));
                };
                let current = port.snapshot().config.concurrency;
                let next = Self::stepped_speedtest_concurrency(current, delta);
                match port.set_concurrency(next) {
                    Ok(()) => {
                        // Read the live bound back from the one shared snapshot.
                        self.diag.speedtest = port.snapshot();
                        Task::none()
                    }
                    Err(error) => Task::done(Message::ShowToast(
                        format!("Concurrency update failed: {error}"),
                        ToastStatus::Error,
                    )),
                }
            }
            Message::TestProxyDelay(name) => {
                if !self.runtime.runtime_testing_delay_proxy.is_empty() {
                    return Task::none();
                }
                if self.commands.is_some() {
                    return self.probe_proxy_by_name(name);
                }
                if let Some(rt) = self.runtime.runtime.clone() {
                    let n = name.clone();
                    let options = match self.applied_probe_options() {
                        Ok(options) => options,
                        Err(failure) => {
                            return Task::done(Message::ShowToast(
                                failure.message,
                                ToastStatus::Error,
                            ));
                        }
                    };
                    let test_url = options.test_url;
                    let timeout_ms = options.timeout_ms;
                    self.runtime.runtime_testing_delay_proxy = name.clone();
                    Task::perform(
                        async move {
                            rt.test_delay(&n, &test_url, timeout_ms)
                                .await
                                .map(|d| d as u64)
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        move |res| Message::ProxyTested(name, res),
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProxyDelayCompleted { name, result } => {
                if self.runtime.runtime_testing_delay_proxy != name {
                    return Task::none();
                }
                self.runtime.runtime_testing_delay_proxy.clear();
                match result {
                    Ok(()) => Task::done(Message::LoadProxies),
                    Err(failure) => {
                        Task::done(Message::ShowToast(failure.message, ToastStatus::Error))
                    }
                }
            }
            Message::ProxyTested(name, result) => {
                self.runtime.runtime_testing_delay_proxy.clear();
                match result {
                    Ok(delay) => Task::batch(vec![
                        Task::done(Message::LoadProxies),
                        Task::done(Message::ShowToast(
                            format!("{}: {}ms", name, delay),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => Task::done(Message::ShowToast(
                        format!("{}: {}", name, e),
                        ToastStatus::Error,
                    )),
                }
            }
            Message::TestGroupDelay(name) => {
                self.run_speedtest_scope(SpeedtestScope::SingleGroup(name))
            }
            Message::TestAllProxyDelays => {
                if self.runtime.runtime_testing_all_delays {
                    return Task::none();
                }
                self.run_speedtest_scope(SpeedtestScope::AllGroups)
            }
            other => self.update_core_runtime_config(other),
        }
    }
}
