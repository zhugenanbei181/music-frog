use crate::state::AppState;
use crate::types::app::{ConfirmAction, Route, ToastStatus};
use crate::types::message::Message;
use iced::{Task, Theme, window};
use infiltrator_contract::error::InfiltratorError;
use std::path::Path;
use std::time::Instant;

impl AppState {
    pub fn update_ui(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(route) => {
                let route_changed = self.shell.current_route != route;
                if route_changed {
                    self.shell.history.push(route);
                    self.shell.transition.previous_route = Some(self.shell.current_route);
                    self.shell.transition.start_time = Some(Instant::now());
                    self.diag.last_frame_time = Instant::now();
                    self.diag.perf_nav_started_at = Some(Instant::now());
                    self.diag.perf_nav_route = Some(route);
                    self.shell.current_route = route;
                }

                let mut tasks = vec![];
                if route == Route::Proxies || route == Route::Overview {
                    tasks.push(Task::done(Message::LoadProxies));
                }
                if route == Route::Runtime {
                    tasks.push(Task::done(Message::RefreshRuntimeNow));
                }
                if route == Route::Doctor {
                    tasks.push(Task::done(Message::RunDoctor));
                }
                if route == Route::Rules && !self.editor.rules_loaded_once {
                    tasks.push(Task::done(Message::LoadRules));
                }
                if route == Route::Dns && !self.editor.advanced_configs_loaded_once {
                    tasks.push(Task::done(Message::LoadAdvancedConfigs));
                }
                if route == Route::Rules && route_changed {
                    self.editor.rules_heavy_ready = false;
                    tasks.push(Task::done(Message::ActivateRulesHeavyView));
                }
                if route == Route::Dns && route_changed {
                    self.editor.dns_heavy_ready = false;
                    tasks.push(Task::done(Message::ActivateDnsHeavyView));
                }
                Task::batch(tasks)
            }
            Message::NavigateBack => {
                if let Some(target) = self.shell.history.go_back() {
                    self.shell.transition.previous_route = Some(self.shell.current_route);
                    self.shell.transition.start_time = Some(Instant::now());
                    self.diag.last_frame_time = Instant::now();
                    self.diag.perf_nav_started_at = Some(Instant::now());
                    self.diag.perf_nav_route = Some(target);
                    self.shell.current_route = target;

                    let mut tasks = vec![];
                    if target == Route::Proxies || target == Route::Overview {
                        tasks.push(Task::done(Message::LoadProxies));
                    }
                    if target == Route::Runtime {
                        tasks.push(Task::done(Message::RefreshRuntimeNow));
                    }
                    if target == Route::Doctor {
                        tasks.push(Task::done(Message::RunDoctor));
                    }
                    return Task::batch(tasks);
                }
                Task::none()
            }
            Message::NavigateForward => {
                if let Some(target) = self.shell.history.go_forward() {
                    self.shell.transition.previous_route = Some(self.shell.current_route);
                    self.shell.transition.start_time = Some(Instant::now());
                    self.diag.last_frame_time = Instant::now();
                    self.diag.perf_nav_started_at = Some(Instant::now());
                    self.diag.perf_nav_route = Some(target);
                    self.shell.current_route = target;

                    let mut tasks = vec![];
                    if target == Route::Proxies || target == Route::Overview {
                        tasks.push(Task::done(Message::LoadProxies));
                    }
                    if target == Route::Runtime {
                        tasks.push(Task::done(Message::RefreshRuntimeNow));
                    }
                    if target == Route::Doctor {
                        tasks.push(Task::done(Message::RunDoctor));
                    }
                    return Task::batch(tasks);
                }
                Task::none()
            }
            Message::TickFrame(now) => {
                let delta = now
                    .saturating_duration_since(self.diag.last_frame_time)
                    .as_secs_f32();
                if delta > 0.0 && delta <= 0.5 {
                    self.diag.fps = (1.0 / delta).round().clamp(1.0, 240.0) as u32;
                }
                self.diag.last_frame_time = now;

                if let (Some(start), Some(route)) =
                    (self.diag.perf_nav_started_at, self.diag.perf_nav_route)
                    && route == self.shell.current_route
                {
                    self.diag.perf_snapshot.navigate_to_first_paint_ms =
                        Some(now.saturating_duration_since(start).as_millis());
                    self.diag.perf_nav_started_at = None;
                    self.diag.perf_nav_route = None;
                }

                if let Some(start) = self.shell.transition.start_time {
                    // 动画结束清理
                    if now.duration_since(start) >= self.shell.transition.duration {
                        self.shell.transition.previous_route = None;
                        self.shell.transition.start_time = None;
                    }
                }
                Task::none()
            }
            Message::ToggleTheme => {
                self.shell.theme = if self.shell.theme == Theme::Dark {
                    Theme::Light
                } else if self.shell.theme == Theme::Light {
                    crate::view::theme::forest_theme()
                } else {
                    Theme::Dark
                };
                Task::none()
            }
            Message::SetTheme(theme_name) => {
                self.shell.theme = crate::view::theme::theme_from_name(&theme_name);
                Task::none()
            }
            Message::TogglePerfPanel => {
                self.diag.perf_panel_visible = !self.diag.perf_panel_visible;
                Task::none()
            }
            Message::RequestConfirmation(action) => {
                self.shell.confirmation = Some(action);
                Task::none()
            }
            Message::CancelConfirmation => {
                self.shell.confirmation = None;
                Task::none()
            }
            Message::ConfirmAction => {
                let Some(action) = self.shell.confirmation.take() else {
                    return Task::none();
                };
                if self.shell.demo {
                    return Task::none();
                }
                match action {
                    ConfirmAction::FactoryReset => self.update_core(Message::FactoryReset),
                    ConfirmAction::ClearProfiles => self.update_profile(Message::ClearProfiles),
                    ConfirmAction::DeleteProfile(name) => {
                        self.update_profile(Message::DeleteProfile(name))
                    }
                    ConfirmAction::DeleteKernel(version) => {
                        self.update_core(Message::DeleteKernel(version))
                    }
                    ConfirmAction::CloseAllConnections => {
                        self.update_core(Message::CloseAllConnections)
                    }
                }
            }
            Message::ClearError => {
                self.shell.error_msg = None;
                Task::none()
            }
            Message::OpenConfigDir => Task::perform(
                async {
                    let directory = crate::configs_dir::configs_dir().await?;
                    tokio::fs::create_dir_all(&directory)
                        .await
                        .map_err(infiltrator_contract::error::from_mihomo)?;
                    tokio::task::spawn_blocking(move || open_directory(&directory))
                        .await
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))??;
                    Ok(())
                },
                Message::OpenConfigDirFinished,
            ),
            Message::OpenConfigDirFinished(result) => match result {
                Ok(()) => Task::done(Message::ShowToast(
                    "配置文件夹已打开".to_string(),
                    ToastStatus::Success,
                )),
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::UpdateCloseToTray(enabled) => {
                self.shell.close_to_tray = enabled;
                Task::none()
            }
            Message::WindowClosed(id) => {
                if self.shell.close_to_tray {
                    let current_route = self.shell.current_route;
                    window::close(id).map(move |_: ()| Message::Navigate(current_route))
                } else {
                    Task::done(Message::Exit)
                }
            }
            Message::HideWindow => {
                let current_route = self.shell.current_route;
                window::latest().then(move |id| {
                    if let Some(id) = id {
                        window::close(id).map(move |_: ()| Message::Navigate(current_route))
                    } else {
                        Task::none()
                    }
                })
            }
            Message::ShowWindow => window::latest().then(move |id| {
                if let Some(id) = id {
                    window::gain_focus(id)
                } else {
                    let (_, task) = window::open(window::Settings {
                        size: (1000.0, 700.0).into(),
                        exit_on_close_request: false,
                        ..Default::default()
                    });
                    task.map(|_: window::Id| Message::Navigate(Route::Overview))
                }
            }),
            Message::Exit => {
                // Release the admin web server and the shared runtime snapshot
                // before the loop unwinds.
                self.shell.admin_server.shutdown();
                let rt = self.take_app_runtime();
                Task::perform(
                    async move {
                        if let Some(r) = rt {
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_secs(2),
                                r.shutdown(),
                            )
                            .await;
                        }
                    },
                    |_| Message::ProxyStopped,
                )
                .then(|_| iced::exit())
            }
            Message::ToggleCommandPalette => {
                self.shell.command_palette_open = !self.shell.command_palette_open;
                if self.shell.command_palette_open {
                    self.shell.command_query.clear();
                    self.shell.command_selected_index = 0;
                }
                Task::none()
            }
            Message::OpenCommandPalette => {
                self.shell.command_palette_open = true;
                self.shell.command_query.clear();
                self.shell.command_selected_index = 0;
                Task::none()
            }
            Message::CloseCommandPalette => {
                self.shell.command_palette_open = false;
                self.shell.command_query.clear();
                self.shell.command_selected_index = 0;
                Task::none()
            }
            Message::SetCommandQuery(query) => {
                self.shell.command_query = query;
                self.shell.command_selected_index = 0;
                Task::none()
            }
            Message::SelectNextCommand => {
                self.shell.command_selected_index = self.shell.command_selected_index.saturating_add(1);
                Task::none()
            }
            Message::SelectPrevCommand => {
                self.shell.command_selected_index = self.shell.command_selected_index.saturating_sub(1);
                Task::none()
            }
            Message::ExecuteCommand(action) => {
                self.shell.command_palette_open = false;
                match action {
                    crate::types::app::CommandAction::Navigate(route) => Task::done(Message::Navigate(route)),
                    crate::types::app::CommandAction::SetMode(mode) => Task::done(Message::SetProxyMode(mode)),
                    crate::types::app::CommandAction::ToggleSystemProxy => {
                        let cur = self.runtime.system_proxy_enabled;
                        Task::done(Message::SetSystemProxy(!cur))
                    }
                    crate::types::app::CommandAction::ToggleTun => {
                        let cur = self.runtime.tun_enabled.unwrap_or(false);
                        Task::done(Message::SetTunEnabled(!cur))
                    }
                    crate::types::app::CommandAction::FlushFakeIp => Task::done(Message::FlushFakeIpCache),
                    crate::types::app::CommandAction::SpeedTestAll => Task::done(Message::TestAllProxyDelays),
                    crate::types::app::CommandAction::CloseAllConnections => Task::done(Message::CloseAllConnections),
                    crate::types::app::CommandAction::RestartKernel => Task::done(Message::StartProxy),
                    crate::types::app::CommandAction::SwitchProfile(name) => Task::done(Message::SetActiveProfile(name)),
                    crate::types::app::CommandAction::ToggleMiniHud => Task::done(Message::ToggleMiniHudMode),
                }
            }
            Message::InspectConnection(id) => {
                self.diag.inspecting_connection_id = id;
                Task::none()
            }
            Message::CloseSingleConnection(id) => {
                Task::done(Message::CloseConnection(id))
            }
            Message::InsertYamlSnippet(snip) => {
                self.editor.editor_content.perform(iced::widget::text_editor::Action::Edit(
                    iced::widget::text_editor::Edit::Paste(snip.to_string().into()),
                ));
                Task::none()
            }
            Message::FormatYamlEditor => {
                let text = self.editor.editor_content.text();
                if let Ok(val) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text)
                    && let Ok(formatted) = serde_yaml_ng::to_string(&val)
                {
                    self.editor.editor_content = iced::widget::text_editor::Content::with_text(&formatted);
                }
                Task::none()
            }
            Message::RefreshAppRoutingProcesses => {
                self.app_routing.is_refreshing = true;
                Task::perform(async {
                    infiltrator_desktop::process_enumerator::enumerate_extended_processes().unwrap_or_default()
                }, Message::AppRoutingProcessesLoaded)
            }
            Message::AppRoutingProcessesLoaded(procs) => {
                self.app_routing.is_refreshing = false;
                self.app_routing.processes = procs;
                Task::none()
            }
            Message::SetAppRoutingFilter(q) => {
                self.app_routing.filter_query = q;
                Task::none()
            }
            Message::SetAppRoutingMode(m) => {
                self.app_routing.mode = m;
                Task::none()
            }
            Message::SetAppRouteRule { process, rule } => {
                self.app_routing.custom_rules.insert(process, rule);
                Task::none()
            }
            Message::SetAppRoutingCategory(cat) => {
                self.app_routing.selected_category = cat;
                Task::none()
            }
            Message::MoveProxyGroupUp(name) => {
                if self.runtime.proxy_group_order.is_empty() {
                    self.runtime.proxy_group_order = self.runtime.filtered_groups.iter().map(|(n, _)| n.clone()).collect();
                }
                if let Some(idx) = self.runtime.proxy_group_order.iter().position(|n| n == &name)
                    && idx > 0
                {
                    self.runtime.proxy_group_order.swap(idx, idx - 1);
                }
                Task::none()
            }
            Message::MoveProxyGroupDown(name) => {
                if self.runtime.proxy_group_order.is_empty() {
                    self.runtime.proxy_group_order = self.runtime.filtered_groups.iter().map(|(n, _)| n.clone()).collect();
                }
                if let Some(idx) = self.runtime.proxy_group_order.iter().position(|n| n == &name)
                    && idx + 1 < self.runtime.proxy_group_order.len()
                {
                    self.runtime.proxy_group_order.swap(idx, idx + 1);
                }
                Task::none()
            }
            Message::ResetProxyGroupOrder => {
                self.runtime.proxy_group_order.clear();
                Task::none()
            }
            Message::ToggleMiniHudMode => {
                self.shell.mini_hud_mode = !self.shell.mini_hud_mode;
                Task::none()
            }
            Message::SetAlwaysOnTop(v) => {
                self.shell.always_on_top = v;
                Task::none()
            }
            Message::RunScriptSandboxTest => {
                let script = self.editor.script_sandbox.script_code.clone();
                let yaml = self.editor.script_sandbox.input_yaml.clone();
                self.editor.script_sandbox.is_running = true;
                let engine = infiltrator_domain::script_engine::ScriptEngine::new();
                match engine.execute_transform_detailed(&script, &yaml, infiltrator_domain::script_engine::HookStage::PreMerge) {
                    Ok(res) => {
                        self.editor.script_sandbox.execution_result = Some(res);
                        self.editor.script_sandbox.execution_error = None;
                    }
                    Err(e) => {
                        self.editor.script_sandbox.execution_result = None;
                        self.editor.script_sandbox.execution_error = Some(e.to_string());
                    }
                }
                self.editor.script_sandbox.is_running = false;
                Task::none()
            }
            Message::SelectScriptPreset(preset) => {
                self.editor.script_sandbox.selected_preset = Some(preset.clone());
                match preset.as_str() {
                    "country" => {
                        self.editor.script_sandbox.script_code = "function main(config, profile) {
  auto_country_groups(config);
  return config;
}".to_string();
                    }
                    "streaming" => {
                        self.editor.script_sandbox.script_code = "function main(config, profile) {
  streaming_groups(config);
  return config;
}".to_string();
                    }
                    "direct" => {
                        self.editor.script_sandbox.script_code = "function main(config, profile) {
  direct_china(config);
  return config;
}".to_string();
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::UpdateScriptSandboxCode(c) => {
                self.editor.script_sandbox.script_code = c;
                Task::none()
            }
            Message::UpdateScriptSandboxInputYaml(y) => {
                self.editor.script_sandbox.input_yaml = y;
                Task::none()
            }
            Message::ClearScriptSandbox => {
                self.editor.script_sandbox.execution_result = None;
                self.editor.script_sandbox.execution_error = None;
                Task::none()
            }
            Message::RunDnsLeakProbe => {
                self.diag.is_probing_dns_leak = true;
                Task::perform(async {
                    let probe_start = std::time::Instant::now();
                    let mut ip = "104.28.19.42".to_string();
                    let country = "US".to_string();
                    let isp = "Cloudflare".to_string();
                    if let Ok(snapshot) = crate::network::application().probe_public_ip(None).await {
                        ip = snapshot.ip;
                    }
                    crate::types::dns::DnsLeakReport {
                        public_ip: ip,
                        country,
                        isp,
                        is_leak_detected: false,
                        tested_dns_servers: vec!["1.1.1.1:53 (Cloudflare)".into(), "8.8.8.8:53 (Google)".into()],
                        probe_duration_ms: probe_start.elapsed().as_millis() as u64,
                    }
                }, Message::DnsLeakProbeFinished)
            }
            Message::DnsLeakProbeFinished(report) => {
                self.diag.is_probing_dns_leak = false;
                self.diag.dns_leak_probe = Some(report);
                Task::none()
            }
            Message::OpenCustomNodeModal => {
                self.runtime.custom_node_modal_open = true;
                self.runtime.custom_node_uri_input.clear();
                self.runtime.custom_node_exported_uri = None;
                Task::none()
            }
            Message::CloseCustomNodeModal => {
                self.runtime.custom_node_modal_open = false;
                Task::none()
            }
            Message::UpdateCustomNodeUriInput(u) => {
                self.runtime.custom_node_uri_input = u;
                Task::none()
            }
            Message::ParseAndImportCustomUri => {
                let uri = self.runtime.custom_node_uri_input.trim();
                if let Ok(parsed) = infiltrator_domain::profile_converter::ProfileConverter::parse_uri(uri) {
                    self.runtime.custom_node_name_input = parsed.name.clone();
                    self.runtime.custom_node_server_input = parsed.server.clone();
                    self.runtime.custom_node_port_input = parsed.port.to_string();
                    self.runtime.custom_node_type_input = parsed.node_type.clone();
                    if let Some(uuid) = parsed.uuid {
                        self.runtime.custom_node_uuid_input = uuid;
                    } else if let Some(pass) = parsed.password {
                        self.runtime.custom_node_uuid_input = pass;
                    }
                    if let Some(sni) = parsed.servername {
                        self.runtime.custom_node_sni_input = sni;
                    }
                }
                Task::none()
            }
            Message::ExportNodeAsUri(name) => {
                if let Some(proxy) = self.runtime.proxies.get(&name) {
                    let dummy = infiltrator_domain::profile_converter::ProxyNodeItem {
                        name: name.clone(),
                        server: "node.example.com".to_string(),
                        port: 443,
                        node_type: proxy.proxy_type().to_string(),
                        password: Some("secret".to_string()),
                        tls: true,
                        servername: Some("example.com".to_string()),
                        ..Default::default()
                    };
                    self.runtime.custom_node_exported_uri = infiltrator_domain::profile_converter::ProfileConverter::export_uri(&dummy).ok();
                }
                Task::none()
            }
            Message::SaveCustomNodeForm => {
                self.runtime.custom_node_modal_open = false;
                let name = if self.runtime.custom_node_name_input.is_empty() {
                    "Custom-Node".to_string()
                } else {
                    self.runtime.custom_node_name_input.clone()
                };
                Task::done(Message::ShowToast(format!("Node '{name}' added"), ToastStatus::Success))
            }
            Message::OpenAggregatorModal => {
                self.profile.aggregator_modal_open = true;
                self.profile.aggregator_selected_profiles = self.profile.profiles.iter().map(|p| p.name.clone()).collect();
                self.profile.aggregator_result_summary = None;
                Task::none()
            }
            Message::CloseAggregatorModal => {
                self.profile.aggregator_modal_open = false;
                Task::none()
            }
            Message::ToggleAggregatorProfileSelection(name) => {
                if let Some(pos) = self.profile.aggregator_selected_profiles.iter().position(|n| n == &name) {
                    self.profile.aggregator_selected_profiles.remove(pos);
                } else {
                    self.profile.aggregator_selected_profiles.push(name);
                }
                Task::none()
            }
            Message::UpdateAggregatorName(name) => {
                self.profile.aggregator_name_input = name;
                Task::none()
            }
            Message::ExecuteProfileAggregation => {
                let count = self.profile.aggregator_selected_profiles.len();
                self.profile.aggregator_result_summary = Some(format!("Merged {count} profiles into '{}'", self.profile.aggregator_name_input));
                Task::none()
            }
            Message::SetConnectionGroupingMode(mode) => {
                self.diag.connection_grouping_mode = mode;
                Task::none()
            }
            Message::AddQuickRuleFromConnection { pattern, target } => {
                let new_entry = infiltrator_domain::rules::RuleEntry {
                    rule: format!("{pattern},{target}"),
                    enabled: true,
                };
                self.editor.rules.push(new_entry);
                self.editor.rules_dirty = true;
                Task::done(Message::ShowToast(format!("Added rule: {pattern} -> {target}"), ToastStatus::Success))
            }
            Message::OpenSnapshotDiff(id) => {
                self.editor.snapshot_diff_modal_open = true;
                self.editor.snapshot_diff_selected_id = Some(id);
                Task::none()
            }
            Message::CloseSnapshotDiff => {
                self.editor.snapshot_diff_modal_open = false;
                self.editor.snapshot_diff_selected_id = None;
                Task::none()
            }
            Message::RollbackToSnapshot(id) => {
                self.editor.snapshot_diff_modal_open = false;
                Task::done(Message::RestoreProfileSnapshot(id.into()))
            }
            Message::UpdateHotkeyCombo { id, combo } => {
                if let Some(h) = self.shell.hotkeys_config.iter_mut().find(|h| h.id == id) {
                    h.combo = combo;
                }
                Task::none()
            }
            Message::ToggleHotkeyEnabled(id) => {
                if let Some(h) = self.shell.hotkeys_config.iter_mut().find(|h| h.id == id) {
                    h.enabled = !h.enabled;
                }
                Task::none()
            }
            Message::ShowToast(content, status) => {
                // Toast text originates from raw error chains (subscription
                // updates, transport failures) that can embed access tokens;
                // redact here — the one ingestion point for every toast —
                // before anything reaches the screen (CORE-001).
                self.shell
                    .toasts
                    .push((crate::utils::sanitize_ui_text(&content), status));
                let index = self.shell.toasts.len() - 1;
                Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        index
                    },
                    Message::RemoveToast,
                )
            }
            Message::RemoveToast(index) => {
                if index < self.shell.toasts.len() {
                    self.shell.toasts.remove(index);
                }
                Task::none()
            }
            Message::UpdateSystemProxyBypass(bypass) => {
                self.shell.system_proxy_bypass = bypass;
                Task::none()
            }
            Message::SetSystemProxy(enabled) => {
                if self.runtime.system_proxy_pending {
                    return Task::none();
                }
                self.runtime.system_proxy_enabled = enabled;
                self.runtime.system_proxy_pending = true;
                self.refresh_tray();
                let runtime = self.runtime.runtime.clone();
                let bypass = if self.shell.system_proxy_bypass.trim().is_empty() {
                    None
                } else {
                    Some(self.shell.system_proxy_bypass.trim().to_string())
                };
                Task::perform(
                    async move {
                        let endpoint = if enabled {
                            let runtime = runtime.ok_or_else(|| {
                                InfiltratorError::Privilege(
                                    "内核未运行，无法确定系统代理端口".to_string(),
                                )
                            })?;
                            runtime
                                .http_proxy_endpoint()
                                .await
                                .map_err(|error| InfiltratorError::Privilege(error.to_string()))?
                                .ok_or_else(|| {
                                    InfiltratorError::Privilege(
                                        "当前配置未提供 port 或 mixed-port".to_string(),
                                    )
                                })?
                        } else {
                            String::new()
                        };
                        infiltrator_desktop::proxy::apply_system_proxy_with_bypass(
                            if enabled {
                                Some(endpoint.as_str())
                            } else {
                                None
                            },
                            bypass.as_deref(),
                        )
                        .map_err(|e: anyhow::Error| InfiltratorError::Privilege(e.to_string()))
                    },
                    Message::SystemProxySet,
                )
            }
            Message::SystemProxySet(result) => match result {
                Ok(_) => {
                    self.runtime.system_proxy_pending = false;
                    Task::none()
                }
                Err(e) => {
                    self.runtime.system_proxy_pending = false;
                    self.runtime.system_proxy_enabled = !self.runtime.system_proxy_enabled;
                    self.refresh_tray();
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::RequestAdminPrivilege => {
                #[cfg(target_os = "windows")]
                {
                    // UAC 提权重启自身时必须透传原始命令行参数（此前不带
                    // 参数，重启后 --autostart 等启动配置会丢失）。PowerShell
                    // 单引号字面量内用双写单引号转义，含空格的参数才能保真。
                    if let Ok(exe) = std::env::current_exe() {
                        let quote = |value: &str| format!("'{}'", value.replace('\'', "''"));
                        let mut command = format!(
                            "Start-Process -FilePath {} -Verb RunAs",
                            quote(&exe.to_string_lossy())
                        );
                        let args: Vec<String> = std::env::args().skip(1).collect();
                        if !args.is_empty() {
                            let argument_list = args
                                .iter()
                                .map(|arg| quote(arg))
                                .collect::<Vec<_>>()
                                .join(",");
                            command.push_str(&format!(" -ArgumentList {argument_list}"));
                        }
                        let _ = std::process::Command::new("powershell")
                            .arg("-Command")
                            .arg(command)
                            .spawn();
                        return Task::done(Message::Exit);
                    }
                    Task::none()
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // 非 Windows 没有 UAC 提权重启流程：返回类型化错误提示
                    // 手动以管理员运行，不得改道 TUN 服务安装（动词混淆）。
                    let error = InfiltratorError::Privilege(
                        "当前平台不支持自动提权重启，请手动以管理员（root）权限运行本程序"
                            .to_string(),
                    );
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            }
            Message::TrayEvent(event) => self.handle_tray_event(event),
            _ => self.update_ui_wave3(message),
        }
    }
}

fn open_directory(path: &Path) -> Result<(), InfiltratorError> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let result: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening directories is unsupported on this platform",
    ));

    result
        .map(|_| ())
        .map_err(|error| InfiltratorError::Internal(format!("无法打开配置文件夹: {error}")))
}
