//! Live runtime monitoring: scoped stream ingestion, shared resource snapshots,
//! explicit public-egress probes, and scoped per-connection operations.

use crate::network::application;
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::{
    IpProbeResult, RuntimeStatus, RuntimeStreamKind, RuntimeStreamState, current_unix_secs,
};
use iced::Task;
use iced::widget::Id;
use iced::widget::operation::{scroll_to, snap_to};
use iced::widget::scrollable::{AbsoluteOffset, RelativeOffset};
use infiltrator_application::connection_application::ConnectionApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_domain::connection_view::matches_search;
use infiltrator_domain::runtime::ConnectionSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::InfiltratorError;
use std::time::Instant;

impl AppState {
    pub(crate) fn sync_logs_viewport_task(&self) -> Task<Message> {
        if self.diag.logs_page.follow().should_follow() {
            snap_to(Id::new("log_scroller"), RelativeOffset::END)
        } else {
            scroll_to(
                Id::new("log_scroller"),
                AbsoluteOffset {
                    x: None,
                    y: Some(self.diag.logs_page.follow().restored_offset(f32::MAX)),
                },
            )
        }
    }

    /// Kick one polling round: scoped streams own live connections and traffic,
    /// while the shared surface pump publishes scoped resource snapshots.
    /// This refreshes slower outbound/config read models.
    /// Public-egress probing is intentionally explicit (`FetchIpInfo`).
    pub(super) fn schedule_runtime_refresh(&mut self, follow_auto_refresh: bool) -> Task<Message> {
        if follow_auto_refresh && !self.runtime.runtime_auto_refresh {
            return Task::none();
        }
        if !matches!(self.runtime.status, RuntimeStatus::Running) {
            return Task::none();
        }
        if self.runtime.runtime.is_none() {
            return Task::none();
        }

        self.runtime.runtime_poll_tick = self.runtime.runtime_poll_tick.saturating_add(1);
        let poll_tick = self.runtime.runtime_poll_tick;

        let mut tasks = Vec::new();

        if poll_tick.is_multiple_of(2) {
            tasks.push(Task::done(Message::RefreshOutboundInventory));
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
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::FetchIpInfo => {
                self.cancel_all_tasks();
                let task_id = self.shell.last_task_id;
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let runtime = runtime.ok_or_else(|| {
                            InfiltratorError::Internal(
                                Lang(&copy_locale)
                                    .tr("core_egress_not_running")
                                    .into_owned(),
                            )
                        })?;
                        let snapshot = application()
                            .probe_through_runtime(runtime)
                            .await
                            .map_err(|failure| InfiltratorError::Internal(failure.message))?;
                        let checked_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                            snapshot.checked_at_epoch_ms,
                        )
                        .map(|value| {
                            value
                                .with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                        })
                        .unwrap_or_default();
                        Ok(IpProbeResult {
                            ip: snapshot.ip,
                            provider: snapshot.provider,
                            checked_at,
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
                if self.commands.is_some() {
                    return Task::none();
                }
                self.diag.traffic = Some(data.clone());
                self.diag.traffic_history.push_back((data.up, data.down));
                if self.diag.traffic_history.len() > 60 {
                    self.diag.traffic_history.pop_front();
                }
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
                self.apply_connections_snapshot(data, Instant::now())
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
                if self.commands.is_some() {
                    return Task::none();
                }
                self.diag.logs.push_back(log);
                if self.diag.logs.len() > 500 {
                    self.diag.logs.pop_front();
                }
                self.diag.observe_log_records();
                self.sync_logs_viewport_task()
            }
            Message::RuntimeStreamLogReceived(scope, log) => {
                if self.core_lifecycle_snapshot().scope().as_ref() == Some(&scope) {
                    return self.update_core_monitoring(Message::LogReceived(log));
                }
                Task::none()
            }
            Message::RuntimeStreamTrafficReceived(scope, data) => {
                if self.core_lifecycle_snapshot().scope().as_ref() == Some(&scope) {
                    return self.update_core_monitoring(Message::TrafficReceived(data));
                }
                Task::none()
            }
            Message::RuntimeStreamConnectionsReceived(scope, data) => {
                if self.core_lifecycle_snapshot().scope().as_ref() == Some(&scope) {
                    return self.update_core_monitoring(Message::ConnectionsReceived(data));
                }
                Task::none()
            }
            Message::RuntimeStreamStateChanged { kind, scope, state } => {
                if self.core_lifecycle_snapshot().scope().as_ref() != Some(&scope) {
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
                    self.set_error(InfiltratorError::Backend(format!(
                        "{} stream unavailable: {error}",
                        stream_kind_label(kind)
                    )));
                }
                Task::none()
            }
            Message::LogsCommandFinished { generation, result } => {
                if generation == self.runtime.core_lifecycle.generation {
                    self.diag.log_command_failure = result.err();
                }
                Task::none()
            }
            Message::ToggleLogFollow => {
                self.diag.logs_page.follow_mut().toggle_follow();
                self.sync_logs_viewport_task()
            }
            Message::LogsScrolled {
                offset,
                content,
                viewport,
            } => {
                self.diag
                    .logs_page
                    .follow_mut()
                    .observe_viewport(offset, content, viewport);
                Task::none()
            }
            Message::ClearRuntimeLogs => {
                if let Some(application) = self.command_ingress() {
                    let generation = self.runtime.core_lifecycle.generation;
                    return Task::perform(
                        async move {
                            application
                                .execute(CommandIntent::ClearLogs)
                                .await
                                .into_unit()
                                .map(|_| ())
                        },
                        move |result| Message::LogsCommandFinished { generation, result },
                    );
                }

                self.diag.logs.clear();
                self.diag.observe_log_records();
                Task::none()
            }
            Message::SetCoreLogLevel(level) => {
                if let Some(application) = self.command_ingress() {
                    let previous = self.diag.log_level.clone();
                    self.diag.log_level = level.as_key().to_owned();
                    return Task::perform(
                        async move {
                            application
                                .execute(CommandIntent::SetCoreLogLevel { level })
                                .await
                                .into_unit()
                                .map_err(|failure| InfiltratorError::Internal(failure.message))
                        },
                        move |result| Message::CoreLogLevelFinished(result, previous),
                    );
                }
                let message = if self.runtime.runtime.is_none() {
                    "core is not running; log level was not changed"
                } else {
                    "Core owner command interface is unavailable; log level was not changed"
                };
                let error = InfiltratorError::Internal(message.to_owned());
                self.set_error(&error);
                Task::none()
            }
            Message::CoreLogLevelFinished(result, previous) => {
                if let Err(error) = result {
                    self.diag.log_level = previous;
                    self.set_error(&error);
                }
                Task::none()
            }
            Message::CloseConnection(id) => {
                if self.diag.inspecting_connection_id.as_ref() == Some(&id) {
                    self.diag.inspecting_connection_id = None;
                }
                if let Some(application) = self.command_ingress() {
                    return Task::perform(
                        async move {
                            match (application
                                .execute(CommandIntent::CloseConnection { id })
                                .await)
                                .into_unit()
                            {
                                Ok(()) => Ok(()),
                                Err(failure) => Err(InfiltratorError::Internal(failure.message)),
                            }
                        },
                        Message::OperationResult,
                    );
                }
                if let Some(rt) = self.runtime.runtime.clone() {
                    let application =
                        ConnectionApplication::new(rt.core_scope_source(), rt.connection_port());
                    Task::perform(
                        async move {
                            application
                                .close(&id)
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
                if let Some(application) = self.command_ingress() {
                    Task::perform(
                        async move {
                            match (application
                                .execute(CommandIntent::CloseAllConnections)
                                .await)
                                .into_unit()
                            {
                                Ok(()) => Ok(()),
                                Err(failure) => Err(InfiltratorError::Internal(failure.message)),
                            }
                        },
                        Message::OperationResult,
                    )
                } else if let Some(runtime) = self.runtime.runtime.clone() {
                    let application = ConnectionApplication::new(
                        runtime.core_scope_source(),
                        runtime.connection_port(),
                    );
                    Task::perform(
                        async move {
                            application
                                .close_all()
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseFilteredConnections => {
                // DUAL-13-07: range teardown reuses the shared keyword predicate
                // (DUAL-13-13) over the live snapshot; an empty filter is a
                // no-op so the button can never silently close everything.
                let Some(rt) = self.runtime.runtime.clone() else {
                    return Task::none();
                };
                let application =
                    ConnectionApplication::new(rt.core_scope_source(), rt.connection_port());
                let raw_filter = self.runtime.runtime_connection_filter.clone();
                let query = raw_filter
                    .strip_prefix("tab:closed")
                    .unwrap_or(&raw_filter)
                    .trim()
                    .to_string();
                if query.is_empty() {
                    return Task::none();
                }
                let ids: Vec<String> = self
                    .diag
                    .connections
                    .as_ref()
                    .map(|snapshot| {
                        snapshot
                            .connections
                            .iter()
                            .filter(|conn| matches_search(*conn, &query))
                            .map(|conn| conn.id.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                Task::batch(ids.into_iter().map(|id| {
                    let application = application.clone();
                    Task::perform(
                        async move {
                            application
                                .close(&id)
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        Message::OperationResult,
                    )
                }))
            }
            Message::SetConnectionIdleTimeout(secs) => {
                self.diag.connection_idle_timeout_secs = secs;
                Task::none()
            }
            Message::SweepIdleConnections => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return Task::none();
                };
                let application =
                    ConnectionApplication::new(rt.core_scope_source(), rt.connection_port());
                let timeout = self.diag.connection_idle_timeout_secs;
                let now = current_unix_secs();
                let idle: Vec<String> = match self.diag.connections.clone() {
                    Some(snapshot) => {
                        self.diag
                            .connection_activity
                            .observe(&snapshot.connections, now);
                        self.diag.connection_activity.idle_ids(now, timeout)
                    }
                    None => Vec::new(),
                };
                self.diag.last_idle_sweep = Some(idle.len());
                Task::batch(idle.into_iter().map(|id| {
                    let application = application.clone();
                    Task::perform(
                        async move {
                            application
                                .close(&id)
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))
                        },
                        Message::OperationResult,
                    )
                }))
            }
            other => self.update_core_doctor(other),
        }
    }

    /// Apply one live connections snapshot at `now`: the polling traffic
    /// fallback, the idle-activity bookkeeping and the shared
    /// DUAL-13-10/12 instantaneous-rate derivation all diff against the
    /// previous observation, so the clock is injected instead of read here.
    /// This is the seam the deterministic tests drive.
    pub(crate) fn apply_connections_snapshot(
        &mut self,
        data: ConnectionSnapshot,
        now: Instant,
    ) -> Task<Message> {
        use infiltrator_domain::runtime::TrafficData;

        let upload_total = data.upload_total;
        let download_total = data.download_total;

        // The WebSocket traffic stream is authoritative. If a core does not
        // expose it, retain a safe polling fallback based on the actual
        // elapsed time rather than assuming every refresh happened exactly
        // two seconds apart.
        if self.commands.is_none()
            && !matches!(
                self.diag.traffic_stream_state,
                RuntimeStreamState::Connected
            )
            && let (Some(prev_up), Some(prev_down), Some(previous_at)) = (
                self.runtime.runtime_prev_upload_total,
                self.runtime.runtime_prev_download_total,
                self.runtime.runtime_prev_snapshot_at,
            )
        {
            let elapsed = now.duration_since(previous_at).as_secs_f64();
            if elapsed > 0.0 {
                let up_rate = (upload_total.saturating_sub(prev_up) as f64 / elapsed) as u64;
                let down_rate = (download_total.saturating_sub(prev_down) as f64 / elapsed) as u64;
                self.diag.traffic = Some(TrafficData {
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

        // DUAL-13-10/12: the shared application rate window derives the
        // instantaneous bytes-per-second of every connection in this
        // snapshot; the row pulse and the rate sort read that book.
        self.diag.connection_rate_book = self
            .diag
            .connection_rates
            .observe_at(now, &data.connections);

        // DUAL-13-11: record byte-change times for idle detection.
        let observed_at = current_unix_secs();
        self.diag
            .connection_activity
            .observe(&data.connections, observed_at);
        if self
            .diag
            .inspecting_connection_id
            .as_ref()
            .is_some_and(|id| {
                !data
                    .connections
                    .iter()
                    .any(|connection| &connection.id == id)
            })
        {
            self.diag.inspecting_connection_id = None;
        }
        let filter = self.runtime.runtime_connection_filter.trim();
        self.diag
            .connections_model
            .edit_query(filter.strip_prefix("tab:closed").unwrap_or(filter).trim());
        self.diag.connections_model.observe(&data.connections);
        self.diag.connections = Some(data);
        self.clamp_connections_page();
        Task::none()
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
