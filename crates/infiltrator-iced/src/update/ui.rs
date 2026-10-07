use crate::configs_dir::configs_dir;
use crate::host::process_enumerator::enumerate_extended_processes;
use crate::routing_application::application;
use crate::state::AppState;
use crate::types::app::{ConfirmAction, Route, ToastStatus};
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::types::rule_trace::RuleTraceAction;
use iced::widget::text_editor::Content;
use iced::{Task, clipboard, window};
use infiltrator_application::connection_rate_application::project_connection;
use infiltrator_application::profile_editor_projection;
use infiltrator_application::proxy_group_order_editor::GroupMove;
use infiltrator_application::rule_statistics_workbench::StatisticsAction;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_contract::error::{InfiltratorError, from_mihomo};
use infiltrator_domain::connection_rate::PULSE_BREATH_HZ;
use infiltrator_domain::connection_view::{ConnectionRuleSpec, append_draft_rule};
use infiltrator_domain::yaml_edit::format::format_yaml;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
#[cfg(target_os = "windows")]
use std::env::{args, current_exe};
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
use std::io;
use std::path::Path;
use std::time::Instant;
use std::{process, time};
use tokio::fs::create_dir_all;
use tokio::task::spawn_blocking;
use tokio::time::timeout;

impl AppState {
    pub fn update_ui(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::Navigate(route) => {
                let route_changed = self.shell.current_route != route;
                if route_changed {
                    self.editor.dns_hosts_editor.cancel();
                    self.diag.dns_cache_actions.cancel();
                    self.diag.dns_query.cancel();
                    self.shell.history.push(route);
                    self.shell.transition.previous_route = Some(self.shell.current_route);
                    self.shell.transition.start_time = Some(Instant::now());
                    self.diag.last_frame_time = Instant::now();
                    self.diag.perf_nav_started_at = Some(Instant::now());
                    self.diag.perf_nav_route = Some(route);
                    self.diag.speedtest_detail_open = false;
                    self.diag.inspecting_connection_id = None;
                    self.shell.confirmation = None;
                    if self.runtime.probe_options_editor.pending.is_none() {
                        self.runtime.probe_options_editor.cancel();
                        self.sync_probe_draft_fields();
                    }
                    self.runtime.probe_options_open = false;
                    if self.runtime.group_order_editor.pending.is_none() {
                        self.runtime.group_order_editor.cancel();
                    }
                    self.runtime.group_order_open = false;
                    self.runtime.custom_node_modal_open = false;
                    self.runtime.inspecting_proxy = None;
                    self.runtime.inspection_probe.dismiss();
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
                    self.diag.speedtest_detail_open = false;
                    self.diag.inspecting_connection_id = None;
                    self.shell.confirmation = None;
                    if self.runtime.probe_options_editor.pending.is_none() {
                        self.runtime.probe_options_editor.cancel();
                        self.sync_probe_draft_fields();
                    }
                    self.runtime.probe_options_open = false;
                    if self.runtime.group_order_editor.pending.is_none() {
                        self.runtime.group_order_editor.cancel();
                    }
                    self.runtime.group_order_open = false;
                    self.runtime.custom_node_modal_open = false;
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
                    self.diag.speedtest_detail_open = false;
                    self.diag.inspecting_connection_id = None;
                    self.shell.confirmation = None;
                    if self.runtime.probe_options_editor.pending.is_none() {
                        self.runtime.probe_options_editor.cancel();
                        self.sync_probe_draft_fields();
                    }
                    self.runtime.probe_options_open = false;
                    if self.runtime.group_order_editor.pending.is_none() {
                        self.runtime.group_order_editor.cancel();
                    }
                    self.runtime.group_order_open = false;
                    self.runtime.custom_node_modal_open = false;
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
            Message::CaptureRegionMeasured(bounds) => {
                if self.shell.demo {
                    self.shell.capture_region_bounds = bounds;
                }
                Task::none()
            }
            Message::CaptureFrameRendered {
                revision,
                bounds,
                screenshot,
            } => {
                self.finish_capture_frame(revision, bounds, screenshot);
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
                if self.runtime.traffic_topology.is_flowing() {
                    self.diag.topology_flow_phase = (self.diag.topology_flow_phase
                        + delta * self.runtime.traffic_topology.flow_speed_hz())
                    .fract();
                } else {
                    self.diag.topology_flow_phase = 0.0;
                }

                // DUAL-13-10: the high-throughput pulse breathes only while a
                // real connection stays above the shared threshold; the same
                // shared phase drives the Bevy pulse through the same domain
                // intensity function.
                if self.diag.connection_pulse_active() {
                    // The breath frequency is the shared domain constant, so
                    // both surfaces pulse at the same rate.
                    self.diag.connection_pulse_phase =
                        (self.diag.connection_pulse_phase + delta * PULSE_BREATH_HZ).fract();
                } else {
                    self.diag.connection_pulse_phase = 0.0;
                }

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
                Task::batch([self.capture_geometry_task(), self.capture_frame_task()])
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
                if self.shell.confirmation == Some(ConfirmAction::RuleStatisticsCleanup) {
                    return self.update_rule_statistics(StatisticsAction::CancelCleanup);
                }
                if matches!(
                    self.shell.confirmation,
                    Some(ConfirmAction::TracerOverride(_))
                ) && !self.editor.rule_trace.cancel_override()
                {
                    return Task::none();
                }
                self.shell.confirmation = None;
                Task::none()
            }
            Message::ConfirmAction => {
                if self.shell.confirmation == Some(ConfirmAction::RuleStatisticsCleanup) {
                    return self.update_rule_statistics(StatisticsAction::ConfirmCleanup);
                }
                if matches!(
                    self.shell.confirmation,
                    Some(ConfirmAction::TracerOverride(_))
                ) {
                    return self.update_core(Message::RuleTrace(RuleTraceAction::ConfirmOverride));
                }
                let Some(action) = self.shell.confirmation.take() else {
                    return Task::none();
                };
                if self.shell.demo {
                    return Task::none();
                }
                match action {
                    ConfirmAction::TracerOverride(_) | ConfirmAction::RuleStatisticsCleanup => {
                        Task::none()
                    }
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
                    let directory = configs_dir().await?;
                    create_dir_all(&directory).await.map_err(from_mihomo)?;
                    spawn_blocking(move || open_directory(&directory))
                        .await
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))??;
                    Ok(())
                },
                Message::OpenConfigDirFinished,
            ),
            Message::OpenConfigDirFinished(result) => match result {
                Ok(()) => Task::done(Message::ShowToast(
                    Lang(&copy_locale).tr("profile_folder_opened").into_owned(),
                    ToastStatus::Success,
                )),
                Err(error) => {
                    let message = localize(
                        &self.shell.lang,
                        "profile_folder_open_failed",
                        &[("reason", error.to_string())],
                    );
                    self.set_error(&message);
                    Task::done(Message::ShowToast(message, ToastStatus::Error))
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
                if let Some(cleanup) = &self.exit_cleanup {
                    cleanup();
                }
                let rt = self.take_app_runtime();
                Task::perform(
                    async move {
                        if let Some(r) = rt {
                            let _ = timeout(time::Duration::from_secs(2), r.shutdown()).await;
                        }
                    },
                    |_| Message::ProxyStopped,
                )
                .then(|_| iced::exit())
            }
            Message::ToggleCommandPalette => {
                self.shell.command_palette_open = !self.shell.command_palette_open;
                if self.shell.command_palette_open {
                    self.rebuild_command_catalogue();
                    self.shell.command_query.clear();
                    self.shell.command_selected_index = 0;
                }
                Task::none()
            }
            Message::OpenCommandPalette => {
                self.shell.command_palette_open = true;
                self.rebuild_command_catalogue();
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
                let len = self.filtered_command_indices().len();
                if len > 0 {
                    self.shell.command_selected_index =
                        (self.shell.command_selected_index + 1) % len;
                }
                Task::none()
            }
            Message::SelectPrevCommand => {
                let len = self.filtered_command_indices().len();
                if len > 0 {
                    self.shell.command_selected_index =
                        (self.shell.command_selected_index + len - 1) % len;
                }
                Task::none()
            }
            Message::ExecuteSelectedCommand => {
                if !self.shell.command_palette_open {
                    return Task::none();
                }
                let selected = self
                    .filtered_command_indices()
                    .get(self.shell.command_selected_index)
                    .and_then(|index| self.shell.command_catalogue.entry(*index))
                    .map(|entry| entry.target.clone());
                selected.map_or_else(Task::none, |target| {
                    self.update_ui(Message::ExecuteCommand(target))
                })
            }
            Message::ExecuteCommand(target) => {
                self.shell.command_palette_open = false;
                // The shared target vocabulary: global-chord actions re-enter
                // the single shortcut handler, everything else routes through
                // the same update arm its own message uses.
                if let Some(action) = target.shortcut_action() {
                    return self.on_shell_shortcut(action);
                }
                match target {
                    CommandTarget::Navigate(page) => {
                        self.update_ui(Message::Navigate(Route::from_shell_page(page)))
                    }
                    CommandTarget::SetProxyMode(mode) => {
                        self.update_ui(Message::SetProxyMode(mode.to_wire().to_owned()))
                    }
                    CommandTarget::SwitchProfile { name, .. } => {
                        self.update_ui(Message::SetActiveProfile(name))
                    }
                    CommandTarget::FlushDnsCache => {
                        let navigation = self.update_ui(Message::Navigate(Route::Dns));
                        let confirmation = self.update_core(Message::FlushFakeIpCache);
                        Task::batch(vec![navigation, confirmation])
                    }
                    CommandTarget::TestAllProxyGroups => {
                        self.update_ui(Message::TestAllProxyDelays)
                    }
                    CommandTarget::RunDoctor => self.update_ui(Message::RunDoctor),
                    CommandTarget::CloseAllConnections => self.update_ui(
                        Message::RequestConfirmation(ConfirmAction::CloseAllConnections),
                    ),
                    CommandTarget::RestartKernel => self.update_ui(Message::StartProxy),
                    // Handled above through the shared shortcut vocabulary.
                    CommandTarget::ToggleSystemProxy
                    | CommandTarget::ToggleTun
                    | CommandTarget::ToggleMiniHud
                    | CommandTarget::CycleTheme => Task::none(),
                }
            }
            Message::CopyConnectionHost => {
                let connection = self.diag.inspecting_connection_id.as_ref().and_then(|id| {
                    self.diag.connections.as_ref().and_then(|snapshot| {
                        snapshot
                            .connections
                            .iter()
                            .find(|connection| &connection.id == id)
                    })
                });
                connection.map_or_else(Task::none, |connection| {
                    let facts = project_connection(
                        connection,
                        self.diag.connection_rate_book.get(&connection.id),
                    );
                    clipboard::write(facts.destination_host)
                })
            }
            Message::InspectConnection(id) => {
                self.diag.inspecting_connection_id = id.filter(|id| {
                    self.diag.connections.as_ref().is_some_and(|snapshot| {
                        snapshot
                            .connections
                            .iter()
                            .any(|connection| &connection.id == id)
                    })
                });
                Task::none()
            }
            Message::CloseSingleConnection(id) => Task::done(Message::CloseConnection(id)),
            // DUAL-09-05: the shared AST-preserving formatter. A serde
            // re-serialize would drop comments and anchors, so it is not an
            // acceptable fallback here: a refusal keeps the user's bytes and
            // says why.
            Message::FormatYamlEditor => {
                if !self.editor.document_session.can_edit() {
                    return Task::none();
                }
                let text = self.editor.editor_content.text();
                match format_yaml(&text) {
                    Ok(report) => {
                        self.editor.editor_content = Content::with_text(&report.content);
                        // The replaced content restarts the widget's scroll at
                        // line 0; the shared window restarts with it (DUAL-09-02).
                        self.reset_document_viewport(EditorPane::Profile);
                        match report.skip_reason() {
                            Some(reason) if reason.is_advisory() => {
                                let message = profile_editor_projection::format_note(
                                    reason,
                                    &self.shell.lang,
                                );
                                return Task::done(Message::ShowToast(message, ToastStatus::Info));
                            }
                            _ => {}
                        }
                        Task::none()
                    }
                    Err(error) => Task::done(Message::ShowToast(
                        format!("Format refused: {error}"),
                        ToastStatus::Error,
                    )),
                }
            }
            Message::RefreshAppRoutingProcesses => {
                self.app_routing.is_refreshing = true;
                let processes = Task::perform(
                    async { enumerate_extended_processes().unwrap_or_default() },
                    Message::AppRoutingProcessesLoaded,
                );
                let config = Task::perform(
                    async {
                        application()
                            .await?
                            .load()
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AppRoutingConfigLoaded,
                );
                Task::batch(vec![processes, config])
            }
            Message::AppRoutingProcessesLoaded(procs) => {
                self.app_routing.is_refreshing = false;
                self.app_routing.processes = procs;
                Task::none()
            }
            Message::AppRoutingConfigLoaded(result) => match result {
                Ok(config) => {
                    self.app_routing.mode = config.mode;
                    self.app_routing.custom_rules = config.rules;
                    Task::none()
                }
                Err(error) => {
                    self.set_error(&error);
                    Task::none()
                }
            },
            Message::AppRoutingPersisted(result) => {
                if let Err(error) = result {
                    self.set_error(&error);
                }
                Task::none()
            }
            Message::SetAppRoutingFilter(q) => {
                self.app_routing.filter_query = q;
                Task::none()
            }
            Message::SetAppRoutingMode(m) => {
                self.app_routing.mode = m;
                let mode = m;
                Task::perform(
                    async move {
                        application()
                            .await?
                            .set_mode(mode)
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AppRoutingPersisted,
                )
            }
            Message::SetAppRouteRule { process, rule } => {
                let package = process.clone();
                self.app_routing.custom_rules.insert(process, rule);
                let domain_rule = rule;
                Task::perform(
                    async move {
                        application()
                            .await?
                            .set_rule(&package, domain_rule)
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AppRoutingPersisted,
                )
            }
            Message::SetAppRoutingCategory(cat) => {
                self.app_routing.selected_category = cat;
                Task::none()
            }
            Message::MoveProxyGroupUp(name) => self.open_group_order(Some((name, GroupMove::Up))),
            Message::MoveProxyGroupDown(name) => {
                self.open_group_order(Some((name, GroupMove::Down)))
            }
            Message::ResetProxyGroupOrder => {
                let task = self.open_group_order(None);
                self.runtime.group_order_editor.reset_draft();
                task
            }
            Message::ToggleMiniHudMode
            | Message::SetAlwaysOnTop(_)
            | Message::MiniHudMoved { .. }
            | Message::MiniHudDragReleased
            | Message::MiniHudPlacementUpdated(_)
            | Message::MiniHudDisplayKnown(_)
            | Message::WindowIdResolved(_) => {
                self.update_mini_hud(message).unwrap_or_else(Task::none)
            }
            Message::WindowChromeDragRequested
            | Message::WindowChromeToggleMaximize
            | Message::WindowChromeMinimize
            | Message::WindowChromeClose => self.update_chrome(message).unwrap_or_else(Task::none),
            Message::Script(_) => self.update_script_workbench(message),
            Message::OpenCustomNodeModal
            | Message::CloseCustomNodeModal
            | Message::UpdateCustomNodeUriInput(_)
            | Message::ParseAndImportCustomUri
            | Message::UpdateCustomNodeDraft(_)
            | Message::UpdateCustomNodeField(_, _)
            | Message::ExportCustomNodeUri
            | Message::SaveCustomNodeForm
            | Message::CustomNodeSaved(_)
            | Message::ScanCustomNodeDialer
            | Message::CustomNodeDialerScanned(_)
            | Message::VerifyCustomNodeCertificateAuthority => self.update_protocol_codec(message),
            Message::SetConnectionGroupingMode(mode) => {
                self.diag.connection_groups.select(mode);
                Task::none()
            }
            Message::AddQuickRuleFromConnection { pattern, target } => {
                // DUAL-13-09: both surfaces append through the shared domain
                // draft seam, which rejects empty patterns and de-duplicates.
                let spec = ConnectionRuleSpec {
                    pattern: pattern.clone(),
                    target: target.clone(),
                };
                if !self.editor.rule_list.editable() {
                    return Task::none();
                }
                let added = append_draft_rule(&mut self.editor.rule_list.draft, &spec);
                if added.is_none() {
                    return Task::none();
                }
                Task::done(Message::ShowToast(
                    format!("Added rule: {pattern} -> {target}"),
                    ToastStatus::Success,
                ))
            }
            Message::OpenSnapshotDiff(_)
            | Message::CloseSnapshotDiff
            | Message::SnapshotDiffLoaded(_)
            | Message::SetSnapshotDiffMode(_)
            | Message::RefreshSnapshotDiff
            | Message::RollbackToSnapshot(_)
            | Message::SetProfileProtectionOverride(_) => self.update_snapshot_diff(message),
            // Group 15 shell domain (appearance preference, global shortcut
            // registry, toast ingestion) lives in `update/shell.rs`.
            Message::ToggleTheme
            | Message::SetTheme(_)
            | Message::SystemThemeChanged(_)
            | Message::CycleThemePreference
            | Message::BeginHotkeyCapture(_)
            | Message::CancelHotkeyCapture
            | Message::KeyboardChord { .. }
            | Message::ToggleHotkeyEnabled(_)
            | Message::ResetHotkey(_)
            | Message::ShortcutsUpdated(_)
            | Message::ShowToast(_, _)
            | Message::RemoveToast(_) => self.update_shell(message).unwrap_or_else(Task::none),
            Message::UpdateSystemProxyBypass(bypass) => {
                self.shell.system_proxy_bypass = bypass;
                Task::none()
            }
            Message::SetSystemProxy(enabled) => self.set_system_proxy(enabled),
            Message::SystemProxySet(result) => self.finish_system_proxy_set(result),
            Message::SystemProxyReconciled(snapshot) => self.reconcile_system_proxy(snapshot),
            Message::SystemProxyRecoveryFinished(snapshot) => {
                self.finish_system_proxy_recovery(snapshot)
            }
            Message::RequestAdminPrivilege => {
                #[cfg(target_os = "windows")]
                {
                    // UAC 提权重启自身时必须透传原始命令行参数（此前不带
                    // 参数，重启后 --autostart 等启动配置会丢失）。PowerShell
                    // 单引号字面量内用双写单引号转义，含空格的参数才能保真。
                    if let Ok(exe) = current_exe() {
                        let quote = |value: &str| format!("'{}'", value.replace('\'', "''"));
                        let mut command = format!(
                            "Start-Process -FilePath {} -Verb RunAs",
                            quote(&exe.to_string_lossy())
                        );
                        let args: Vec<String> = args().skip(1).collect();
                        if !args.is_empty() {
                            let argument_list = args
                                .iter()
                                .map(|arg| quote(arg))
                                .collect::<Vec<_>>()
                                .join(",");
                            command.push_str(&format!(" -ArgumentList {argument_list}"));
                        }
                        let _ = process::Command::new("powershell")
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
                        Lang(&copy_locale)
                            .tr("elevation_restart_unavailable")
                            .as_ref()
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
    let result = process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let result = process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let result = process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let result: io::Result<process::Child> = Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "opening directories is unsupported on this platform",
    ));

    result
        .map(|_| ())
        .map_err(|error| InfiltratorError::Io(error.to_string()))
}
