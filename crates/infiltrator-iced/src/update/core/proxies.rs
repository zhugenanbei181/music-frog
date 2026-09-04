//! Proxies domain: proxy list loading and filtering, group/proxy selection,
//! delay testing and the persisted runtime-panel preferences (sort keys,
//! delay test URL/timeout, connection filter/sort).

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_core::error::InfiltratorError;
use infiltrator_core::settings::{AppSettings, RuntimePanelConfig};

pub(super) const DEFAULT_RUNTIME_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
pub(super) const DEFAULT_RUNTIME_DELAY_TIMEOUT_MS: u32 = 5000;
pub(super) const MIN_RUNTIME_DELAY_TIMEOUT_MS: u32 = 100;
pub(super) const MAX_RUNTIME_DELAY_TIMEOUT_MS: u32 = 60_000;
const DEFAULT_RUNTIME_CONNECTION_SORT: &str = "download_desc";

impl AppState {
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

    fn delay_sortable_value(&self, name: &str) -> Option<u32> {
        self.runtime
            .proxies
            .get(name)
            .and_then(|proxy| proxy.history().last().map(|item| item.delay))
            .filter(|delay| *delay > 0)
    }

    fn compare_delay_members(&self, left: &str, right: &str) -> std::cmp::Ordering {
        let left_fav = self.runtime.favorite_proxies.contains(left);
        let right_fav = self.runtime.favorite_proxies.contains(right);
        if left_fav != right_fav {
            return if left_fav {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        let left_delay = self.delay_sortable_value(left);
        let right_delay = self.delay_sortable_value(right);

        let compare_delay = |desc: bool| match (left_delay, right_delay) {
            (None, None) => left.cmp(right),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(lv), Some(rv)) => {
                let base = if desc { rv.cmp(&lv) } else { lv.cmp(&rv) };
                if base == std::cmp::Ordering::Equal {
                    left.cmp(right)
                } else {
                    base
                }
            }
        };

        match self.runtime.proxy_delay_sort.as_str() {
            "name_asc" => left.cmp(right),
            "name_desc" => right.cmp(left),
            "delay_desc" => compare_delay(true),
            _ => compare_delay(false),
        }
    }

    fn normalized_delay_test_url(&self) -> String {
        let trimmed = self.runtime.runtime_delay_test_url.trim();
        if trimmed.is_empty() {
            DEFAULT_RUNTIME_DELAY_TEST_URL.to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn normalized_delay_timeout_ms(&self) -> u32 {
        self.runtime
            .runtime_delay_timeout_ms
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value >= MIN_RUNTIME_DELAY_TIMEOUT_MS)
            .unwrap_or(DEFAULT_RUNTIME_DELAY_TIMEOUT_MS)
            .min(MAX_RUNTIME_DELAY_TIMEOUT_MS)
    }

    pub(super) fn persist_runtime_panel_settings_task(&self) -> Task<Message> {
        let runtime_panel = RuntimePanelConfig {
            auto_refresh: self.runtime.runtime_auto_refresh,
            delay_sort: Self::normalize_delay_sort_key(&self.runtime.proxy_delay_sort).to_string(),
            delay_test_url: self.normalized_delay_test_url(),
            delay_timeout_ms: self.normalized_delay_timeout_ms(),
            connection_filter: self.runtime.runtime_connection_filter.clone(),
            connection_sort: Self::normalize_connection_sort_key(
                &self.runtime.runtime_connection_sort,
            )
            .to_string(),
        };

        Task::perform(
            async move {
                let base_dir =
                    mihomo_platform::paths::get_home_dir().map_err(InfiltratorError::from)?;
                let settings_path = infiltrator_core::settings::settings_path(&base_dir)
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                let mut settings = infiltrator_core::settings::load_settings(&settings_path)
                    .await
                    .unwrap_or_else(|_| AppSettings::default());
                settings.runtime_panel = runtime_panel;
                infiltrator_core::settings::save_settings(&settings_path, &settings)
                    .await
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                Ok(())
            },
            Message::RuntimePanelSettingsSaved,
        )
    }

    pub fn recompute_filtered_groups(&mut self) {
        let mut groups: Vec<_> = self
            .runtime
            .proxies
            .iter()
            .filter(|(_, p)| p.is_group())
            .collect();

        // Sort groups: GLOBAL first, then by type
        groups.sort_by(|(na, pa), (nb, pb)| {
            if *na == "GLOBAL" {
                return std::cmp::Ordering::Less;
            }
            if *nb == "GLOBAL" {
                return std::cmp::Ordering::Greater;
            }
            pa.proxy_type().cmp(pb.proxy_type())
        });

        let mut result = Vec::new();
        for (group_name, group_info) in groups {
            let mut members: Vec<String> =
                group_info.all().map(|all| all.to_vec()).unwrap_or_default();

            // 1. Filter with smart pinyin and ISO region matching (T01-07)
            if !self.runtime.proxy_filter.is_empty() {
                members.retain(|m| {
                    infiltrator_shared::fuzzy_search::pinyin_fuzzy_match(
                        m,
                        &self.runtime.proxy_filter,
                    )
                });
            }

            if self.runtime.filter_alive_only {
                members.retain(|m| self.delay_sortable_value(m).is_some());
            }

            if members.is_empty()
                && (!self.runtime.proxy_filter.is_empty() || self.runtime.filter_alive_only)
            {
                continue;
            }

            members.sort_by(|left, right| self.compare_delay_members(left, right));

            result.push((group_name.clone(), members));
        }
        self.runtime.filtered_groups = result;
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
                    self.runtime.is_loading_proxies = true;
                    Task::perform(
                        async move {
                            rt.client()
                                .get_proxies()
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::ProxiesLoaded,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProxiesLoaded(result) => {
                self.runtime.is_loading_proxies = false;
                match result {
                    Ok(proxies) => {
                        self.runtime.proxies = proxies;
                        self.refresh_tray();
                        self.recompute_filtered_groups();
                        self.sync_runtime_proxy_selection();
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::SelectProxy(group, name) => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .switch_proxy(&group, &name)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        |_| Message::LoadProxies,
                    )
                } else {
                    Task::none()
                }
            }
            Message::FilterProxies(filter) => {
                self.runtime.proxy_filter = filter;
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::ToggleFilterAlive(enabled) => {
                self.runtime.filter_alive_only = enabled;
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::ToggleFavoriteProxy(proxy) => {
                if self.runtime.favorite_proxies.contains(&proxy) {
                    self.runtime.favorite_proxies.remove(&proxy);
                } else {
                    self.runtime.favorite_proxies.insert(proxy);
                }
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::ToggleProxyCompactView => {
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

                let node_item = infiltrator_domain::profile_converter::ProxyNodeItem {
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
                crate::update::core::profile_apply::save_task(
                    runtime,
                    move |content| {
                        let mut nodes =
                            infiltrator_domain::profile_converter::ProfileConverter::parse_nodes(
                                content,
                                infiltrator_domain::profile_converter::ProfileFormat::ClashYaml,
                            )
                            .unwrap_or_default();
                        nodes.insert(0, node_item);
                        infiltrator_domain::profile_converter::ProfileConverter::export_nodes(
                            &nodes,
                            infiltrator_domain::profile_converter::ProfileFormat::ClashYaml,
                        )
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
                self.runtime.inspecting_proxy = proxy;
                Task::none()
            }
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
                let normalized = Self::normalize_delay_sort_key(&sort_key).to_string();
                self.runtime.proxy_delay_sort = normalized.clone();
                self.runtime.proxy_sort_by_delay = normalized.starts_with("delay_");
                Task::batch(vec![
                    Task::done(Message::UpdateFilteredGroups),
                    self.persist_runtime_panel_settings_task(),
                ])
            }
            Message::UpdateDelayTestUrl(url) => {
                self.runtime.runtime_delay_test_url = url;
                self.persist_runtime_panel_settings_task()
            }
            Message::UpdateDelayTimeoutMs(timeout) => {
                self.runtime.runtime_delay_timeout_ms = timeout;
                self.persist_runtime_panel_settings_task()
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
            Message::AllProxyDelaysTested(result) => {
                self.runtime.runtime_testing_all_delays = false;
                match result {
                    Ok((success, failed)) => Task::batch(vec![
                        Task::done(Message::LoadProxies),
                        Task::done(Message::ShowToast(
                            format!(
                                "Delay test complete: {} success, {} failed",
                                success, failed
                            ),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::TestProxyDelay(name) => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    let n = name.clone();
                    let test_url = self.normalized_delay_test_url();
                    let timeout_ms = self.normalized_delay_timeout_ms();
                    self.runtime.runtime_testing_delay_proxy = name.clone();
                    Task::perform(
                        async move {
                            rt.client()
                                .test_delay(&n, &test_url, timeout_ms)
                                .await
                                .map(|d| d as u64)
                                .map_err(InfiltratorError::from)
                        },
                        move |res| Message::ProxyTested(name, res),
                    )
                } else {
                    Task::none()
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
                if let Some(rt) = self.runtime.runtime.clone() {
                    let proxies = self.runtime.proxies.clone();
                    let test_url = self.normalized_delay_test_url();
                    let timeout_ms = self.normalized_delay_timeout_ms();
                    self.runtime.runtime_testing_all_delays = true;
                    Task::perform(
                        async move {
                            let members = proxies
                                .get(&name)
                                .and_then(|p| p.all())
                                .map(|all| all.to_vec())
                                .unwrap_or_default();
                            let tester = infiltrator_core::flow_control::BatchDelayTester::new(
                                30,
                                test_url,
                                std::time::Duration::from_millis(timeout_ms as u64),
                            );
                            let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
                            let client_arc = std::sync::Arc::new(rt.client());
                            let outcomes = tester
                                .test_proxies(
                                    members,
                                    move |proxy, url| {
                                        let client = client_arc.clone();
                                        async move {
                                            client
                                                .test_delay(&proxy, &url, timeout_ms)
                                                .await
                                                .map(|d| d as u64)
                                                .map_err(|e| e.to_string())
                                        }
                                    },
                                    cancel_rx,
                                )
                                .await;
                            let mut success = 0usize;
                            let mut failed = 0usize;
                            for outcome in outcomes {
                                if outcome.result.is_ok() {
                                    success += 1;
                                } else {
                                    failed += 1;
                                }
                            }
                            Ok((success, failed))
                        },
                        Message::AllProxyDelaysTested,
                    )
                } else {
                    Task::none()
                }
            }
            Message::TestAllProxyDelays => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    if self.runtime.runtime_testing_all_delays
                        || !self.runtime.runtime_testing_delay_proxy.is_empty()
                    {
                        return Task::none();
                    }
                    let test_url = self.normalized_delay_test_url();
                    let timeout_ms = self.normalized_delay_timeout_ms();
                    let candidates: Vec<String> = self
                        .runtime
                        .proxies
                        .iter()
                        .filter_map(|(name, info)| {
                            if info.is_group() {
                                None
                            } else {
                                Some(name.clone())
                            }
                        })
                        .collect();
                    self.runtime.runtime_testing_all_delays = true;
                    Task::perform(
                        async move {
                            let tester = infiltrator_core::flow_control::BatchDelayTester::new(
                                30,
                                test_url,
                                std::time::Duration::from_millis(timeout_ms as u64),
                            );
                            let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
                            let client_arc = std::sync::Arc::new(rt.client());
                            let outcomes = tester
                                .test_proxies(
                                    candidates,
                                    move |proxy, url| {
                                        let client = client_arc.clone();
                                        async move {
                                            client
                                                .test_delay(&proxy, &url, timeout_ms)
                                                .await
                                                .map(|d| d as u64)
                                                .map_err(|e| e.to_string())
                                        }
                                    },
                                    cancel_rx,
                                )
                                .await;
                            let mut success = 0usize;
                            let mut failed = 0usize;
                            for outcome in outcomes {
                                if outcome.result.is_ok() {
                                    success += 1;
                                } else {
                                    failed += 1;
                                }
                            }
                            Ok((success, failed))
                        },
                        Message::AllProxyDelaysTested,
                    )
                } else {
                    Task::none()
                }
            }
            other => self.update_core_runtime_config(other),
        }
    }
}
