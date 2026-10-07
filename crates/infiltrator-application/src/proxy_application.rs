//! Runtime proxy use-cases over the controller port with full 5-group classification,
//! instant write-back, four-dimensional sorting, alive filtering, and preferences.
//!
//! Track 2 / Group 04: Proxies & Sorting.

use crate::proxy_preferences_application::ProxyPreferencesApplication;
use futures_util::stream;
use futures_util::stream::StreamExt;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxies::ProxyGroupClassification;
use infiltrator_domain::proxy::{ProxyGroup, ProxyHistory};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::Arc;

/// A controller-neutral leaf proxy projection.
#[derive(Debug, Clone, PartialEq)]
pub struct ProxyNode {
    pub name: String,
    pub proxy_type: String,
    pub udp: Option<bool>,
    pub history: Vec<ProxyHistory>,
    pub delay: Option<u32>,
    pub alive: Option<bool>,
}

/// Detailed proxy strategy group projection with typed classification.
#[derive(Debug, Clone, PartialEq)]
pub struct ProxyGroupDetail {
    pub name: String,
    pub group_type: String,
    pub classification: ProxyGroupClassification,
    pub now: String,
    pub all: Vec<String>,
    pub history: Vec<ProxyHistory>,
}

/// Proxy use-cases shared by CLI, Admin, and UI surfaces.
#[derive(Clone)]
pub struct ProxyApplication {
    gateway: Arc<dyn RuntimeGateway>,
    preferences: ProxyPreferencesApplication,
}

