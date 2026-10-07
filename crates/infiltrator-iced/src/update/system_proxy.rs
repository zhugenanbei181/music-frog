//! Iced adapter for shared system-proxy reconciliation results.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::system_proxy::{
    SystemProxyOwnership, SystemProxyRecoverySnapshot, SystemProxyRecoveryStatus,
    SystemProxySnapshot, SystemProxyStatus,
};
use infiltrator_contract::system_toggle::SystemToggle;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

impl AppState {
    pub(super) fn set_system_proxy(&mut self, enabled: bool) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        if self.runtime.system_proxy_pending {
            return Task::none();
        }
        if let Err(failure) = SystemToggleApplication::intent(
            &self.runtime.system_toggles,
            SystemToggle::SystemProxy,
            enabled,
        ) {
            let error = InfiltratorError::Privilege(failure.message);
            self.set_error(&error);
            return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
        }
        self.runtime.system_proxy_enabled = enabled;
        self.runtime.system_proxy_pending = true;
        self.runtime.system_toggles = self
            .runtime
            .system_toggles
            .clone()
            .with_pending(SystemToggle::SystemProxy, enabled);
        self.refresh_tray();
        let runtime = self.runtime.runtime.clone();
        let proxy_application = self.runtime.system_proxy_application.clone();
        let bypass = if self.shell.system_proxy_bypass.trim().is_empty() {
            None
        } else {
            Some(self.shell.system_proxy_bypass.trim().to_string())
        };
        Task::perform(
            async move {
                let application = proxy_application.ok_or_else(|| {
                    InfiltratorError::Privilege(
                        Lang(&copy_locale)
                            .tr("system_proxy_control_unavailable")
                            .into_owned(),
                    )
                })?;
                let endpoint = if enabled {
                    let runtime = runtime.ok_or_else(|| {
                        InfiltratorError::Privilege(
                            Lang(&copy_locale)
                                .tr("system_proxy_requires_core")
                                .into_owned(),
                        )
                    })?;
                    runtime
                        .http_proxy_endpoint()
                        .await
                        .map_err(|error| InfiltratorError::Privilege(error.to_string()))?
                        .ok_or_else(|| {
                            InfiltratorError::Privilege(
                                Lang(&copy_locale)
                                    .tr("system_proxy_port_missing")
                                    .into_owned(),
                            )
                        })?
                } else {
                    String::new()
                };
                application
                    .set_enabled(enabled, enabled.then_some(endpoint), bypass)
                    .await
                    .map_err(|failure| InfiltratorError::Privilege(failure.message))
            },
            Message::SystemProxySet,
        )
    }

    pub(super) fn finish_system_proxy_set(
        &mut self,
        result: Result<SystemProxySnapshot, InfiltratorError>,
    ) -> Task<Message> {
        match result {
            Ok(snapshot) => {
                self.runtime.system_proxy_pending = false;
                self.runtime.system_proxy_enabled = snapshot.is_enabled();
                self.runtime.system_proxy = snapshot;
                self.runtime.system_toggles = self
                    .runtime
                    .system_toggles
                    .clone()
                    .with_system_proxy_readback(self.runtime.system_proxy_enabled)
                    .with_legacy_tun(self.runtime.tun_enabled);
                self.refresh_tray();
                Task::none()
            }
            Err(error) => {
                self.runtime.system_proxy_pending = false;
                self.runtime.system_proxy_enabled = !self.runtime.system_proxy_enabled;
                self.runtime.system_toggles = self
                    .runtime
                    .system_toggles
                    .clone()
                    .with_system_proxy_readback(self.runtime.system_proxy_enabled)
                    .with_legacy_tun(self.runtime.tun_enabled);
                self.refresh_tray();
                self.set_error(&error);
                Task::none()
            }
        }
    }

    pub(super) fn finish_system_proxy_recovery(
        &mut self,
        snapshot: SystemProxyRecoverySnapshot,
    ) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        self.runtime.system_proxy_recovery = snapshot.clone();
        match &snapshot.status {
            SystemProxyRecoveryStatus::Restored { .. } => Task::done(Message::ShowToast(
                Lang(&copy_locale)
                    .tr("system_proxy_orphan_restored")
                    .into_owned(),
                ToastStatus::Warning,
            )),
            SystemProxyRecoveryStatus::SkippedExternal { .. } => Task::done(Message::ShowToast(
                Lang(&copy_locale)
                    .tr("system_proxy_external_preserved")
                    .into_owned(),
                ToastStatus::Warning,
            )),
            SystemProxyRecoveryStatus::Failed { failure } => {
                self.set_error(&failure.message);
                Task::done(Message::ShowToast(
                    localize(
                        &copy_locale,
                        "system_proxy_recovery_failed_notice",
                        &[("reason", failure.message.clone())],
                    ),
                    ToastStatus::Error,
                ))
            }
            SystemProxyRecoveryStatus::Unknown
            | SystemProxyRecoveryStatus::NotNeeded
            | SystemProxyRecoveryStatus::SkippedLiveOwner { .. } => Task::none(),
        }
    }

    pub(super) fn reconcile_system_proxy(
        &mut self,
        snapshot: SystemProxySnapshot,
    ) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        self.runtime.system_proxy = snapshot.clone();
        if matches!(
            &snapshot.status,
            SystemProxyStatus::Enabled | SystemProxyStatus::Disabled
        ) {
            self.runtime.system_proxy_enabled = snapshot.is_enabled();
            self.runtime.system_toggles = self
                .runtime
                .system_toggles
                .clone()
                .with_system_proxy_readback(snapshot.is_enabled());
        }
        if snapshot.ownership == SystemProxyOwnership::Repaired
            && snapshot.repair_count > self.runtime.system_proxy_last_repair_count
        {
            self.runtime.system_proxy_last_repair_count = snapshot.repair_count;
            Task::done(Message::ShowToast(
                Lang(&copy_locale)
                    .tr("system_proxy_owned_restored")
                    .into_owned(),
                ToastStatus::Warning,
            ))
        } else {
            Task::none()
        }
    }
}
