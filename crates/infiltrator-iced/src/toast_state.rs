use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
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
    max_visible: usize,
    active_toasts: Vec<ToastItem>,
    next_id: u64,
}

impl ToastManager {
    pub fn new(max_visible: usize) -> Self {
        Self {
            max_visible,
            active_toasts: Vec::new(),
            next_id: 1,
        }
    }

    pub fn push(&mut self, level: ToastLevel, message: String, duration_ms: u64) {
        for toast in self.active_toasts.iter_mut().rev() {
            if toast.level == level && toast.message == message {
                let elapsed = toast.duration_ms.saturating_sub(toast.remaining_ms);
                if elapsed <= 2000 {
                    toast.remaining_ms = duration_ms;
                    toast.duration_ms = duration_ms;
                    return;
                }
            }
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

        if self.active_toasts.len() > self.max_visible {
            self.active_toasts.remove(0);
        }
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
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
