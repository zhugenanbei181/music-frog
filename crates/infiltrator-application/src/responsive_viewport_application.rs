//! Application seam for managing responsive viewport dimensions and tier projection.

use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
use std::sync::{Arc, Mutex};

/// Service calculating and providing the active responsive viewport snapshot.
#[derive(Clone, Debug, Default)]
pub struct ResponsiveViewportApplication {
    state: Arc<Mutex<ResponsiveViewportSnapshot>>,
}

impl ResponsiveViewportApplication {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ResponsiveViewportSnapshot::default())),
        }
    }

    pub fn snapshot(&self) -> ResponsiveViewportSnapshot {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn update_dimensions(&self, width_px: f32, height_px: f32) -> ResponsiveViewportSnapshot {
        let next = ResponsiveViewportSnapshot::from_dimensions(width_px, height_px);
        if let Ok(mut guard) = self.state.lock() {
            *guard = next.clone();
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::responsive_viewport::ViewportTier;

    #[test]
    fn test_responsive_viewport_application_updates() {
        let app = ResponsiveViewportApplication::new();
        assert_eq!(app.snapshot().tier, ViewportTier::Expanded);

        let snap = app.update_dimensions(400.0, 800.0);
        assert_eq!(snap.tier, ViewportTier::Compact);
        assert_eq!(app.snapshot().tier, ViewportTier::Compact);
    }
}
