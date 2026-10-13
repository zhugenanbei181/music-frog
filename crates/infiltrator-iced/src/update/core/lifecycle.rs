//! Core lifecycle commands and app-level autostart wiring.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::{RuntimeStatus, RuntimeStreamState};
use iced::Task;
use infiltrator_contract::core_control::CoreControlAction;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_shared::autostart;

impl AppState {
    pub fn cancel_all_tasks(&mut self) {
        self.shell.last_task_id += 1;
        self.runtime.lifecycle_token = self.runtime.lifecycle_token.wrapping_add(1);
        self.runtime.lifecycle_pending = None;
    }

    /// Runtime start/stop plus autostart toggles. Unmatched messages fall
    /// through to the next domain in the [`update_core`] chain.
    pub(super) fn update_core_lifecycle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartCore => self.submit_shared_core_control(CoreControlAction::Start),
            Message::StopCore => self.submit_shared_core_control(CoreControlAction::Stop),
            Message::CoreControlFinished {
                action,
                token,
                result,
                requested_scope,
                terminal_scope,
                snapshot,
            } => self.finish_shared_core_control(
                action,
                token,
                result,
                requested_scope,
                terminal_scope,
                *snapshot,
            ),
            Message::CoreStopped => {
                self.runtime.lifecycle_pending = None;
                self.runtime.lifecycle_failure = None;
                self.diag.traffic = None;
                self.diag.traffic_history.clear();
                self.diag.connections = None;
                self.diag.connection_groups.clear_observation();
                self.runtime.core_resources = CoreResourceSnapshot::NotObserved;
                self.diag.public_ip = None;
                self.diag.public_ip_provider = None;
                self.diag.public_ip_checked_at = None;
                self.diag.public_ip_error = None;
                self.runtime.runtime_selected_group.clear();
                self.runtime.runtime_selected_proxy.clear();
                self.runtime.runtime_prev_upload_total = None;
                self.runtime.runtime_prev_download_total = None;
                self.runtime.runtime_prev_snapshot_at = None;
                self.runtime.runtime_poll_tick = 0;
                self.diag.logs.clear();
                self.diag.logs_stream_state = RuntimeStreamState::Idle;
                self.diag.traffic_stream_state = RuntimeStreamState::Idle;
                self.diag.connections_stream_state = RuntimeStreamState::Idle;
                self.runtime.routing_intent = None;
                self.runtime.routing_intent_state = Default::default();
                self.runtime.routing_intent_actions.invalidate();
                self.runtime.runtime_control = Default::default();
                self.runtime.tun_enabled = None;
                self.runtime.system_toggles =
                    self.runtime.system_toggles.clone().with_tun_readback(None);
                self.runtime.status = RuntimeStatus::Stopped;
                self.refresh_tray();
                Task::none()
            }
            Message::SetAutostart(enabled) => {
                self.runtime.autostart_enabled = enabled;
                Task::perform(
                    async move {
                        autostart::set_autostart_enabled(crate::AUTOSTART_REG_NAME, enabled)
                            .map_err(|e: anyhow::Error| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::AutostartSet,
                )
            }
            Message::AutostartSet(result) => {
                if let Err(e) = result {
                    self.runtime.autostart_enabled = !self.runtime.autostart_enabled;
                    self.set_error(&e);
                }
                self.refresh_tray();
                Task::none()
            }
            other => self.update_core_settings(other),
        }
    }
}
