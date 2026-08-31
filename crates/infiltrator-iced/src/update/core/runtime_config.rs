//! Live runtime configuration: querying the running mihomo for its active
//! config and patching mode/TUN/sniffer toggles through the REST API.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::{RuntimeConfig, RuntimePatchSnapshot};
use infiltrator_shared::locales::Localizer;
use iced::Task;
use infiltrator_core::error::InfiltratorError;
use infiltrator_desktop::tun_service::{ServiceModeStatus, TunServiceManager};

impl AppState {
    fn runtime_unavailable(&mut self, operation: &str) -> Task<Message> {
        let error = InfiltratorError::Internal(format!("内核未运行，无法{operation}"));
        self.set_error(&error);
        Task::done(Message::ShowToast(
            error.to_string(),
            crate::types::app::ToastStatus::Error,
        ))
    }

    fn begin_runtime_patch(&mut self) -> u64 {
        if self.runtime.runtime.is_some() {
            if self.runtime.pending_runtime_patch.is_none() {
                self.runtime.pending_runtime_patch = Some(RuntimePatchSnapshot {
                    proxy_mode: self.runtime.proxy_mode.clone(),
                    tun_enabled: self.runtime.tun_enabled,
                    tun_stack: self.editor.tun_stack.clone(),
                    tun_auto_route: self.editor.tun_auto_route,
                    tun_strict_route: self.editor.tun_strict_route,
                    sniffer_enabled: self.editor.sniffer_enabled,
                });
            }
            self.runtime.runtime_patch_token = self.runtime.runtime_patch_token.wrapping_add(1);
        }
        self.runtime.runtime_patch_token
    }

    fn restore_runtime_patch(&mut self) {
        if let Some(previous) = self.runtime.pending_runtime_patch.take() {
            self.runtime.proxy_mode = previous.proxy_mode;
            self.runtime.tun_enabled = previous.tun_enabled;
            self.editor.tun_stack = previous.tun_stack;
            self.editor.tun_auto_route = previous.tun_auto_route;
            self.editor.tun_strict_route = previous.tun_strict_route;
            self.editor.sniffer_enabled = previous.sniffer_enabled;
            self.refresh_tray();
        }
    }

    fn patch_tun_enabled(&mut self, enabled: bool) -> Task<Message> {
        let Some(rt) = self.runtime.runtime.clone() else {
            if enabled {
                let error = InfiltratorError::Privilege("内核未运行，无法启用 TUN".to_string());
                self.set_error(&error);
                return Task::done(Message::ShowToast(
                    error.to_string(),
                    crate::types::app::ToastStatus::Error,
                ));
            }
            return Task::none();
        };
        let token = self.begin_runtime_patch();
        let generation = rt.session().generation();
        self.runtime.tun_enabled = Some(enabled);
        self.refresh_tray();
        Task::perform(
            async move {
                rt.client()
                    .patch_config(serde_json::json!({ "tun": { "enable": enabled } }))
                    .await
                    .map_err(InfiltratorError::from)
            },
            move |result| Message::RuntimePatchResult(result, token, generation),
        )
    }