impl ProxyApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            preferences: ProxyPreferencesApplication::new(),
        }
    }

    pub fn with_preferences(
        gateway: Arc<dyn RuntimeGateway>,
        preferences: ProxyPreferencesApplication,
    ) -> Self {
        Self {
            gateway,
            preferences,
        }
    }

    pub fn preferences(&self) -> &ProxyPreferencesApplication {
        &self.preferences
    }

    pub async fn list_nodes(&self) -> Result<Vec<ProxyNode>, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let mut nodes = proxies
            .into_iter()
            .filter_map(|(name, proxy)| {
                (!proxy.is_group()).then(|| ProxyNode {
                    name,
                    proxy_type: proxy.proxy_type().to_string(),
                    udp: proxy.udp_observation(),
                    history: proxy.history().to_vec(),
                    delay: proxy.delay(),
                    alive: proxy.health_observation(),
                })
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(nodes)
    }

    pub async fn list_groups(&self) -> Result<Vec<ProxyGroup>, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let mut groups = proxies
            .into_iter()
            .filter_map(|(name, proxy)| {
                proxy.is_group().then(|| ProxyGroup {
                    name,
                    now: proxy.now().unwrap_or_default().to_string(),
                    all: proxy.all().unwrap_or_default().to_vec(),
                    history: proxy.history().to_vec(),
                })
            })
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(groups)
    }

    pub async fn list_group_details(&self) -> Result<Vec<ProxyGroupDetail>, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let mut groups = proxies
            .into_iter()
            .filter_map(|(name, proxy)| {
                if !proxy.is_group() {
                    return None;
                }
                let raw_type = proxy.proxy_type();
                let classification = ProxyGroupClassification::from_str_loose(raw_type)
                    .unwrap_or(ProxyGroupClassification::Selector);
                Some(ProxyGroupDetail {
                    name,
                    group_type: raw_type.to_string(),
                    classification,
                    now: proxy.now().unwrap_or_default().to_string(),
                    all: proxy.all().unwrap_or_default().to_vec(),
                    history: proxy.history().to_vec(),
                })
            })
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(groups)
    }

    /// Immediate node selection write-back via `PUT /proxies/{group}`.
    ///
    /// Validates that:
    /// 1. The target group exists.
    /// 2. The target group is a recognized proxy group.
    /// 3. The group supports manual selection (Selector).
    /// 4. The requested proxy is a member of the group.
    pub async fn switch(&self, group: &str, proxy: &str) -> Result<(), Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let target_group = proxies.get(group).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("proxy group {group} was not found"),
                false,
            )
        })?;

        if !target_group.is_group() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("{group} is not a proxy group"),
                false,
            ));
        }

        let classification = ProxyGroupClassification::from_str_loose(target_group.proxy_type())
            .unwrap_or(ProxyGroupClassification::Selector);
        if !classification.is_manual_selectable() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!(
                    "proxy group {group} is of type {} and cannot be manually switched",
                    classification.as_str()
                ),
                false,
            ));
        }

        if let Some(members) = target_group.all()
            && !members.iter().any(|m| m == proxy)
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("proxy {proxy} is not a member of group {group}"),
                false,
            ));
        }

        self.gateway
            .switch_proxy(group, proxy)
            .await
            .map_err(Failure::from)
    }

    pub async fn current(&self, group: &str) -> Result<String, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let proxy = proxies.get(group).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("proxy group {group} was not found"),
                false,
            )
        })?;
        if !proxy.is_group() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("{group} is not a proxy group"),
                false,
            ));
        }
        Ok(proxy.now().unwrap_or_default().to_string())
    }

    pub async fn test_delay(
        &self,
        proxy: &str,
        url: &str,
        timeout_ms: u32,
    ) -> Result<u32, Failure> {
        self.gateway
            .test_delay(proxy, url, timeout_ms)
            .await
            .map_err(Failure::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyDelayOutcome {
    pub proxy_name: String,
    pub result: Result<u32, Failure>,
}

/// Test a bounded set of proxies concurrently through the runtime gateway.
pub async fn test_proxy_delays<G: RuntimeGateway + ?Sized>(
    gateway: Arc<G>,
    proxies: Vec<String>,
    test_url: String,
    timeout_ms: u32,
    max_concurrency: usize,
) -> Vec<ProxyDelayOutcome> {
    stream::iter(proxies.into_iter().map(|proxy_name| {
        let gateway = Arc::clone(&gateway);
        let test_url = test_url.clone();
        async move {
            let result = gateway
                .test_delay(&proxy_name, &test_url, timeout_ms)
                .await
                .map_err(Failure::from);
            ProxyDelayOutcome { proxy_name, result }
        }
    }))
    .buffer_unordered(max_concurrency.max(1))
    .collect()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy_projection::project_groups_snapshot;
    use async_trait::async_trait;
    #[cfg(test)]
    use infiltrator_contract::command::ProxyMode;
    use infiltrator_contract::proxies::ProxySortOrder;
    use infiltrator_contract::proxies::ProxyUiPreferences;
    use infiltrator_domain::proxy::Proxy;
    use infiltrator_domain::proxy::{ProxyBase, Shadowsocks};
    use infiltrator_domain::proxy_observation::RuntimeProxyObservation;
    use infiltrator_domain::runtime::{
        ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
    };
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::runtime_gateway::RuntimeStream;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockGateway {
        proxies: Mutex<HashMap<String, Proxy>>,
        switches: Mutex<Vec<(String, String)>>,
    }

    impl MockGateway {
        fn new(proxies: HashMap<String, Proxy>) -> Self {
            Self {
                proxies: Mutex::new(proxies),
                switches: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl RuntimeGateway for MockGateway {
        async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
            Ok(self.proxies.lock().unwrap().clone())
        }

        async fn switch_proxy(&self, group: &str, proxy: &str) -> Result<(), PortError> {
            self.switches
                .lock()
                .unwrap()
                .push((group.to_string(), proxy.to_string()));
            Ok(())
        }

        async fn test_delay(
            &self,
            _proxy: &str,
            _url: &str,
            _timeout_ms: u32,
        ) -> Result<u32, PortError> {
            Ok(50)
        }

        async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
            Err(PortError::Failed("not implemented".into()))
        }
        async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
            Ok(())
        }
        async fn set_proxy_mode(&self, _mode: ProxyMode) -> Result<(), PortError> {
            Ok(())
        }
        async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
            Err(PortError::Failed("not implemented".into()))
        }
        async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn close_all_connections(&self) -> Result<(), PortError> {
            Ok(())
        }
        async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
            Err(PortError::Failed("not implemented".into()))
        }
        async fn stream_logs(
            &self,
            _level: Option<String>,
        ) -> Result<RuntimeStream<String>, PortError> {
            Err(PortError::Failed("not implemented".into()))
        }
        async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
            Err(PortError::Failed("not implemented".into()))
        }
        async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
            Ok(())
        }
        async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
            Ok(Vec::new())
        }
        async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
            Ok(Vec::new())
        }
        async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn trigger_gc(&self) -> Result<(), PortError> {
            Ok(())
        }
        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Err(PortError::Failed("not implemented".into()))
        }
    }

    #[tokio::test]
    async fn runtime_node_listing_preserves_unknown_flags_and_reported_zero_observations() {
        let facts: RuntimeProxyObservation = serde_json::from_str(
            r#"{"name":"leaf","type":"Shadowsocks","history":[{"time":"last","delay":0}]}"#,
        )
        .unwrap();
        let gateway = Arc::new(MockGateway::new(HashMap::from([(
            "leaf".into(),
            Proxy::Observed(facts),
        )])));
        let nodes = ProxyApplication::new(gateway).list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "leaf");
        assert_eq!(nodes[0].proxy_type, "Shadowsocks");
        assert_eq!(nodes[0].alive, None);
        assert_eq!(nodes[0].udp, None);
        assert_eq!(nodes[0].delay, Some(0));
    }

    fn sample_proxies() -> HashMap<String, Proxy> {
        let mut map = HashMap::new();

        map.insert(
            "Node-A".to_string(),
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name: "Node-A".to_string(),
                    udp: true,
                    history: vec![ProxyHistory {
                        time: "".into(),
                        delay: 100,
                    }],
                    alive: true,
                    delay: Some(100),
                },
                server: "1.1.1.1".into(),
                port: 443,
                cipher: "aes-256-gcm".into(),
                plugin: None,
                plugin_opts: None,
            }),
        );

        map.insert(
            "Node-B".to_string(),
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name: "Node-B".to_string(),
                    udp: false,
                    history: vec![ProxyHistory {
                        time: "".into(),
                        delay: 35,
                    }],
                    alive: true,
                    delay: Some(35),
                },
                server: "2.2.2.2".into(),
                port: 443,
                cipher: "aes-256-gcm".into(),
                plugin: None,
                plugin_opts: None,
            }),
        );

        map.insert(
            "Node-Dead".to_string(),
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name: "Node-Dead".to_string(),
                    udp: false,
                    history: vec![],
                    alive: false,
                    delay: None,
                },
                server: "3.3.3.3".into(),
                port: 443,
                cipher: "aes-256-gcm".into(),
                plugin: None,
                plugin_opts: None,
            }),
        );

        map.insert(
            "SelectorGroup".to_string(),
            Proxy::Selector(ProxyGroup {
                name: "SelectorGroup".to_string(),
                now: "Node-A".to_string(),
                all: vec!["Node-A".into(), "Node-B".into(), "Node-Dead".into()],
                history: vec![],
            }),
        );

        map.insert(
            "RelayGroup".to_string(),
            Proxy::Relay(ProxyGroup {
                name: "RelayGroup".to_string(),
                now: "Node-B".to_string(),
                all: vec!["Node-A".into(), "Node-B".into()],
                history: vec![],
            }),
        );

        map
    }

    #[tokio::test]
    async fn test_list_group_details_five_classifications() {
        let gateway = Arc::new(MockGateway::new(sample_proxies()));
        let app = ProxyApplication::new(gateway);

        let groups = app.list_group_details().await.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].classification, ProxyGroupClassification::Relay);
        assert_eq!(groups[1].classification, ProxyGroupClassification::Selector);
    }

    #[tokio::test]
    async fn test_switch_proxy_validations() {
        let gateway = Arc::new(MockGateway::new(sample_proxies()));
        let app = ProxyApplication::new(gateway.clone());

        // 1. Selector group switch succeeds
        assert!(app.switch("SelectorGroup", "Node-B").await.is_ok());
        let switches = gateway.switches.lock().unwrap().clone();
        assert_eq!(
            switches,
            vec![("SelectorGroup".to_string(), "Node-B".to_string())]
        );

        // 2. Switching non-existent group fails
        assert!(app.switch("NonExistent", "Node-A").await.is_err());

        // 3. Switching non-member fails
        assert!(app.switch("SelectorGroup", "InvalidNode").await.is_err());

        // 4. Switching Relay group (automated) fails manual selection
        assert!(app.switch("RelayGroup", "Node-A").await.is_err());
    }

    #[test]
    fn test_project_groups_snapshot_sorting_and_alive_filtering() {
        let proxies = sample_proxies();
        let mut prefs = ProxyUiPreferences::default();

        // 1. Default: LatencyAsc, FilterAlive off
        let (groups, alive) = project_groups_snapshot(&proxies, &prefs);
        assert_eq!(alive.total_nodes, 3);
        assert_eq!(alive.alive_nodes, 2);
        assert_eq!(alive.dead_nodes, 1);

        let selector = groups.iter().find(|g| g.name == "SelectorGroup").unwrap();
        assert_eq!(selector.proxies.len(), 3);
        assert_eq!(selector.proxies[0].name, "Node-B"); // 35ms lowest
        assert_eq!(selector.proxies[1].name, "Node-A"); // 100ms
        assert_eq!(selector.proxies[2].name, "Node-Dead"); // None at bottom

        // 2. FilterAlive on -> Dead node eliminated
        prefs.filter_alive = true;
        let (groups_alive, _) = project_groups_snapshot(&proxies, &prefs);
        let selector_alive = groups_alive
            .iter()
            .find(|g| g.name == "SelectorGroup")
            .unwrap();
        assert_eq!(selector_alive.proxies.len(), 2);
        assert!(!selector_alive.proxies.iter().any(|n| n.name == "Node-Dead"));

        // 3. Favorite pinning beats lower latency
        prefs.filter_alive = false;
        prefs.favorite_proxies.push("Node-A".to_string()); // 100ms pinned
        let (groups_fav, _) = project_groups_snapshot(&proxies, &prefs);
        let selector_fav = groups_fav
            .iter()
            .find(|g| g.name == "SelectorGroup")
            .unwrap();
        assert_eq!(selector_fav.proxies[0].name, "Node-A"); // Pinned first!
        assert_eq!(selector_fav.proxies[1].name, "Node-B");

        // 4. Sort NameDesc
        prefs.favorite_proxies.clear();
        prefs.sort_order = ProxySortOrder::NameDesc;
        let (groups_desc, _) = project_groups_snapshot(&proxies, &prefs);
        let selector_desc = groups_desc
            .iter()
            .find(|g| g.name == "SelectorGroup")
            .unwrap();
        assert_eq!(selector_desc.proxies[0].name, "Node-Dead");
        assert_eq!(selector_desc.proxies[1].name, "Node-B");
        assert_eq!(selector_desc.proxies[2].name, "Node-A");
    }
}
