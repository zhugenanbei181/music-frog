//! In-memory and persistent preference management for proxy strategy groups,
//! four-way sorting, alive filtering, favorite pinning, and drag-and-drop order.

use infiltrator_contract::proxies::{ProxySortOrder, ProxyUiPreferences};
use std::sync::{Arc, Mutex};

/// Thread-safe controller for proxy UI preferences shared by CLI, Desktop (Iced),
/// and Bevy surfaces.
#[derive(Clone, Debug, Default)]
pub struct ProxyPreferencesApplication {
    state: Arc<Mutex<ProxyUiPreferences>>,
}

impl ProxyPreferencesApplication {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProxyUiPreferences::default())),
        }
    }

    pub fn with_initial(prefs: ProxyUiPreferences) -> Self {
        Self {
            state: Arc::new(Mutex::new(prefs)),
        }
    }

    pub fn preferences(&self) -> ProxyUiPreferences {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn is_group_expanded(&self, group: &str) -> bool {
        self.state
            .lock()
            .map(|guard| guard.is_group_expanded(group))
            .unwrap_or(true)
    }

    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.state
            .lock()
            .map(|guard| guard.is_group_collapsed(group))
            .unwrap_or(false)
    }

    /// Toggle a group's expanded/collapsed state. Returns true if now expanded.
    pub fn toggle_group_expand(&self, group: &str) -> bool {
        self.state
            .lock()
            .map(|mut guard| guard.toggle_group_expand(group))
            .unwrap_or(true)
    }

    pub fn set_group_expanded(&self, group: &str, expanded: bool) {
        if let Ok(mut guard) = self.state.lock() {
            guard.set_group_expanded(group, expanded);
        }
    }

    pub fn expand_all(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.expand_all();
        }
    }

    pub fn collapse_all(&self, groups: &[String]) {
        if let Ok(mut guard) = self.state.lock() {
            guard.collapse_all(groups);
        }
    }

    pub fn sort_order(&self) -> ProxySortOrder {
        self.state
            .lock()
            .map(|guard| guard.sort_order)
            .unwrap_or_default()
    }

    pub fn set_sort_order(&self, order: ProxySortOrder) {
        if let Ok(mut guard) = self.state.lock() {
            guard.set_sort_order(order);
        }
    }

    pub fn filter_alive(&self) -> bool {
        self.state
            .lock()
            .map(|guard| guard.filter_alive)
            .unwrap_or(false)
    }

    pub fn toggle_filter_alive(&self) -> bool {
        self.state
            .lock()
            .map(|mut guard| {
                let next = !guard.filter_alive;
                guard.set_filter_alive(next);
                next
            })
            .unwrap_or(false)
    }

    pub fn set_filter_alive(&self, enabled: bool) {
        if let Ok(mut guard) = self.state.lock() {
            guard.set_filter_alive(enabled);
        }
    }

    pub fn is_favorite(&self, proxy: &str) -> bool {
        self.state
            .lock()
            .map(|guard| guard.is_favorite(proxy))
            .unwrap_or(false)
    }

    /// Toggle favorite status. Returns true if now favorite.
    pub fn toggle_favorite(&self, proxy: &str) -> bool {
        self.state
            .lock()
            .map(|mut guard| guard.toggle_favorite(proxy))
            .unwrap_or(false)
    }

    pub fn set_compact_view(&self, compact: bool) {
        if let Ok(mut guard) = self.state.lock() {
            guard.set_compact_view(compact);
        }
    }

    pub fn compact_view(&self) -> bool {
        self.state
            .lock()
            .map(|guard| guard.compact_view)
            .unwrap_or(false)
    }

    pub fn reorder_groups(&self, group_names: Vec<String>) {
        if let Ok(mut guard) = self.state.lock() {
            guard.reorder_groups(group_names);
        }
    }

    pub fn reset_group_order(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.reset_group_order();
        }
    }

    pub fn custom_group_order(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|guard| guard.custom_group_order.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_application_thread_safety_and_toggles() {
        let app = ProxyPreferencesApplication::new();
        assert!(app.is_group_expanded("Group-A"));
        assert!(!app.filter_alive());
        assert_eq!(app.sort_order(), ProxySortOrder::LatencyAsc);

        // Toggle expand
        let is_exp = app.toggle_group_expand("Group-A");
        assert!(!is_exp);
        assert!(!app.is_group_expanded("Group-A"));
        assert!(app.is_group_collapsed("Group-A"));

        // Toggle filter alive
        assert!(app.toggle_filter_alive());
        assert!(app.filter_alive());
        assert!(!app.toggle_filter_alive());
        assert!(!app.filter_alive());

        // Favorite toggle
        assert!(!app.is_favorite("Node-1"));
        assert!(app.toggle_favorite("Node-1"));
        assert!(app.is_favorite("Node-1"));
        assert!(!app.toggle_favorite("Node-1"));
        assert!(!app.is_favorite("Node-1"));

        // Reorder
        app.reorder_groups(vec!["Group-B".into(), "Group-A".into()]);
        assert_eq!(app.custom_group_order(), vec!["Group-B", "Group-A"]);
        app.reset_group_order();
        assert!(app.custom_group_order().is_empty());
    }
}
