//! `update::core` — the runtime message core, split by business domain.
//!
//! [`AppState::update_core`] is the single entry point used by the
//! dispatcher in `src/update.rs`; its signature and path
//! (`crate::update::core`) are stable. Each domain submodule owns one slice
//! of the original message match and forwards unmatched messages to the
//! next domain in the chain:
//! lifecycle → settings → monitoring → proxies → runtime_config → rules →
//! advanced → dns_config → tun_config → rebuild → kernels (fallback).
mod advanced;
mod dns_config;
mod kernels;
mod lifecycle;
mod monitoring;
mod proxies;
mod rebuild;
mod rules;
mod runtime_config;
mod settings;
mod tun_config;

use crate::state::AppState;
use crate::types::Message;
use iced::Task;

impl AppState {
    /// Public entry used by the dispatcher in `src/update.rs`.
    pub fn update_core(&mut self, message: Message) -> Task<Message> {
        self.update_core_lifecycle(message)
    }
}
