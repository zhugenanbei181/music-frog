//! `update::core` — the runtime message core, split by business domain.
//!
//! [`AppState::update_core`] is the single entry point used by the
//! dispatcher in `src/update.rs`; its signature and path
//! (`crate::update::core`) are stable. Each domain submodule owns one slice
//! of the original message match and forwards unmatched messages to the
//! next domain in the chain:
//! lifecycle → settings → monitoring → doctor → proxies → runtime_config →
//! rules → json_editors → mrs → advanced → dns_config → dns_leak →
//! stun → tun_config → rebuild → kernels (fallback).
mod advanced;
mod dns_cache;
mod dns_config;
mod dns_hosts;
mod dns_leak;
mod dns_leak_commands;
mod dns_query;
mod doctor;
mod doctor_commands;
mod json_editors;
mod kernels;
mod lifecycle;
mod monitoring;
mod mrs;
pub(crate) mod profile_apply;
mod proxies;
mod proxy_mode;
mod rebuild;
mod rule_list;
mod rules;
mod rules_provider;
mod runtime_config;
mod settings;
mod shared_lifecycle;
mod stun;
mod tun_config;

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;

impl AppState {
    /// Public entry used by the dispatcher in `src/update.rs`.
    pub fn update_core(&mut self, message: Message) -> Task<Message> {
        if matches!(
            &message,
            Message::SetIpv6Routing(_)
                | Message::SetTunEnabled(_)
                | Message::SetTunStack(_)
                | Message::SetTunAutoRoute(_)
                | Message::SetTunStrictRoute(_)
                | Message::SetSnifferEnabled(_)
        ) && (self.runtime.mode_actions.pending.is_some()
            || self.runtime.pending_runtime_patch.is_some())
        {
            return Task::none();
        }
        match message {
            Message::SetProxyMode(_)
            | Message::ProxyModeFinished { .. }
            | Message::RetryProxyMode
            | Message::DismissProxyModeFailure => self.update_proxy_mode(message),
            other => self.update_core_lifecycle(other),
        }
    }
}
