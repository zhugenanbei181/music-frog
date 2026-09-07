//! Headless test suite for Track 3 / Group 06: Speedtest, Jitter, and Stability.

use async_trait::async_trait;
use infiltrator_application::speedtest_application::SpeedtestApplication;
use infiltrator_contract::speedtest::{
    JitterCalculation, PacketLossRating, SpeedtestPhase, SpeedtestScope,
};
use infiltrator_domain::proxy::{Proxy, ProxyBase, ProxyGroup, Shadowsocks};
use infiltrator_domain::runtime::{ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct MockGateway {
    proxies: HashMap<String, Proxy>,
    delay_count: AtomicUsize,
    active_concurrency: AtomicUsize,
    max_concurrency: AtomicUsize,
    last_url: Mutex<String>,
    last_timeout: Mutex<u32>,
    delay_fn: Box<dyn Fn(&str, &str, u32) -> Result<u32, PortError> + Send + Sync>,
}

impl MockGateway {
    fn new(proxies: HashMap<String, Proxy>) -> Self {
        Self {
            proxies,
            delay_count: AtomicUsize::new(0),
            active_concurrency: AtomicUsize::new(0),
            max_concurrency: AtomicUsize::new(0),
            last_url: Mutex::new(String::new()),
            last_timeout: Mutex::new(0),
            delay_fn: Box::new(|_p, _u, _t| Ok(50)),
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
impl RuntimeGateway for MockGateway {
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
        Ok(ConfigSnapshot::default())
    }
    async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
        Ok(())
    }
    async fn set_proxy_mode(&self, _mode: infiltrator_contract::command::ProxyMode) -> Result<(), PortError> {
        Ok(())
    }
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
        Ok(self.proxies.clone())
    }
    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError> {
        self.delay_count.fetch_add(1, Ordering::SeqCst);
        *self.last_url.lock().unwrap() = url.to_string();
        *self.last_timeout.lock().unwrap() = timeout_ms;

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
    async fn stream_logs(&self, _level: Option<String>) -> Result<infiltrator_ports::runtime_gateway::RuntimeStream<String>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }
    async fn stream_traffic(&self) -> Result<infiltrator_ports::runtime_gateway::RuntimeStream<infiltrator_domain::runtime::TrafficData>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }
    async fn stream_connections(&self) -> Result<infiltrator_ports::runtime_gateway::RuntimeStream<infiltrator_domain::runtime::ConnectionSnapshot>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }
}

fn create_test_topology(node_count: usize) -> HashMap<String, Proxy> {
    let mut map = HashMap::new();
    let mut all_hk = Vec::new();
    let mut all_sg = Vec::new();

    for i in 0..node_count {
        let name = if i % 2 == 0 {
            let n = format!("🇭🇰 香港 BGP {i:02}");
            all_hk.push(n.clone());
            n
        } else {
            let n = format!("🇸🇬 新加坡 Anycast {i:02}");
            all_sg.push(n.clone());
            n
        };

        map.insert(
            name.clone(),
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name,
                    alive: true,
                    delay: Some(30 + (i as u32 % 50)),
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }

    map.insert(
        "HongKongGroup".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "HongKongGroup".to_string(),
            now: all_hk.first().cloned().unwrap_or_default(),
            all: all_hk,
            history: Vec::new(),
        }),
    );

    map.insert(
        "SingaporeGroup".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "SingaporeGroup".to_string(),
            now: all_sg.first().cloned().unwrap_or_default(),
            all: all_sg,
            history: Vec::new(),
        }),
    );

    map
}

#[tokio::test]
async fn test_group_06_semaphore_concurrency_30() {
    // 60 nodes total; default limit = 30
    let proxies = create_test_topology(60);
    let gateway = Arc::new(MockGateway::new(proxies).with_delay_fn(|_p, _u, _t| {
        std::thread::sleep(Duration::from_millis(15));
        Ok(40)
    }));

    let app = SpeedtestApplication::new(gateway.clone());
    let snapshot = app
        .test_delays(SpeedtestScope::AllGroups, None, None)
        .await
        .expect("test all should succeed");

    assert_eq!(snapshot.node_results.len(), 60);
    assert!(gateway.max_concurrency.load(Ordering::SeqCst) <= 30);
    assert_eq!(snapshot.phase, SpeedtestPhase::Completed);
}

