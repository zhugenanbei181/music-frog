//! Live runtime monitoring: the periodic polling loop (traffic, memory,
//! public IP, proxies) plus log ingestion and per-connection operations.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, RuntimeStatus};
use iced::Task;

impl AppState {
    /// Kick one polling round: connections + memory always, proxies every
    /// other tick, runtime config every 6th and IP info every 15th.
    pub(super) fn schedule_runtime_refresh(&mut self, follow_auto_refresh: bool) -> Task<Message> {
        if follow_auto_refresh && !self.runtime_auto_refresh {
            return Task::none();
        }
        if !matches!(self.status, RuntimeStatus::Running) {
            return Task::none();
        }
        let Some(rt) = self.runtime.clone() else {
            return Task::none();
        };

        self.runtime_poll_tick = self.runtime_poll_tick.saturating_add(1);
        let poll_tick = self.runtime_poll_tick;

        let rt_for_connections = rt.clone();
        let rt_for_memory = rt.clone();
        let mut tasks = vec![
            Task::perform(
                async move {
                    rt_for_connections
                        .client()
                        .get_connections()
                        .await
                        .map_err(InfiltratorError::from)
                },
                |result| match result {
                    Ok(snapshot) => Message::ConnectionsReceived(mihomo_api::types::ConnectionSnapshot {
                        download_total: snapshot.download_total,
                        upload_total: snapshot.upload_total,
                        connections: snapshot.connections,
                    }),
                    Err(_) => Message::Noop,
                },
            ),
            Task::perform(
                async move {
                    rt_for_memory
                        .client()
                        .get_memory()
                        .await
                        .map_err(InfiltratorError::from)
                },
                |result| match result {
                    Ok(memory) => Message::MemoryReceived(memory),
                    Err(_) => Message::Noop,
                },
            ),
        ];

        if poll_tick.is_multiple_of(2) {
            tasks.push(Task::done(Message::LoadProxies));
        }
        if poll_tick.is_multiple_of(6) {
            tasks.push(Task::done(Message::FetchRuntimeConfig));
        }
        if poll_tick.is_multiple_of(15) {
            tasks.push(Task::done(Message::FetchIpInfo));
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
                let task_id = self.last_task_id;
                Task::perform(
                    async move {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(5))
                            .build()
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?;
                        let resp = client
                            .get("https://api.ipify.org")
                            .send()
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .text()
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?;
                        Ok(resp)
                    },
                    move |res| Message::IpInfoReceived(res, task_id),
                )
            }
            Message::RefreshRuntimeNow => self.schedule_runtime_refresh(false),
            Message::TickRuntimeRefresh => self.schedule_runtime_refresh(true),
            Message::UpdateRuntimeAutoRefresh(enabled) => {
                self.runtime_auto_refresh = enabled;
                if !enabled {
                    self.runtime_poll_tick = 0;
                    return self.persist_runtime_panel_settings_task();
                }
                Task::batch(vec![
                    Task::done(Message::RefreshRuntimeNow),
                    self.persist_runtime_panel_settings_task(),
                ])
            }
            Message::TrafficReceived(data) => {
                self.traffic = Some(data.clone());
                self.traffic_history.push_back((data.up, data.down));
                if self.traffic_history.len() > 60 {
                    self.traffic_history.pop_front();
                }
                Task::none()
            }
            Message::MemoryReceived(data) => {
                self.memory = Some(data);
                Task::none()
            }
            Message::IpInfoReceived(result, task_id) => {
                if task_id == self.last_task_id {
                    match result {
                        Ok(ip) => self.public_ip = Some(ip),
                        Err(e) => self.set_error(&e),
                    }
                }
                Task::none()
            }
            Message::ConnectionsReceived(data) => {
                let upload_total = data.upload_total;
                let download_total = data.download_total;

                if let (Some(prev_up), Some(prev_down)) = (
                    self.runtime_prev_upload_total,
                    self.runtime_prev_download_total,
                ) {
                    let up_rate = upload_total.saturating_sub(prev_up) / 2;
                    let down_rate = download_total.saturating_sub(prev_down) / 2;
                    self.traffic = Some(mihomo_api::types::TrafficData {
                        up: up_rate,
                        down: down_rate,
                    });
                    self.traffic_history.push_back((up_rate, down_rate));
                    if self.traffic_history.len() > 60 {
                        self.traffic_history.pop_front();
                    }
                }

                self.runtime_prev_upload_total = Some(upload_total);
                self.runtime_prev_download_total = Some(download_total);
                self.connections = Some(data);
                Task::none()
            }
            Message::LogReceived(log) => {
                self.logs.push_back(log);
                if self.logs.len() > 500 {
                    self.logs.pop_front();
                }
                iced::widget::operation::snap_to(
                    iced::widget::Id::new("log_scroller"),
                    iced::widget::scrollable::RelativeOffset::END,
                )
            }
            Message::ClearRuntimeLogs => {
                self.logs.clear();
                Task::none()
            }
            Message::SetLogLevel(level) => {
                self.log_level = level.clone();
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "log-level": level }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseConnection(id) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .close_connection(&id)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseAllConnections => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .close_all_connections()
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            other => self.update_core_proxies(other),
        }
    }
}