    pub(crate) fn install_or_start_tun_service(
        &mut self,
        status: ServiceModeStatus,
    ) -> Task<Message> {
        let Some(runtime) = self.runtime.runtime.clone() else {
            return Task::done(Message::ShowToast(
                "内核未运行，无法准备 TUN 服务".to_string(),
                crate::types::app::ToastStatus::Error,
            ));
        };
        self.shell.error_msg = None;
        self.editor.is_saving_tun = false;
        let binary = runtime.core_binary_path().to_path_buf();
        self.runtime.tun_service_status = Some(status);
        self.runtime.is_installing_tun_service = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match status {
                    ServiceModeStatus::InstalledStopped => TunServiceManager::start_service(),
                    ServiceModeStatus::NotInstalled | ServiceModeStatus::MissingPrivilege => {
                        TunServiceManager::install_service(&binary)
                    }
                    ServiceModeStatus::InstalledAndRunning | ServiceModeStatus::Unsupported => {
                        Ok(())
                    }
                })
                .await
                .map_err(|error| InfiltratorError::Privilege(error.to_string()))?
                .map_err(|error| InfiltratorError::Privilege(error.to_string()))
            },
            Message::TunServiceInstalled,
        )
    }

    /// Runtime config fetch and live patch toggles. Unmatched messages fall
    /// through to the next domain in the `update_core` chain.
    pub(super) fn update_core_runtime_config(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FetchRuntimeConfig => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    let generation = rt.session().generation();
                    Task::perform(
                        async move {
                            let config = rt
                                .client()
                                .get_config()
                                .await
                                .map_err(InfiltratorError::from)?;
                            let mode = config.mode;
                            let (tun_en, tun_st, tun_ar, tun_sr) = config
                                .tun
                                .map(|t| (t.enable, t.stack, t.auto_route, t.strict_route))
                                .unwrap_or((false, String::new(), false, false));
                            let (dns, fallback, enhanced) = config
                                .dns
                                .map(|d| {
                                    (
                                        d.nameserver,
                                        d.fallback.unwrap_or_default(),
                                        d.enhanced_mode,
                                    )
                                })
                                .unwrap_or((vec![], vec![], String::new()));
                            let sniff = config.sniffer.map(|s| s.enable).unwrap_or(false);
                            let script_block_present = config.script.is_some();
                            Ok(RuntimeConfig {
                                mode,
                                script_block_present,
                                tun_enabled: tun_en,
                                dns_nameservers: dns,
                                dns_fallback: fallback,
                                dns_enhanced_mode: enhanced,
                                tun_stack: tun_st,
                                tun_auto_route: tun_ar,
                                tun_strict_route: tun_sr,
                                sniffer_enabled: sniff,
                            })
                        },
                        move |result| Message::RuntimeConfigFetched(result, generation),
                    )
                } else {
                    Task::none()
                }
            }
            Message::RuntimeConfigFetched(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                match result {
                    Ok(config) => {
                        self.runtime.proxy_mode = Some(config.mode);
                        self.runtime.script_block_present = config.script_block_present;
                        self.runtime.tun_enabled = Some(config.tun_enabled);
                        self.editor.dns_nameservers = config.dns_nameservers;
                        self.editor.dns_fallback_servers = config.dns_fallback;
                        self.editor.dns_enhanced_mode = config.dns_enhanced_mode;
                        self.editor.tun_stack = config.tun_stack;
                        self.editor.tun_auto_route = config.tun_auto_route;
                        self.editor.tun_strict_route = config.tun_strict_route;
                        self.editor.sniffer_enabled = config.sniffer_enabled;
                        self.refresh_tray();
                    }
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            Message::SetProxyMode(mode) => {
                // `mode: script` is only valid when the loaded profile carries
                // a top-level `script:` block (the core reports it via
                // `GET /configs`); refuse the patch otherwise.
                if mode == "script" && !self.runtime.script_block_present {
                    let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                    let error = InfiltratorError::Config(
                        lang.tr("toast_script_mode_unavailable").into_owned(),
                    );
                    return Task::done(Message::ShowToast(
                        error.to_string(),
                        crate::types::app::ToastStatus::Error,
                    ));
                }
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable("切换代理模式");
                };
                let token = self.begin_runtime_patch();
                let generation = rt.session().generation();
                self.runtime.proxy_mode = Some(mode.clone());
                self.refresh_tray();
                Task::perform(
                    async move {
                        rt.client()
                            .patch_config(serde_json::json!({ "mode": mode }))
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::ModeSetResult(result) => match result {
                Ok(_) => {
                    self.runtime.pending_runtime_patch = None;
                    Task::done(Message::FetchRuntimeConfig)
                }
                Err(e) => {
                    self.restore_runtime_patch();
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::SetTunEnabled(enabled) => {
                if self.runtime.runtime.is_none() {
                    return self.runtime_unavailable("修改 TUN 状态");
                }
                if enabled && let Some(runtime) = self.runtime.runtime.clone() {
                    let status = runtime.tun_service_status();
                    self.runtime.tun_service_status = Some(status);
                    match status {
                        ServiceModeStatus::InstalledAndRunning => {}
                        ServiceModeStatus::InstalledStopped
                        | ServiceModeStatus::NotInstalled
                        | ServiceModeStatus::MissingPrivilege => {
                            return self.install_or_start_tun_service(status);
                        }
                        ServiceModeStatus::Unsupported => {
                            let error = InfiltratorError::Privilege(
                                "当前平台未提供 TUN 服务模式".to_string(),
                            );
                            self.set_error(&error);
                            return Task::done(Message::ShowToast(
                                error.to_string(),
                                crate::types::app::ToastStatus::Error,
                            ));
                        }
                    }
                }
                self.patch_tun_enabled(enabled)
            }
            Message::InstallTunService => {
                let Some(runtime) = self.runtime.runtime.clone() else {
                    let error =
                        InfiltratorError::Privilege("内核未运行，无法准备 TUN 服务".to_string());
                    self.set_error(&error);
                    return Task::done(Message::ShowToast(
                        error.to_string(),
                        crate::types::app::ToastStatus::Error,
                    ));
                };
                let status = runtime.tun_service_status();
                self.runtime.tun_service_status = Some(status);
                match status {
                    ServiceModeStatus::InstalledAndRunning => Task::done(Message::ShowToast(
                        "TUN 服务已就绪".to_string(),
                        crate::types::app::ToastStatus::Success,
                    )),
                    ServiceModeStatus::InstalledStopped
                    | ServiceModeStatus::NotInstalled
                    | ServiceModeStatus::MissingPrivilege => {
                        self.install_or_start_tun_service(status)
                    }
                    ServiceModeStatus::Unsupported => {
                        let error =
                            InfiltratorError::Privilege("当前平台未提供 TUN 服务模式".to_string());
                        self.set_error(&error);
                        Task::done(Message::ShowToast(
                            error.to_string(),
                            crate::types::app::ToastStatus::Error,
                        ))
                    }
                }
            }
            Message::RefreshTunServiceStatus => {
                let Some(runtime) = self.runtime.runtime.clone() else {
                    self.runtime.tun_service_status = None;
                    return Task::none();
                };
                let binary = runtime.core_binary_path().to_path_buf();
                Task::perform(
                    tokio::task::spawn_blocking(move || {
                        TunServiceManager::check_status_for(&binary)
                    }),
                    |result| match result {
                        Ok(status) => Message::TunServiceStatusLoaded(Ok(status)),
                        Err(error) => Message::TunServiceStatusLoaded(Err(
                            InfiltratorError::Privilege(error.to_string()),
                        )),
                    },
                )
            }
            Message::TunServiceStatusLoaded(result) => {
                match result {
                    Ok(status) => self.runtime.tun_service_status = Some(status),
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            Message::TunServiceInstalled(result) => {
                self.runtime.is_installing_tun_service = false;
                match result {
                    Ok(()) => {
                        self.runtime.tun_service_status = self
                            .runtime
                            .runtime
                            .as_ref()
                            .map(|runtime| runtime.tun_service_status());
                        self.patch_tun_enabled(true)
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(
                            error.to_string(),
                            crate::types::app::ToastStatus::Error,
                        ))
                    }
                }
            }
            Message::SetTunStack(stack) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable("修改 TUN 堆栈");
                };
                let token = self.begin_runtime_patch();
                let generation = rt.session().generation();
                self.editor.tun_stack = stack.clone();
                Task::perform(
                    async move {
                        rt.client()
                            .patch_config(serde_json::json!({ "tun": { "stack": stack } }))
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetTunAutoRoute(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable("修改 TUN 自动路由");
                };
                let token = self.begin_runtime_patch();
                let generation = rt.session().generation();
                self.editor.tun_auto_route = enabled;
                Task::perform(
                    async move {
                        rt.client()
                            .patch_config(serde_json::json!({ "tun": { "auto-route": enabled } }))
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetTunStrictRoute(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable("修改 TUN 严格路由");
                };
                let token = self.begin_runtime_patch();
                let generation = rt.session().generation();
                self.editor.tun_strict_route = enabled;
                Task::perform(
                    async move {
                        rt.client()
                            .patch_config(serde_json::json!({ "tun": { "strict-route": enabled } }))
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetSnifferEnabled(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable("修改嗅探器状态");
                };
                let token = self.begin_runtime_patch();
                let generation = rt.session().generation();
                self.editor.sniffer_enabled = enabled;
                Task::perform(
                    async move {
                        rt.client()
                            .patch_config(serde_json::json!({ "sniffer": { "enable": enabled } }))
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::RuntimePatchResult(result, token, generation) => {
                if token != self.runtime.runtime_patch_token
                    || generation != self.runtime.runtime_generation
                {
                    return Task::none();
                }
                match result {
                    Ok(_) => {
                        self.runtime.pending_runtime_patch = None;
                        Task::done(Message::FetchRuntimeConfig)
                    }
                    Err(error) => {
                        self.restore_runtime_patch();
                        self.set_error(&error);
                        Task::none()
                    }
                }
            }
            Message::OperationResult(result) => match result {
                Ok(_) => Task::done(Message::FetchRuntimeConfig),
                Err(e) => {
                    self.set_error(&e);
                    Task::none()
                }
            },
            other => self.update_core_rules(other),
        }
    }
}