#[tokio::test]
async fn test_group_06_single_strategy_group_independent_speedtest() {
    let proxies = create_test_topology(20);
    let gateway = Arc::new(MockGateway::new(proxies));
    let app = SpeedtestApplication::new(gateway.clone());

    let snapshot = app
        .test_delays(
            SpeedtestScope::SingleGroup("HongKongGroup".to_string()),
            None,
            None,
        )
        .await
        .expect("single group test should succeed");

    assert_eq!(snapshot.node_results.len(), 10);
    for name in snapshot.node_results.keys() {
        assert!(name.contains("香港"));
    }
    assert_eq!(gateway.delay_count.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn test_group_06_custom_target_url_and_timeout() {
    let proxies = create_test_topology(4);
    let gateway = Arc::new(MockGateway::new(proxies));
    let app = SpeedtestApplication::new(gateway.clone());

    let custom_url = "https://1.1.1.1/cdn-cgi/trace".to_string();
    let custom_timeout = 7500;

    let snapshot = app
        .test_delays(
            SpeedtestScope::AllGroups,
            Some(custom_url.clone()),
            Some(custom_timeout),
        )
        .await
        .expect("custom target test should succeed");

    assert_eq!(snapshot.config.test_url, custom_url);
    assert_eq!(snapshot.config.timeout_ms, custom_timeout);
    assert_eq!(*gateway.last_url.lock().unwrap(), custom_url);
    assert_eq!(*gateway.last_timeout.lock().unwrap(), custom_timeout);
}

#[test]
fn test_group_06_jitter_ms_and_rtt_standard_deviation() {
    // 5 samples with known values: 100, 105, 95, 110, 90
    // Mean = 100.0
    // Diff: 0, 5, -5, 10, -10 -> sum of squares: 0 + 25 + 25 + 100 + 100 = 250
    // Variance = 250 / (5 - 1) = 62.5
    // Std dev = sqrt(62.5) ≈ 7.90569
    // Consecutive diffs: |105-100|=5, |95-105|=10, |110-95|=15, |90-110|=20 -> sum = 50
    // Jitter = 50 / 4 = 12.5
    let jitter = JitterCalculation::from_samples(&[
        Some(100),
        Some(105),
        Some(95),
        Some(110),
        Some(90),
    ]);

    assert_eq!(jitter.sample_count, 5);
    assert_eq!(jitter.successful_probes, 5);
    assert_eq!(jitter.lost_probes, 0);
    assert_eq!(jitter.loss_percent, 0.0);
    assert_eq!(jitter.loss_rating, PacketLossRating::Excellent);
    assert!((jitter.mean_rtt_ms - 100.0).abs() < 1e-4);
    assert!((jitter.std_dev_ms - 7.90569).abs() < 1e-3);
    assert!((jitter.jitter_ms - 12.5).abs() < 1e-3);
    assert_eq!(jitter.min_rtt_ms, Some(90));
    assert_eq!(jitter.max_rtt_ms, Some(110));
    assert!(jitter.star_rating >= 3);
}

#[test]
fn test_group_06_packet_loss_gradient_rating_tiers() {
    // 0% -> Excellent
    assert_eq!(PacketLossRating::from_loss_percent(0.0), PacketLossRating::Excellent);
    assert_eq!(PacketLossRating::from_loss_percent(0.0).label(), "极佳 (0%)");

    // <5% -> Good
    assert_eq!(PacketLossRating::from_loss_percent(2.5), PacketLossRating::Good);
    assert_eq!(PacketLossRating::from_loss_percent(5.0), PacketLossRating::Good);
    assert_eq!(PacketLossRating::from_loss_percent(5.0).label(), "良好 (<5%)");

    // 5-20% -> Fair
    assert_eq!(PacketLossRating::from_loss_percent(5.1), PacketLossRating::Fair);
    assert_eq!(PacketLossRating::from_loss_percent(12.0), PacketLossRating::Fair);
    assert_eq!(PacketLossRating::from_loss_percent(20.0), PacketLossRating::Fair);
    assert_eq!(PacketLossRating::from_loss_percent(20.0).label(), "一般 (5-20%)");

    // >20% -> Poor
    assert_eq!(PacketLossRating::from_loss_percent(20.1), PacketLossRating::Poor);
    assert_eq!(PacketLossRating::from_loss_percent(45.0), PacketLossRating::Poor);
    assert_eq!(PacketLossRating::from_loss_percent(80.0), PacketLossRating::Poor);
    assert_eq!(PacketLossRating::from_loss_percent(80.0).label(), "较差 (>20%)");

    // 100% -> Dead
    assert_eq!(PacketLossRating::from_loss_percent(100.0), PacketLossRating::Dead);
    assert_eq!(PacketLossRating::from_loss_percent(100.0).label(), "超时 (100%)");
}

#[tokio::test]
async fn test_group_06_cancellation_safe_interruption() {
    let proxies = create_test_topology(10);
    let app_holder: Arc<Mutex<Option<SpeedtestApplication>>> = Arc::new(Mutex::new(None));
    let holder_clone = Arc::clone(&app_holder);

    let gateway = Arc::new(MockGateway::new(proxies).with_delay_fn(move |_p, _u, _t| {
        if let Some(ref app) = *holder_clone.lock().unwrap() {
            app.cancel();
        }
        Ok(45)
    }));

    let app = SpeedtestApplication::new(gateway);
    *app_holder.lock().unwrap() = Some(app.clone());

    let snapshot = app
        .test_delays(SpeedtestScope::AllGroups, None, None)
        .await
        .expect("returns snapshot on cancellation");

    assert_eq!(snapshot.phase, SpeedtestPhase::Cancelled);
}

#[tokio::test]
async fn test_group_06_historical_cache_persists_3_runs() {
    let proxies = create_test_topology(4);
    let gateway = Arc::new(MockGateway::new(proxies));
    let app = SpeedtestApplication::new(gateway);

    for i in 0..5 {
        let _ = app
            .test_delays(
                SpeedtestScope::AllGroups,
                Some(format!("http://test-{i}.local")),
                None,
            )
            .await;
    }

    let history = app.get_history();
    assert_eq!(history.len(), 3);
    // Should contain the last 3: test-2, test-3, test-4
    assert_eq!(history[0].target_url, "http://test-2.local");
    assert_eq!(history[1].target_url, "http://test-3.local");
    assert_eq!(history[2].target_url, "http://test-4.local");
}

#[tokio::test]
async fn test_group_06_dead_nodes_archiving() {
    let mut proxies = HashMap::new();
    proxies.insert(
        "Healthy-01".to_string(),
        Proxy::Shadowsocks(Shadowsocks {
            base: ProxyBase {
                name: "Healthy-01".to_string(),
                alive: true,
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    proxies.insert(
        "Timeout-02".to_string(),
        Proxy::Shadowsocks(Shadowsocks {
            base: ProxyBase {
                name: "Timeout-02".to_string(),
                alive: false,
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    let gateway = Arc::new(MockGateway::new(proxies).with_delay_fn(|node, _u, _t| {
        if node == "Healthy-01" {
            Ok(35)
        } else {
            Err(PortError::Failed("timeout".into()))
        }
    }));

    let app = SpeedtestApplication::new(gateway);
    let snapshot = app
        .test_delays(SpeedtestScope::AllGroups, None, None)
        .await
        .expect("batch test success");

    assert_eq!(snapshot.node_count(), 2);
    assert_eq!(snapshot.alive_nodes_count(), 1);
    let dead = snapshot.dead_nodes();
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].node_name, "Timeout-02");
    assert_eq!(dead[0].packet_loss, PacketLossRating::Dead);
}
