//! Focused unit tests for the shared speedtest application engine.

use super::*;
use async_trait::async_trait;
use infiltrator_contract::speedtest::EgressCountryMatch;
use infiltrator_domain::proxy::{ProxyBase, ProxyGroup, Shadowsocks, Vmess};
use infiltrator_domain::runtime::{
    ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

type DelayFn = dyn Fn(&str, &str, u32) -> Result<u32, PortError> + Send + Sync;

struct TestGateway {
    proxies: HashMap<String, Proxy>,
    delay_probe_count: AtomicUsize,
    active_concurrency: AtomicUsize,
    max_concurrency: AtomicUsize,
    delay_fn: Box<DelayFn>,
}

impl TestGateway {
    fn new(proxies: HashMap<String, Proxy>) -> Self {
        Self {
            proxies,
            delay_probe_count: AtomicUsize::new(0),
            active_concurrency: AtomicUsize::new(0),
            max_concurrency: AtomicUsize::new(0),
            delay_fn: Box::new(|_proxy, _url, _timeout| Ok(45)),
        }
    }

    fn with_delay_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&str, &str, u32) -> Result<u32, PortError> + Send + Sync + 'static,
    {
        self.delay_fn = Box::new(f);
        self
    }
}

#[async_trait]
impl RuntimeGateway for TestGateway {
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
        Ok(ConfigSnapshot::default())
    }
    async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
        Ok(())
    }
    async fn set_proxy_mode(
        &self,
        _mode: infiltrator_contract::command::ProxyMode,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
        Ok(self.proxies.clone())
    }
    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError> {
        self.delay_probe_count.fetch_add(1, Ordering::SeqCst);
        let current = self.active_concurrency.fetch_add(1, Ordering::SeqCst) + 1;
        let mut max = self.max_concurrency.load(Ordering::SeqCst);
        while current > max {
            match self.max_concurrency.compare_exchange_weak(
                max,
                current,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => max = actual,
            }
        }

        let res = (self.delay_fn)(proxy, url, timeout_ms);
        self.active_concurrency.fetch_sub(1, Ordering::SeqCst);
        res
    }
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        Ok(Vec::new())
    }
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        Ok(Vec::new())
    }
    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        Ok(())
    }
    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        Ok(ConnectionSnapshot::default())
    }
    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        Ok(MemoryData::default())
    }
    async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn close_all_connections(&self) -> Result<(), PortError> {
        Ok(())
    }
    async fn stream_logs(
        &self,
        _level: Option<String>,
    ) -> Result<infiltrator_ports::runtime_gateway::RuntimeStream<String>, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            "not supported",
        ))
    }
    async fn stream_traffic(
        &self,
    ) -> Result<
        infiltrator_ports::runtime_gateway::RuntimeStream<infiltrator_domain::runtime::TrafficData>,
        PortError,
    > {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            "not supported",
        ))
    }
    async fn stream_connections(
        &self,
    ) -> Result<
        infiltrator_ports::runtime_gateway::RuntimeStream<
            infiltrator_domain::runtime::ConnectionSnapshot,
        >,
        PortError,
    > {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            "not supported",
        ))
    }
}

