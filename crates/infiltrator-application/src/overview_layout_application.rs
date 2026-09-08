//! Application seam for managing Overview page card order and drag-and-drop customization.

use infiltrator_contract::overview_layout::{OverviewCardKind, OverviewLayoutSnapshot};
use std::sync::{Arc, Mutex};

/// Thread-safe application state manager for Overview page layout order.
#[derive(Clone, Debug, Default)]
pub struct OverviewLayoutApplication {
    state: Arc<Mutex<OverviewLayoutSnapshot>>,
}

impl OverviewLayoutApplication {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OverviewLayoutSnapshot::default())),
        }
    }

    pub fn with_initial(snapshot: OverviewLayoutSnapshot) -> Self {
        Self {
            state: Arc::new(Mutex::new(snapshot)),
        }
    }

    pub fn snapshot(&self) -> OverviewLayoutSnapshot {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn move_up(&self, kind: OverviewCardKind) -> bool {
        self.state
            .lock()
            .map(|mut guard| guard.move_up(kind))
            .unwrap_or(false)
    }

    pub fn move_down(&self, kind: OverviewCardKind) -> bool {
        self.state
            .lock()
            .map(|mut guard| guard.move_down(kind))
            .unwrap_or(false)
    }

    pub fn reorder(&self, from: usize, to: usize) -> bool {
        self.state
            .lock()
            .map(|mut guard| guard.reorder(from, to))
            .unwrap_or(false)
    }

    pub fn set_order(&self, order: Vec<OverviewCardKind>) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = OverviewLayoutSnapshot::new(order);
        }
    }

    pub fn reset_to_default(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.reset_to_default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overview_layout_application_thread_safety_and_moves() {
        let app = OverviewLayoutApplication::new();
        let snap = app.snapshot();
        assert_eq!(snap.order.len(), 8);
        assert!(!snap.is_customized);

        assert!(app.move_down(OverviewCardKind::ModeSegment));
        let snap = app.snapshot();
        assert!(snap.is_customized);
        assert_eq!(snap.order[0], OverviewCardKind::Traffic);
        assert_eq!(snap.order[1], OverviewCardKind::ModeSegment);

        assert!(app.move_up(OverviewCardKind::ModeSegment));
        let snap = app.snapshot();
        assert!(!snap.is_customized);
        assert_eq!(snap.order[0], OverviewCardKind::ModeSegment);

        app.set_order(vec![OverviewCardKind::Quota, OverviewCardKind::Traffic]);
        let snap = app.snapshot();
        assert_eq!(snap.order.len(), 2);
        assert!(snap.is_customized);

        app.reset_to_default();
        assert_eq!(app.snapshot().order.len(), 8);
        assert!(!app.snapshot().is_customized);
    }
}
