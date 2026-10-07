//! Shared in-memory preference management for proxy strategy groups,
//! four-way sorting, alive filtering, favorite pinning, and drag-and-drop order.

#[cfg(test)]
use crate::proxy_projection::project_proxy_groups;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxies::{ProxySortOrder, ProxyUiPreferences};
#[cfg(test)]
use infiltrator_contract::surface_snapshot::ProxyGroupSnapshot;
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

    fn read<T>(&self, read: impl FnOnce(&ProxyUiPreferences) -> T) -> Result<T, Failure> {
        self.state
            .lock()
            .map(|state| read(&state))
            .map_err(|_| Self::unavailable())
    }
    fn update<T>(&self, change: impl FnOnce(&mut ProxyUiPreferences) -> T) -> Result<T, Failure> {
        self.state
            .lock()
            .map(|mut state| change(&mut state))
            .map_err(|_| Self::unavailable())
    }
    fn unavailable() -> Failure {
        Failure::new(
            ErrorCode::InvalidState,
            "proxy preference state is poisoned",
            false,
        )
    }
    pub fn preferences(&self) -> Result<ProxyUiPreferences, Failure> {
        self.read(Clone::clone)
    }
    pub fn is_group_expanded(&self, group: &str) -> Result<bool, Failure> {
        self.read(|state| state.is_group_expanded(group))
    }
    pub fn is_group_collapsed(&self, group: &str) -> Result<bool, Failure> {
        self.read(|state| state.is_group_collapsed(group))
    }
    pub fn toggle_group_expand(&self, group: &str) -> Result<bool, Failure> {
        self.update(|state| state.toggle_group_expand(group))
    }
    pub fn set_group_expanded(&self, group: &str, expanded: bool) -> Result<(), Failure> {
        self.update(|state| state.set_group_expanded(group, expanded))
    }
    pub fn expand_all(&self) -> Result<(), Failure> {
        self.update(ProxyUiPreferences::expand_all)
    }
    pub fn collapse_all(&self, groups: &[String]) -> Result<(), Failure> {
        self.update(|state| state.collapse_all(groups))
    }
    pub fn sort_order(&self) -> Result<ProxySortOrder, Failure> {
        self.read(|state| state.sort_order)
    }
    pub fn set_sort_order(&self, order: ProxySortOrder) -> Result<(), Failure> {
        self.update(|state| state.set_sort_order(order))
    }
    pub fn set_search_query(&self, query: String) -> Result<(), Failure> {
        self.update(|state| state.search_query = query)
    }
    pub fn filter_alive(&self) -> Result<bool, Failure> {
        self.read(|state| state.filter_alive)
    }
    pub fn toggle_filter_alive(&self) -> Result<bool, Failure> {
        self.update(|state| {
            let enabled = !state.filter_alive;
            state.set_filter_alive(enabled);
            enabled
        })
    }
    pub fn set_filter_alive(&self, enabled: bool) -> Result<(), Failure> {
        self.update(|state| state.set_filter_alive(enabled))
    }
    pub fn is_favorite(&self, proxy: &str) -> Result<bool, Failure> {
        self.read(|state| state.is_favorite(proxy))
    }
    pub fn toggle_favorite(&self, proxy: &str) -> Result<bool, Failure> {
        self.update(|state| state.toggle_favorite(proxy))
    }
    pub fn set_compact_view(&self, compact: bool) -> Result<(), Failure> {
        self.update(|state| state.set_compact_view(compact))
    }
    pub fn compact_view(&self) -> Result<bool, Failure> {
        self.read(|state| state.compact_view)
    }
    pub fn reorder_groups(&self, group_names: Vec<String>) -> Result<(), Failure> {
        self.update(|state| state.reorder_groups(group_names))
    }
    pub fn reset_group_order(&self) -> Result<(), Failure> {
        self.update(ProxyUiPreferences::reset_group_order)
    }
    pub fn custom_group_order(&self) -> Result<Vec<String>, Failure> {
        self.read(|state| state.custom_group_order.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_application_thread_safety_and_toggles() {
        let app = ProxyPreferencesApplication::new();
        assert!(app.is_group_expanded("Group-A").unwrap());
        assert!(!app.filter_alive().unwrap());
        assert_eq!(app.sort_order().unwrap(), ProxySortOrder::LatencyAsc);

        // Toggle expand
        let is_exp = app.toggle_group_expand("Group-A").unwrap();
        assert!(!is_exp);
        assert!(!app.is_group_expanded("Group-A").unwrap());
        assert!(app.is_group_collapsed("Group-A").unwrap());

        // Toggle filter alive
        assert!(app.toggle_filter_alive().unwrap());
        assert!(app.filter_alive().unwrap());
        assert!(!app.toggle_filter_alive().unwrap());
        assert!(!app.filter_alive().unwrap());

        // Favorite toggle
        assert!(!app.is_favorite("Node-1").unwrap());
        assert!(app.toggle_favorite("Node-1").unwrap());
        assert!(app.is_favorite("Node-1").unwrap());
        assert!(!app.toggle_favorite("Node-1").unwrap());
        assert!(!app.is_favorite("Node-1").unwrap());

        // Reorder
        app.reorder_groups(vec!["Group-B".into(), "Group-A".into()])
            .unwrap();
        assert_eq!(
            app.custom_group_order().unwrap(),
            vec!["Group-B", "Group-A"]
        );
        app.reset_group_order().unwrap();
        assert!(app.custom_group_order().unwrap().is_empty());
    }

    #[tokio::test]
    async fn preference_commands_refuse_missing_owner_and_publish_real_shared_changes() {
        use crate::command_application::{CommandApplication, CommandHandler};
        use infiltrator_contract::command::CommandIntent;
        use infiltrator_contract::error::ErrorCode;

        let commands = [
            CommandIntent::SetProxySearchQuery { query: "ss".into() },
            CommandIntent::ToggleProxyGroupExpand {
                group: "Group-A".into(),
            },
            CommandIntent::SetProxyGroupExpanded {
                group: "Group-A".into(),
                expanded: true,
            },
            CommandIntent::SetProxySortOrder {
                order: ProxySortOrder::NameDesc,
            },
            CommandIntent::ToggleFilterAlive { enabled: true },
            CommandIntent::ToggleFavoriteProxy {
                proxy: "Node-A".into(),
            },
            CommandIntent::SetProxyCompactView { compact: true },
            CommandIntent::ReorderProxyGroups {
                group_names: vec!["Group-B".into(), "Group-A".into()],
            },
            CommandIntent::ResetProxyGroupOrder,
        ];
        for command in &commands {
            let failure = CommandApplication::new()
                .handle(command.clone())
                .await
                .unwrap_err();
            assert_eq!(failure.code, ErrorCode::NotReady);
            assert!(failure.message.contains("proxy preferences application"));
        }
        let preferences = ProxyPreferencesApplication::new();
        let application = CommandApplication::new().with_proxy_preferences(preferences.clone());
        for command in commands {
            application.handle(command).await.unwrap();
        }
        let result = preferences.preferences().unwrap();
        assert_eq!(result.search_query, "ss");
        assert!(result.is_group_expanded("Group-A"));
        assert_eq!(result.sort_order, ProxySortOrder::NameDesc);
        assert!(result.filter_alive);
        assert!(result.is_favorite("Node-A"));
        assert!(result.compact_view);
        assert!(result.custom_group_order.is_empty());
    }

    #[test]
    fn projection_replays_group_identity_and_preserves_selection_through_sort_and_filter() {
        use infiltrator_contract::surface_snapshot::ProxyNodeSnapshot;
        let make_group = |name: &str| ProxyGroupSnapshot {
            name: name.into(),
            group_type: "Selector".into(),
            classification: None,
            current: "Node-B".into(),
            expanded: true,
            proxies: vec![
                ProxyNodeSnapshot {
                    name: "Node-A".into(),
                    node_type: "VLESS".into(),
                    delay_ms: Some(70),
                    alive: Some(true),
                    selected: false,
                    favorite: false,
                    features: vec![],
                },
                ProxyNodeSnapshot {
                    name: "Node-B".into(),
                    node_type: "Trojan".into(),
                    delay_ms: Some(30),
                    alive: Some(true),
                    selected: true,
                    favorite: false,
                    features: vec![],
                },
                ProxyNodeSnapshot {
                    name: "Node-Dead".into(),
                    node_type: "VLESS".into(),
                    delay_ms: Some(0),
                    alive: Some(false),
                    selected: false,
                    favorite: false,
                    features: vec![],
                },
            ],
        };
        let input = vec![make_group("Group-A"), make_group("Group-B")];
        let preferences = ProxyPreferencesApplication::new();
        preferences.set_group_expanded("Group-B", false).unwrap();
        preferences
            .reorder_groups(vec!["Group-B".into(), "removed-group".into()])
            .unwrap();
        preferences.toggle_favorite("Node-A").unwrap();
        preferences.set_filter_alive(true).unwrap();
        let mut changed = input.clone();
        changed.push(make_group("newly-published-group"));
        let changed = project_proxy_groups(changed, &preferences.preferences().unwrap());
        assert!(
            changed
                .iter()
                .find(|group| group.name == "newly-published-group")
                .unwrap()
                .expanded
        );
        assert!(
            !changed
                .iter()
                .find(|group| group.name == "Group-B")
                .unwrap()
                .expanded
        );
        let output = project_proxy_groups(input.clone(), &preferences.preferences().unwrap());
        assert_eq!(
            output
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Group-B", "Group-A"]
        );
        assert!(!output[0].expanded);
        assert!(output[1].expanded);
        for group in output {
            assert_eq!(group.current, "Node-B");
            assert_eq!(
                group
                    .proxies
                    .iter()
                    .map(|node| node.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Node-A", "Node-B"]
            );
            assert!(group.proxies[0].favorite);
            assert!(!group.proxies[0].selected);
            assert!(group.proxies[1].selected);
        }
        preferences.set_group_expanded("Group-B", true).unwrap();
        preferences.set_filter_alive(false).unwrap();
        preferences.reset_group_order().unwrap();
        let restored = project_proxy_groups(input, &preferences.preferences().unwrap());
        assert_eq!(restored[0].name, "Group-A");
        assert!(
            restored
                .iter()
                .all(|group| group.expanded && group.proxies.len() == 3)
        );
    }
    #[tokio::test]
    async fn poisoned_preferences_refuse_reads_and_all_commands_instead_of_reporting_default_or_success()
     {
        use crate::command_application::{CommandApplication, CommandHandler};
        use infiltrator_contract::command::CommandIntent;
        use std::thread::spawn;
        let preferences = ProxyPreferencesApplication::new();
        preferences
            .set_search_query("retained query".into())
            .unwrap();
        let state = preferences.state.clone();
        assert!(
            spawn(move || {
                let _guard = state.lock().unwrap();
                panic!("fixture poisons the preference lock");
            })
            .join()
            .is_err()
        );
        assert_eq!(
            preferences.preferences().unwrap_err().code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            preferences.is_group_expanded("Group-A").unwrap_err().code,
            ErrorCode::InvalidState
        );
        let commands = CommandApplication::new().with_proxy_preferences(preferences.clone());
        for intent in [
            CommandIntent::SetProxySearchQuery {
                query: "lost query".into(),
            },
            CommandIntent::SetProxySortOrder {
                order: ProxySortOrder::NameAsc,
            },
            CommandIntent::ToggleFavoriteProxy {
                proxy: "Node-A".into(),
            },
            CommandIntent::ToggleProxyGroupExpand {
                group: "Group-A".into(),
            },
            CommandIntent::SetProxyGroupExpanded {
                group: "Group-A".into(),
                expanded: false,
            },
            CommandIntent::ToggleFilterAlive { enabled: true },
            CommandIntent::SetProxyCompactView { compact: true },
            CommandIntent::ReorderProxyGroups {
                group_names: vec!["Group-A".into()],
            },
            CommandIntent::ResetProxyGroupOrder,
        ] {
            let failure = commands.handle(intent).await.unwrap_err();
            assert_eq!(failure.code, ErrorCode::InvalidState);
            assert!(!failure.retryable);
        }
    }
}