fn sample_proxies() -> HashMap<String, Proxy> {
    let mut map = HashMap::new();
    map.insert(
        "HK-Node-1".to_string(),
        Proxy::Shadowsocks(Shadowsocks {
            base: ProxyBase {
                name: "HK-Node-1".to_string(),
                alive: true,
                delay: Some(35),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    map.insert(
        "HK-Node-2".to_string(),
        Proxy::Shadowsocks(Shadowsocks {
            base: ProxyBase {
                name: "HK-Node-2".to_string(),
                alive: true,
                delay: Some(40),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    map.insert(
        "JP-Node-1".to_string(),
        Proxy::Vmess(Vmess {
            base: ProxyBase {
                name: "JP-Node-1".to_string(),
                alive: true,
                delay: Some(70),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    map.insert(
        "HongKong".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "HongKong".to_string(),
            now: "HK-Node-1".to_string(),
            all: vec!["HK-Node-1".to_string(), "HK-Node-2".to_string()],
            history: Vec::new(),
        }),
    );
    map.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "HK-Node-1".to_string(),
            all: vec![
                "HK-Node-1".to_string(),
                "HK-Node-2".to_string(),
                "JP-Node-1".to_string(),
            ],
            history: Vec::new(),
        }),
    );
    map
}

#[tokio::test]
async fn test_single_group_independent_speedtest() {
    let gateway = Arc::new(TestGateway::new(sample_proxies()));
    let app = SpeedtestApplication::new(gateway.clone());

    // Test only "HongKong" group
    let snapshot = app
        .test_delays(
            SpeedtestScope::SingleGroup("HongKong".to_string()),
            None,
            None,
        )
        .await
        .expect("test delays should succeed");

    assert_eq!(snapshot.phase, SpeedtestPhase::Completed);
    assert_eq!(snapshot.node_results.len(), 2);
    assert!(snapshot.node_results.contains_key("HK-Node-1"));
    assert!(snapshot.node_results.contains_key("HK-Node-2"));
    assert!(!snapshot.node_results.contains_key("JP-Node-1"));
    assert_eq!(gateway.delay_probe_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_concurrency_limiting_semaphore_30() {
    let mut map = HashMap::new();
    for i in 0..45 {
        let name = format!("Node-{i:02}");
        map.insert(
            name.clone(),
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name,
                    alive: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }

    let gateway = Arc::new(
        TestGateway::new(map).with_delay_fn(|_name, _url, _timeout| {
            std::thread::sleep(std::time::Duration::from_millis(15));
            Ok(50)
        }),
    );

    let app = SpeedtestApplication::new(gateway.clone()).with_concurrency(30);

    let snapshot = app
        .test_delays(SpeedtestScope::AllGroups, None, None)
        .await
        .expect("batch test success");

    assert_eq!(snapshot.node_results.len(), 45);
    assert!(gateway.max_concurrency.load(Ordering::SeqCst) <= 30);
}

#[tokio::test]
async fn test_runtime_set_concurrency_updates_config_and_effective_bound() {
    let mut map = HashMap::new();
    for i in 0..45 {
        let name = format!("Node-{i:02}");
        map.insert(
            name.clone(),
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name,
                    alive: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }

    let gateway = Arc::new(
        TestGateway::new(map).with_delay_fn(|_name, _url, _timeout| {
            std::thread::sleep(std::time::Duration::from_millis(15));
            Ok(50)
        }),
    );

    let app = SpeedtestApplication::new(gateway.clone());

    // The live bound is published into the shared config read model, and a
    // zero request is clamped to 1 rather than stalling the batch.
    app.set_concurrency(0);
    assert_eq!(app.snapshot().config.concurrency, 1);
    assert_eq!(app.concurrency(), 1);

    app.set_concurrency(4);
    assert_eq!(app.snapshot().config.concurrency, 4);

    let snapshot = app
        .test_delays(SpeedtestScope::AllGroups, None, None)
        .await
        .expect("batch test success");

    // The batch honored the runtime bound, not the constructor default.
    assert_eq!(snapshot.node_results.len(), 45);
    assert_eq!(snapshot.config.concurrency, 4);
    assert!(gateway.max_concurrency.load(Ordering::SeqCst) <= 4);
}

#[tokio::test]
async fn test_dynamic_custom_url_and_timeout_propagation() {
    let observed_url = Arc::new(Mutex::new(String::new()));
    let observed_timeout = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let u_clone = Arc::clone(&observed_url);
    let t_clone = Arc::clone(&observed_timeout);

    let gateway = Arc::new(TestGateway::new(sample_proxies()).with_delay_fn(
        move |_node, url, timeout| {
            *u_clone.lock().unwrap() = url.to_string();
            t_clone.store(timeout, Ordering::SeqCst);
            Ok(35)
        },
    ));

    let app = SpeedtestApplication::new(gateway);

    let custom_url = "https://cp.cloudflare.com/generate_204".to_string();
    let custom_timeout = 8888;

    let snapshot = app
        .test_delays(
            SpeedtestScope::SingleNode("HK-Node-1".to_string()),
            Some(custom_url.clone()),
            Some(custom_timeout),
        )
        .await
        .expect("success");

    assert_eq!(*observed_url.lock().unwrap(), custom_url);
    assert_eq!(observed_timeout.load(Ordering::SeqCst), custom_timeout);
    assert_eq!(snapshot.config.test_url, custom_url);
    assert_eq!(snapshot.config.timeout_ms, custom_timeout);
}

#[tokio::test]
async fn test_probe_node_jitter_and_rtt_standard_deviation() {
    let probe_index = Arc::new(AtomicUsize::new(0));
    let p_clone = Arc::clone(&probe_index);

    // Sequence of RTTs: 40, 50, 40, 50
    // Mean = 45.0
    // Sample std dev = sqrt((( -5 )^2 * 4) / 3) = sqrt(100 / 3) ≈ 5.7735
    let gateway = Arc::new(TestGateway::new(sample_proxies()).with_delay_fn(
        move |_node, _url, _timeout| {
            let idx = p_clone.fetch_add(1, Ordering::SeqCst);
            if idx.is_multiple_of(2) {
                Ok(40)
            } else {
                Ok(50)
            }
        },
    ));

    let app = SpeedtestApplication::new(gateway);
    let jitter = app
        .probe_node_jitter("HK-Node-1", 4, None, None)
        .await
        .expect("jitter probe success");

    assert_eq!(jitter.sample_count, 4);
    assert_eq!(jitter.successful_probes, 4);
    assert_eq!(jitter.loss_percent, 0.0);
    assert_eq!(jitter.loss_rating, PacketLossRating::Excellent);
    assert!((jitter.mean_rtt_ms - 45.0).abs() < 1e-4);
    assert!((jitter.std_dev_ms - 5.7735).abs() < 1e-3);
    assert_eq!(jitter.min_rtt_ms, Some(40));
    assert_eq!(jitter.max_rtt_ms, Some(50));
}

#[tokio::test]
async fn test_cancellation_and_safe_interruption() {
    let app_holder: Arc<Mutex<Option<SpeedtestApplication>>> = Arc::new(Mutex::new(None));
    let app_holder_clone = Arc::clone(&app_holder);

    let gateway = Arc::new(TestGateway::new(sample_proxies()).with_delay_fn(
        move |_node, _url, _timeout| {
            if let Some(ref app) = *app_holder_clone.lock().unwrap() {
                app.cancel();
            }
            Ok(40)
        },
    ));

    let app = SpeedtestApplication::new(gateway);
    *app_holder.lock().unwrap() = Some(app.clone());

    let snapshot = app
        .test_delays(SpeedtestScope::AllGroups, None, None)
        .await
        .expect("test returns snapshot");

    assert_eq!(snapshot.phase, SpeedtestPhase::Cancelled);
}

#[tokio::test]
async fn test_history_caching_last_3_runs() {
    let gateway = Arc::new(TestGateway::new(sample_proxies()));
    let app = SpeedtestApplication::new(gateway);

    for _ in 0..5 {
        let _ = app
            .test_delays(
                SpeedtestScope::SingleNode("HK-Node-1".to_string()),
                None,
                None,
            )
            .await;
    }

    let history = app.get_history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].target_url, DEFAULT_DELAY_TEST_URL);
}

#[tokio::test]
async fn test_record_bandwidth_populates_shared_snapshot() {
    let gateway = Arc::new(TestGateway::new(sample_proxies()));
    let app = SpeedtestApplication::new(gateway);

    // 10 MiB over 1000 ms == 80 Mbps, rounded by the domain calculator.
    let mbps = app
        .record_bandwidth("HK-Node-1", 10 * 1024 * 1024, 1000)
        .expect("bandwidth recorded");
    assert!((mbps - 80.0).abs() < 0.01);

    let snapshot = app.snapshot();
    let entry = snapshot
        .node_results
        .get("HK-Node-1")
        .expect("node result published");
    assert_eq!(entry.bandwidth_mbps, Some(mbps));

    // A zero-duration sample is honest 0.0, never a divide-by-zero fiction.
    assert_eq!(app.record_bandwidth("HK-Node-1", 4096, 0).unwrap(), 0.0);
}

#[tokio::test]
async fn test_record_outbound_ip_populates_and_compares_label_country() {
    let gateway = Arc::new(TestGateway::new(sample_proxies()));
    let app = SpeedtestApplication::new(gateway);

    // "HK-Node-1" carries an HK label; a report of a HK egress is a match.
    app.record_outbound_ip("HK-Node-1", "103.242.175.12", Some("HK"))
        .expect("egress recorded");
    let snapshot = app.snapshot();
    let entry = snapshot
        .node_results
        .get("HK-Node-1")
        .expect("node published");
    assert_eq!(entry.label_country.as_deref(), Some("HK"));
    assert_eq!(entry.outbound_ip.as_deref(), Some("103.242.175.12"));
    assert_eq!(entry.outbound_country.as_deref(), Some("HK"));
    assert_eq!(entry.egress_country_match(), EgressCountryMatch::Match);
    assert!(snapshot.egress_country_mismatches().is_empty());

    // A different egress country is an honest mismatch, not silently rewritten.
    app.record_outbound_ip("HK-Node-2", "45.32.1.9", Some("US"))
        .expect("egress recorded");
    let snapshot = app.snapshot();
    let entry = snapshot.node_results.get("HK-Node-2").unwrap();
    assert_eq!(entry.egress_country_match(), EgressCountryMatch::Mismatch);
    assert_eq!(snapshot.egress_country_mismatches().len(), 1);
    assert_eq!(snapshot.egress_reported_count(), 2);

    // An empty IP is rejected rather than stored as a fiction.
    let err = app
        .record_outbound_ip("HK-Node-1", "   ", Some("HK"))
        .expect_err("empty ip rejected");
    assert_eq!(
        err.code,
        infiltrator_contract::error::ErrorCode::InvalidInput
    );

    // A probe with no geolocation is honest `Unlabelled`, never a guessed CC.
    app.record_outbound_ip("HK-Node-1", "1.2.3.4", None)
        .expect("egress without country recorded");
    let snapshot = app.snapshot();
    let entry = snapshot.node_results.get("HK-Node-1").unwrap();
    assert_eq!(entry.outbound_country, None);
    assert_eq!(entry.egress_country_match(), EgressCountryMatch::Unknown);
}

#[tokio::test]
async fn test_test_delays_publishes_label_country_not_egress_fiction() {
    let gateway = Arc::new(TestGateway::new(sample_proxies()));
    let app = SpeedtestApplication::new(gateway);

    let snapshot = app
        .test_delays(
            SpeedtestScope::SingleNode("JP-Node-1".to_string()),
            None,
            None,
        )
        .await
        .expect("probe succeeds");

    let entry = snapshot.node_results.get("JP-Node-1").unwrap();
    // The label fact is published; the egress fact stays honestly absent until
    // a real host probe reports it.
    assert_eq!(entry.label_country.as_deref(), Some("JP"));
    assert_eq!(entry.outbound_ip, None);
    assert_eq!(entry.outbound_country, None);
    assert_eq!(entry.egress_country_match(), EgressCountryMatch::Unknown);
}

#[derive(Default)]
struct MemoryHistoryStore {
    records: Mutex<Vec<HistoricalSpeedtestRecord>>,
}

impl SpeedtestHistoryStore for MemoryHistoryStore {
    fn load(&self) -> Result<Vec<HistoricalSpeedtestRecord>, PortError> {
        Ok(self.records.lock().unwrap().clone())
    }

    fn save(&self, records: &[HistoricalSpeedtestRecord]) -> Result<(), PortError> {
        *self.records.lock().unwrap() = records.to_vec();
        Ok(())
    }
}

#[tokio::test]
async fn test_history_store_round_trip_restores_across_restart() {
    let store = Arc::new(MemoryHistoryStore::default());
    let gateway = Arc::new(TestGateway::new(sample_proxies()));
    let app = SpeedtestApplication::new(gateway.clone()).with_history_store(store.clone());

    for i in 0..4 {
        let _ = app
            .test_delays(
                SpeedtestScope::AllGroups,
                Some(format!("http://run-{i}.local")),
                None,
            )
            .await;
    }

    // Bounded to the existing cap and written through to the host store.
    assert_eq!(store.load().unwrap().len(), 3);
    assert_eq!(app.get_history().len(), 3);

    // A fresh application over the same store restores identical history:
    // the cross-restart proof.
    let restarted = SpeedtestApplication::new(gateway).with_history_store(store);
    let restored = restarted.get_history();
    assert_eq!(restored, app.get_history());
    assert_eq!(restored[0].target_url, "http://run-1.local");
    assert_eq!(restored[2].target_url, "http://run-3.local");
}
