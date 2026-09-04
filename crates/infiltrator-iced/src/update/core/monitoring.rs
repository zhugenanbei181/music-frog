//! Live runtime monitoring: the periodic polling loop (traffic, memory,
//! public IP, proxies) plus log ingestion and per-connection operations.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::{IpProbeResult, RuntimeStatus, RuntimeStreamKind, RuntimeStreamState};
use iced::Task;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway};

impl AppState {
    /// Kick one polling round: connections + memory always, proxies every
    /// other tick and runtime config every 6th. Public-egress probing is
    /// intentionally explicit (`FetchIpInfo`) because it contacts an
    /// external provider.
    pub(super) fn schedule_runtime_refresh(&mut self, follow_auto_refresh: bool) -> Task<Message> {
        if follow_auto_refresh && !self.runtime.runtime_auto_refresh {
            return Task::none();
        }
        if !matches!(self.runtime.status, RuntimeStatus::Running) {
            return Task::none();
        }
        let Some(rt) = self.runtime.runtime.clone() else {
            return Task::none();
        };

        self.runtime.runtime_poll_tick = self.runtime.runtime_poll_tick.saturating_add(1);
        let poll_tick = self.runtime.runtime_poll_tick;

        let rt_for_connections = rt.clone();
        let rt_for_memory = rt.clone();
        let mut tasks = vec![
            Task::perform(
                async move {
                    rt_for_connections
                        .get_connections()
                        .await
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))
                },
                |result| match result {
                    Ok(snapshot) => Message::ConnectionsReceived(snapshot),
                    Err(error) => Message::RuntimePollFailed(format!("连接刷新失败: {error}")),
                },
            ),
            Task::perform(
                async move {
                    rt_for_memory
                        .get_memory()
                        .await
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))
                },
                |result| match result {
                    Ok(memory) => Message::MemoryReceived(memory),
                    Err(error) => Message::RuntimePollFailed(format!("内存刷新失败: {error}")),
                },
            ),
        ];

        if poll_tick.is_multiple_of(2) {
            tasks.push(Task::done(Message::LoadProxies));
        }
        if poll_tick.is_multiple_of(6) {
            tasks.push(Task::done(Message::FetchRuntimeConfig));
        }
        Task::batch(tasks)
    }

    /// Polling, live data ingestion, logs and connection operations.
    /// Unmatched messages fall through to the next domain in the
    /// `update_core` chain.
    pub(super) fn update_core_monitoring(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FetchIpInfo => {
                self.cancel_all_tasks();
                let task_id = self.shell.last_task_id;
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let mut builder =
                            reqwest::Client::builder().timeout(std::time::Duration::from_secs(5));
                        let runtime = runtime.ok_or_else(|| {
                            InfiltratorError::Internal(
                                "内核未运行，无法探测代理出口 IP".to_string(),
                            )
                        })?;
                        let endpoint = ManagedRuntime::http_proxy_endpoint(runtime.as_ref())
                            .await
                            .map_err(|error| InfiltratorError::Internal(error.to_string()))?
                            .ok_or_else(|| {
                                InfiltratorError::Internal(
                                    "当前配置未提供可用的 port 或 mixed-port".to_string(),
                                )
                            })?;
                        let proxy = reqwest::Proxy::http(format!("http://{endpoint}"))
                            .map_err(|error| InfiltratorError::Internal(error.to_string()))?;
                        builder = builder.proxy(proxy);
                        let client = builder
                            .build()
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?;
                        let provider = "api.ipify.org".to_string();
                        let resp = client
                            .get("https://api.ipify.org")
                            .send()
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .text()
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?;
                        let ip = resp.trim().to_string();
                        if ip.parse::<std::net::IpAddr>().is_err() {
                            return Err(InfiltratorError::Internal(format!(
                                "出口 IP provider {provider} 返回了无效地址"
                            )));
                        }
                        Ok(IpProbeResult {
                            ip,
                            provider,
                            checked_at: chrono::Local::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                        })
                    },
                    move |res| Message::IpInfoReceived(res, task_id),
                )
            }
            Message::RefreshRuntimeNow => self.schedule_runtime_refresh(false),
            Message::TickRuntimeRefresh => self.schedule_runtime_refresh(true),
            Message::UpdateRuntimeAutoRefresh(enabled) => {
                self.runtime.runtime_auto_refresh = enabled;
                if !enabled {
                    self.runtime.runtime_poll_tick = 0;
                    return self.persist_runtime_panel_settings_task();
                }
                Task::batch(vec![
                    Task::done(Message::RefreshRuntimeNow),
                    self.persist_runtime_panel_settings_task(),
                ])
            }
            Message::TrafficReceived(data) => {
                self.diag.traffic = Some(data.clone());
                self.diag.traffic_history.push_back((data.up, data.down));
                if self.diag.traffic_history.len() > 60 {
                    self.diag.traffic_history.pop_front();
                }
                Task::none()
            }
            Message::MemoryReceived(data) => {
                self.diag.memory = Some(data);
                Task::none()
            }
            Message::IpInfoReceived(result, task_id) => {
                if task_id == self.shell.last_task_id {
                    match result {
                        Ok(result) => {
                            self.diag.public_ip = Some(result.ip);
                            self.diag.public_ip_provider = Some(result.provider);
                            self.diag.public_ip_checked_at = Some(result.checked_at);
                            self.diag.public_ip_error = None;
                        }
                        Err(e) => {
                            self.diag.public_ip_error = Some(e.to_string());
                            self.set_error(&e);
                        }
                    }
                }
                Task::none()
            }
            Message::ConnectionsReceived(data) => {
                let upload_total = data.upload_total;
                let download_total = data.download_total;

                // The WebSocket traffic stream is authoritative. If a core
                // does not expose it, retain a safe polling fallback based on
                // the actual elapsed time rather than assuming every refresh
                // happened exactly two seconds apart.
                let now = std::time::Instant::now();
                if !matches!(
                    self.diag.traffic_stream_state,
                    RuntimeStreamState::Connected
                ) && let (Some(prev_up), Some(prev_down), Some(previous_at)) = (
                    self.runtime.runtime_prev_upload_total,
                    self.runtime.runtime_prev_download_total,
                    self.runtime.runtime_prev_snapshot_at,
                ) {
                    let elapsed = now.duration_since(previous_at).as_secs_f64();
                    if elapsed > 0.0 {
                        let up_rate =
                            (upload_total.saturating_sub(prev_up) as f64 / elapsed) as u64;
                        let down_rate =
                            (download_total.saturating_sub(prev_down) as f64 / elapsed) as u64;
                        self.diag.traffic = Some(infiltrator_domain::runtime::TrafficData {
                            up: up_rate,
                            down: down_rate,
                        });
                        self.diag.traffic_history.push_back((up_rate, down_rate));
                        if self.diag.traffic_history.len() > 60 {
                            self.diag.traffic_history.pop_front();
                        }
                    }
                }

                self.runtime.runtime_prev_upload_total = Some(upload_total);
                self.runtime.runtime_prev_download_total = Some(download_total);
                self.runtime.runtime_prev_snapshot_at = Some(now);
                self.diag.connections = Some(data);
                self.clamp_connections_page();
                Task::none()
            }
            Message::ConnectionsPrevPage => {
                self.diag.connections_page = self.diag.connections_page.saturating_sub(1);
                Task::none()
            }
            Message::ConnectionsNextPage => {
                self.diag.connections_page += 1;
                self.clamp_connections_page();
                Task::none()
            }
            Message::LogReceived(log) => {
                self.diag.logs.push_back(log);
                if self.diag.logs.len() > 500 {
                    self.diag.logs.pop_front();
                }
                iced::widget::operation::snap_to(
                    iced::widget::Id::new("log_scroller"),
                    iced::widget::scrollable::RelativeOffset::END,
                )
            }
            Message::RuntimeStreamLogReceived(generation, log) => {
                if generation == self.runtime.runtime_generation {
                    return self.update_core_monitoring(Message::LogReceived(log));
                }
                Task::none()
            }
            Message::RuntimeStreamTrafficReceived(generation, data) => {
                if generation == self.runtime.runtime_generation {
                    return self.update_core_monitoring(Message::TrafficReceived(data));
                }
                Task::none()
            }
            Message::RuntimeStreamConnectionsReceived(generation, data) => {
                if generation == self.runtime.runtime_generation {
                    return self.update_core_monitoring(Message::ConnectionsReceived(data));
                }
                Task::none()
            }
            Message::RuntimeStreamStateChanged {
                kind,
                generation,
                state,
            } => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                match kind {
                    RuntimeStreamKind::Logs => self.diag.logs_stream_state = state.clone(),
                    RuntimeStreamKind::Traffic => self.diag.traffic_stream_state = state.clone(),
                    RuntimeStreamKind::Connections => {
                        self.diag.connections_stream_state = state.clone()
                    }
                }
                if let RuntimeStreamState::Failed(error) = state {
                    self.set_error(InfiltratorError::Mihomo(format!(
                        "{} stream unavailable: {error}",
                        stream_kind_label(kind)
                    )));
                }
                Task::none()
            }
            Message::RuntimePollFailed(error) => {
                self.set_error(InfiltratorError::Mihomo(error));
                Task::none()
            }
            Message::ClearRuntimeLogs => {
                self.diag.logs.clear();
                Task::none()
            }
            Message::SetLogLevel(level) => {
                self.diag.log_level = level.clone();
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.patch_config(serde_json::json!({ "log-level": level }))
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseConnection(id) => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.close_connection(&id)
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseAllConnections => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.close_all_connections()
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            other => self.update_core_doctor(other),
        }
    }

    /// Keep the connections page inside the valid range for the current
    /// snapshot size (snapshots shrink as connections close).
    pub(crate) fn clamp_connections_page(&mut self) {
        let total = self
            .diag
            .connections
            .as_ref()
            .map(|snapshot| snapshot.connections.len())
            .unwrap_or(0);
        let size = self.diag.connections_page_size.max(1);
        let max_page = total.saturating_sub(1) / size;
        self.diag.connections_page = self.diag.connections_page.min(max_page);
    }

    /// `(page, start, end)` window into the filtered connection list after
    /// clamping; the view slices with these bounds.
    pub(crate) fn connections_window(&self, total: usize) -> (usize, usize, usize) {
        let size = self.diag.connections_page_size.max(1);
        let max_page = total.saturating_sub(1) / size;
        let page = self.diag.connections_page.min(max_page);
        let start = page * size;
        (page, start, (start + size).min(total))
    }
}

fn stream_kind_label(kind: RuntimeStreamKind) -> &'static str {
    match kind {
        RuntimeStreamKind::Logs => "logs",
        RuntimeStreamKind::Traffic => "traffic",
        RuntimeStreamKind::Connections => "connections",
    }
}
