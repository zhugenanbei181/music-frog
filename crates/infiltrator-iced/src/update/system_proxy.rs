//! Iced adapter for shared system-proxy reconciliation results.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::system_proxy::{SystemProxyOwnership, SystemProxySnapshot, SystemProxyStatus};

impl AppState {
    pub(super) fn reconcile_system_proxy(
        &mut self,
        snapshot: SystemProxySnapshot,
    ) -> Task<Message> {
        self.runtime.system_proxy = snapshot.clone();
        if matches!(
            &snapshot.status,
            SystemProxyStatus::Enabled | SystemProxyStatus::Disabled
        ) {
            self.runtime.system_proxy_enabled = snapshot.is_enabled();
        }
        if snapshot.ownership == SystemProxyOwnership::Repaired
            && snapshot.repair_count > self.runtime.system_proxy_last_repair_count
        {
            self.runtime.system_proxy_last_repair_count = snapshot.repair_count;
            Task::done(Message::ShowToast(
                "系统代理设置被其他程序修改，已自动恢复".to_owned(),
                ToastStatus::Warning,
            ))
        } else {
            Task::none()
        }
    }
}
