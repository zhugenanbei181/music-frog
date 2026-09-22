//! Iced toast queue over the shared notification policy.
//!
//! Severity vocabulary, the dedup window and the visible cap come from
//! `infiltrator_contract::toast`; this module only owns the per-surface
//! timers and the visible order. Text redaction stays at the single toast
//! ingestion point (`AppState::push_toast` → `crate::utils::sanitize_ui_text`).

use infiltrator_contract::toast::{ToastAdmission, ToastGate, ToastPolicy, ToastSeverity};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    /// Map onto the shared severity vocabulary both surfaces agree on.
    pub const fn severity(&self) -> ToastSeverity {
        match self {
            Self::Info => ToastSeverity::Info,
            Self::Success => ToastSeverity::Success,
            Self::Warning => ToastSeverity::Warning,
            Self::Error => ToastSeverity::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ToastItem {
    pub id: u64,
    pub level: ToastLevel,
    pub message: String,
    pub duration_ms: u64,
    pub remaining_ms: u64,
}

pub struct ToastManager {
    policy: ToastPolicy,
    gate: ToastGate,
    now_ms: u64,
    active_toasts: Vec<ToastItem>,
    next_id: u64,
}

impl ToastManager {
    pub fn new(max_visible: usize) -> Self {
        let policy = ToastPolicy {
            max_visible,
            ..ToastPolicy::default()
        };
        Self {
            policy,
            gate: ToastGate::new(policy),
            now_ms: 0,
            active_toasts: Vec::new(),
            next_id: 1,
        }
    }

    pub const fn policy(&self) -> ToastPolicy {
        self.policy
    }

    /// Offer a toast. An identical `(level, message)` inside the shared dedup
    /// window refreshes the live toast instead of stacking a duplicate.
    pub fn push(&mut self, level: ToastLevel, message: String, duration_ms: u64) {
        if self.gate.admit(level.severity(), &message, self.now_ms) == ToastAdmission::Coalesced {
            if let Some(existing) = self
                .active_toasts
                .iter_mut()
                .rev()
                .find(|toast| toast.level == level && toast.message == message)
            {
                existing.duration_ms = duration_ms;
                existing.remaining_ms = duration_ms;
            }
            return;
        }

        let item = ToastItem {
            id: self.next_id,
            level,
            message,
            duration_ms,
            remaining_ms: duration_ms,
        };
        self.next_id += 1;

        self.active_toasts.push(item);

        while self.active_toasts.len() > self.policy.max_visible {
            self.active_toasts.remove(0);
        }
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
        for toast in &mut self.active_toasts {
            toast.remaining_ms = toast.remaining_ms.saturating_sub(elapsed_ms);
        }
        self.active_toasts.retain(|t| t.remaining_ms > 0);
    }

    pub fn dismiss(&mut self, id: u64) {
        self.active_toasts.retain(|t| t.id != id);
    }

    pub fn active_toasts(&self) -> &[ToastItem] {
        &self.active_toasts
    }
}

#[cfg(test)]
#[path = "../tests/gui/toast_state_tests.rs"]
mod tests;
