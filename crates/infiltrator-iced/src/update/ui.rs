use crate::state::AppState;
use crate::types::app::{ConfirmAction, Route, ToastStatus};
use crate::types::message::Message;
use iced::{Task, Theme, window};
use infiltrator_core::error::InfiltratorError;
use std::path::Path;
use std::time::Instant;

impl AppState {
    pub fn update_ui(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(route) => {
                let route_changed = self.shell.current_route != route;
                if route_changed {
                    // 记录旧页面并启动计时器
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
                        .map_err(InfiltratorError::from)?;
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
            Message::ShowToast(content, status) => {
                // Toast text originates from raw error chains (subscription
                // updates, reqwest failures) that can embed access tokens;
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
            _ => Task::none(),
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
