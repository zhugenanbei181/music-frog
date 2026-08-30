//! Live runtime configuration: querying the running mihomo for its active
//! config and patching mode/TUN/sniffer toggles through the REST API.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, RuntimeConfig};
use iced::Task;

impl AppState {
    /// Runtime config fetch and live patch toggles. Unmatched messages fall
    /// through to the next domain in the `update_core` chain.
    pub(super) fn update_core_runtime_config(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FetchRuntimeConfig => {
                if let Some(rt) = self.runtime.runtime.clone() {
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
                            Ok(RuntimeConfig {
                                mode,
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
                        Message::RuntimeConfigFetched,
                    )
                } else {
                    Task::none()
                }
            }
            Message::RuntimeConfigFetched(result) => {
                if let Ok(config) = result {
                    self.runtime.proxy_mode = Some(config.mode);
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
                Task::none()
            }
            Message::SetProxyMode(mode) => {
                self.runtime.proxy_mode = Some(mode.clone());
                self.refresh_tray();
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "mode": mode }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::ModeSetResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ModeSetResult(result) => match result {
                Ok(_) => Task::done(Message::FetchRuntimeConfig),
                Err(e) => {
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::SetTunEnabled(enabled) => {
                self.runtime.tun_enabled = Some(enabled);
                self.refresh_tray();
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "tun": { "enable": enabled } }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetTunStack(stack) => {
                self.editor.tun_stack = stack.clone();
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "tun": { "stack": stack } }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetTunAutoRoute(enabled) => {
                self.editor.tun_auto_route = enabled;
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(
                                    serde_json::json!({ "tun": { "auto-route": enabled } }),
                                )
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetTunStrictRoute(enabled) => {
                self.editor.tun_strict_route = enabled;
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(
                                    serde_json::json!({ "tun": { "strict-route": enabled } }),
                                )
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetSnifferEnabled(enabled) => {
                self.editor.sniffer_enabled = enabled;
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(
                                    serde_json::json!({ "sniffer": { "enable": enabled } }),
                                )
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
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
