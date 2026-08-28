use crate::state::AppState;
use crate::types::{InfiltratorError, Message, Route};
use iced::{Task, Theme, window};
use std::time::Instant;

impl AppState {
    pub fn update_ui(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(route) => {
                let route_changed = self.current_route != route;
                if route_changed {
                    // 记录旧页面并启动计时器
                    self.transition.previous_route = Some(self.current_route);
                    self.transition.start_time = Some(Instant::now());
                    self.last_frame_time = Instant::now();
                    self.perf_nav_started_at = Some(Instant::now());
                    self.perf_nav_route = Some(route);
                    self.current_route = route;
                }

                let mut tasks = vec![];
                if route == Route::Proxies || route == Route::Overview {
                    tasks.push(Task::done(Message::LoadProxies));
                }
                if route == Route::Overview || route == Route::Runtime {
                    tasks.push(Task::done(Message::FetchIpInfo));
                }
                if route == Route::Runtime {
                    tasks.push(Task::done(Message::RefreshRuntimeNow));
                }
                if route == Route::Rules && !self.rules_loaded_once {
                    tasks.push(Task::done(Message::LoadRules));
                }
                if route == Route::Dns && !self.advanced_configs_loaded_once {
                    tasks.push(Task::done(Message::LoadAdvancedConfigs));
                }
                if route == Route::Rules && route_changed {
                    self.rules_heavy_ready = false;
                    tasks.push(Task::done(Message::ActivateRulesHeavyView));
                }
                if route == Route::Dns && route_changed {
                    self.dns_heavy_ready = false;
                    tasks.push(Task::done(Message::ActivateDnsHeavyView));
                }
                Task::batch(tasks)
            }
            Message::TickFrame(now) => {
                let delta = now
                    .saturating_duration_since(self.last_frame_time)
                    .as_secs_f32();
                if delta > 0.0 && delta <= 0.5 {
                    self.fps = (1.0 / delta).round().clamp(1.0, 240.0) as u32;
                }
                self.last_frame_time = now;

                if let (Some(start), Some(route)) = (self.perf_nav_started_at, self.perf_nav_route)
                {
                    if route == self.current_route {
                        self.perf_snapshot.navigate_to_first_paint_ms =
                            Some(now.saturating_duration_since(start).as_millis());
                        self.perf_nav_started_at = None;
                        self.perf_nav_route = None;
                    }
                }

                if let Some(start) = self.transition.start_time {
                    // 动画结束清理
                    if now.duration_since(start) >= self.transition.duration {
                        self.transition.previous_route = None;
                        self.transition.start_time = None;
                    }
                }
                Task::none()
            }
            Message::ToggleTheme => {
                self.theme = if self.theme == Theme::Dark {
                    Theme::Light
                } else {
                    Theme::Dark
                };
                Task::none()
            }
            Message::TogglePerfPanel => {
                self.perf_panel_visible = !self.perf_panel_visible;
                Task::none()
            }
            Message::WindowClosed(id) => {
                let current_route = self.current_route;
                window::close(id).map(move |_: ()| Message::Navigate(current_route))
            }
            Message::HideWindow => {
                let current_route = self.current_route;
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
                let rt = self.runtime.take();
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
                self.toasts.push((content, status));
                let index = self.toasts.len() - 1;
                Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        index
                    },
                    Message::RemoveToast,
                )
            }
            Message::RemoveToast(index) => {
                if index < self.toasts.len() {
                    self.toasts.remove(index);
                }
                Task::none()
            }
            Message::SetSystemProxy(enabled) => {
                self.system_proxy_enabled = enabled;
                Task::perform(
                    async move {
                        let endpoint = if enabled {
                            Some("127.0.0.1:7890")
                        } else {
                            None
                        };
                        infiltrator_desktop::proxy::apply_system_proxy(endpoint)
                            .map_err(|e: anyhow::Error| InfiltratorError::Privilege(e.to_string()))
                    },
                    Message::SystemProxySet,
                )
            }
            Message::SystemProxySet(result) => match result {
                Ok(_) => Task::none(),
                Err(e) => {
                    self.system_proxy_enabled = !self.system_proxy_enabled;
                    self.error_msg = Some(e.to_string());
                    Task::none()
                }
            },
            Message::RequestAdminPrivilege => {
                #[cfg(target_os = "windows")]
                {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new("powershell")
                            .arg("-Command")
                            .arg(format!(
                                "Start-Process -FilePath '{}' -Verb RunAs",
                                exe.to_string_lossy()
                            ))
                            .spawn();
                        return Task::done(Message::Exit);
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    use crate::locales::{Lang, Localizer};
                    let lang = Lang(&self.lang);
                    return Task::done(Message::ShowToast(
                        lang.tr("settings_uac_unsupported").to_string(),
                        crate::types::ToastStatus::Warning,
                    ));
                }
                Task::none()
            }
            Message::TrayIconEvent(tray_icon::TrayIconEvent::Click { .. }) => {
                Task::done(Message::ShowWindow)
            }
            Message::TrayIconEvent(_) => Task::none(),
            Message::MenuEvent(event) => {
                let id = event.id.as_ref();
                match id {
                    "show" => Task::done(Message::ShowWindow),
                    "quit" => Task::done(Message::Exit),
                    "toggle_theme" => Task::done(Message::ToggleTheme),
                    "mode_rule" => Task::done(Message::SetProxyMode("rule".to_string())),
                    "mode_global" => Task::done(Message::SetProxyMode("global".to_string())),
                    "mode_direct" => Task::done(Message::SetProxyMode("direct".to_string())),
                    "toggle_system_proxy" => {
                        Task::done(Message::SetSystemProxy(!self.system_proxy_enabled))
                    }
                    "toggle_tun" => {
                        Task::done(Message::SetTunEnabled(!self.tun_enabled.unwrap_or(false)))
                    }
                    _ => {
                        if let Some(node_name) = id.strip_prefix("proxy_GLOBAL_") {
                            return Task::done(Message::SelectProxy(
                                "GLOBAL".to_string(),
                                node_name.to_string(),
                            ));
                        }
                        Task::none()
                    }
                }
            }
            _ => Task::none(),
        }
    }
}
