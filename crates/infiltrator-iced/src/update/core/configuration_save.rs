//! Configuration save status shared by DNS, provider, sniffer, and TUN flows.

use crate::configs_dir::config_manager;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::runtime::ConfigurationSaveState;
use iced::Task;
use tokio::time;
use tokio::time::sleep;

impl AppState {
    pub(super) fn begin_configuration_save(&mut self, label: &str) {
        self.runtime.configuration_save_state = ConfigurationSaveState::Saving {
            label: label.to_string(),
        };
    }

    pub(super) fn finish_configuration_save(&mut self, label: String) -> Task<Message> {
        if let Some(runtime) = self.runtime.runtime.clone() {
            // A successful apply may move the CoreApplication to a new
            // generation. Refresh the app/admin snapshot before the stream
            // subscription accepts events from that generation.
            self.sync_runtime_slot(Some(runtime));
        }
        self.runtime.configuration_save_state = ConfigurationSaveState::Done {
            label: label.clone(),
        };
        let clear_backup = Task::perform(
            async {
                if let Ok(manager) = config_manager().await
                    && let Ok(profile) = manager.get_current().await
                {
                    let _ = manager.clear_backup(&profile).await;
                }
            },
            |_| Message::Noop,
        );
        Task::batch(vec![
            clear_backup,
            Task::done(Message::ShowToast(
                format!("{label} saved"),
                ToastStatus::Success,
            )),
            Task::perform(
                async {
                    sleep(time::Duration::from_secs(4)).await;
                },
                |_| Message::ClearConfigurationSaveState,
            ),
        ])
    }

    /// Clear the transient configuration save notice.
    pub(super) fn update_core_configuration_save(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ClearConfigurationSaveState => {
                self.runtime.configuration_save_state = ConfigurationSaveState::Idle;
                Task::none()
            }
            other => self.update_core_kernels(other),
        }
    }
}
