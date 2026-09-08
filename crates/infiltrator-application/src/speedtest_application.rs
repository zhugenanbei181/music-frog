//! Application service for concurrent speedtesting, jitter analysis, and stability evaluation.

use futures_util::stream::{self, StreamExt};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::speedtest::{
    HistoricalSpeedtestRecord, JitterCalculation, NodeSpeedtestResult, PacketLossRating,
    SpeedtestPhase, SpeedtestProgress, SpeedtestScope, SpeedtestSnapshot, SpeedtestTargetConfig,
};
use infiltrator_domain::filter::extract_country_code;
use infiltrator_domain::proxy::Proxy;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
const DEFAULT_DELAY_TIMEOUT_MS: u32 = 5000;
const DEFAULT_CONCURRENCY_LIMIT: usize = 30;
const MAX_HISTORY_ENTRIES: usize = 3;

/// Shared application service managing speedtesting workflows.
#[derive(Clone)]
pub struct SpeedtestApplication {
    gateway: Arc<dyn RuntimeGateway>,
    state: Arc<Mutex<SpeedtestSnapshot>>,
    cancel_token: Arc<AtomicBool>,
    concurrency_limit: usize,
}

impl SpeedtestApplication {
    /// Create a new speedtest application with default Semaphore(30) concurrency.
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            state: Arc::new(Mutex::new(SpeedtestSnapshot::default())),
            cancel_token: Arc::new(AtomicBool::new(false)),
            concurrency_limit: DEFAULT_CONCURRENCY_LIMIT,
        }
    }

    /// Set a custom concurrency limit for batch speedtesting.
    pub fn with_concurrency(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit.max(1);
        self
    }

    /// Get current snapshot of the speedtest engine.
    pub fn snapshot(&self) -> SpeedtestSnapshot {
        self.state.lock().unwrap().clone()
    }

    /// Whether a speedtest or latency probe is actively running.
    pub fn is_running(&self) -> bool {
        self.snapshot().is_running()
    }

    /// Request cancellation of the active speedtest batch.
    pub fn cancel(&self) -> bool {
        let was_running = self.is_running();
        self.cancel_token.store(true, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        if state.is_running() {
            state.phase = SpeedtestPhase::Cancelled;
            state.revision += 1;
        }
        was_running
    }

    /// Run concurrent latency probes across nodes specified by scope.
    pub async fn test_delays(
        &self,
        scope: SpeedtestScope,
        custom_url: Option<String>,
        custom_timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, Failure> {
        self.cancel_token.store(false, Ordering::SeqCst);

        let test_url = custom_url.unwrap_or_else(|| DEFAULT_DELAY_TEST_URL.to_string());
        let timeout_ms = custom_timeout_ms.unwrap_or(DEFAULT_DELAY_TIMEOUT_MS);

        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let candidates = resolve_candidates(&proxies, &scope)?;

        if candidates.is_empty() {
            let mut state = self.state.lock().unwrap();
            state.phase = SpeedtestPhase::Completed;
            state.scope = scope;
            state.progress = SpeedtestProgress::default();
            state.revision += 1;
            return Ok(state.clone());
        }

        let total_nodes = candidates.len();
        {
            let mut state = self.state.lock().unwrap();
            state.generation += 1;
            state.revision += 1;
            state.phase = SpeedtestPhase::ProbingLatency;
            state.scope = scope.clone();
            state.config = SpeedtestTargetConfig {
                test_url: test_url.clone(),
                timeout_ms,
                concurrency: self.concurrency_limit,
                probe_rounds: 1,
            };
            state.progress = SpeedtestProgress {
                completed_nodes: 0,
                total_nodes,
                percent: 0.0,
                current_node: None,
                current_mbps: None,
            };
        }

        let state_arc = Arc::clone(&self.state);
        let cancel_token = Arc::clone(&self.cancel_token);
        let gateway = Arc::clone(&self.gateway);
        let completed_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let results_stream = stream::iter(candidates.into_iter().map(|candidate| {
            let gateway = Arc::clone(&gateway);
            let state_arc = Arc::clone(&state_arc);
            let cancel_token = Arc::clone(&cancel_token);
            let completed_counter = Arc::clone(&completed_counter);
            let test_url = test_url.clone();
            let node_name = candidate.name;
            let group_name = candidate.group_name;
            let proxy_type = candidate.proxy_type;

            async move {
                if cancel_token.load(Ordering::Relaxed) {
                    return None;
                }

                let test_res = gateway.test_delay(&node_name, &test_url, timeout_ms).await;
                let now_ms = current_epoch_ms();

                let (delay_ms, is_alive, packet_loss, star_rating) = match test_res {
                    Ok(d) => {
                        let stars = match d {
                            0..=99 => 5,
                            100..=199 => 4,
                            200..=349 => 3,
                            350..=499 => 2,
                            _ => 1,
                        };
                        (Some(d), true, PacketLossRating::Excellent, stars)
                    }
                    Err(_) => (None, false, PacketLossRating::Dead, 1),
                };

                let jitter = delay_ms.map(|d| JitterCalculation::from_samples(&[Some(d)]));
                let outbound_country = extract_country_code(&node_name).map(str::to_string);

                let completed = completed_counter.fetch_add(1, Ordering::SeqCst) + 1;
                let percent = (completed as f64 / total_nodes as f64) * 100.0;

                {
                    let mut state = state_arc.lock().unwrap();
                    state.progress.completed_nodes = completed;
                    state.progress.percent = percent;
                    state.progress.current_node = Some(node_name.clone());
                    state.revision += 1;
                }

                Some(NodeSpeedtestResult {
                    node_name,
                    group_name,
                    proxy_type,
                    delay_ms,
                    jitter,
                    bandwidth_mbps: None,
                    packet_loss,
                    star_rating,
                    outbound_ip: None,
                    outbound_country,
                    is_alive,
                    tested_at_epoch_ms: now_ms,
                })
            }
        }))
        .buffer_unordered(self.concurrency_limit.max(1));

        let collected: Vec<Option<NodeSpeedtestResult>> = results_stream.collect().await;

        let mut final_state = self.state.lock().unwrap();
        if self.cancel_token.load(Ordering::Relaxed) {
            final_state.phase = SpeedtestPhase::Cancelled;
        } else {
            final_state.phase = SpeedtestPhase::Completed;
        }

        for item in collected.into_iter().flatten() {
            final_state
                .node_results
                .insert(item.node_name.clone(), item);
        }

        final_state.progress.percent = 100.0;
        final_state.progress.current_node = None;
        final_state.revision += 1;

        if !self.cancel_token.load(Ordering::Relaxed) {
            let record = build_historical_record(&final_state, &scope, &test_url);
            push_history_record(&mut final_state.recent_history, record);
        }

        Ok(final_state.clone())
    }

    /// Perform multi-round ping on a specific node to calculate exact RTT standard deviation and packet loss.
    pub async fn probe_node_jitter(
        &self,
        node: &str,
        rounds: usize,
        custom_url: Option<String>,
        custom_timeout_ms: Option<u32>,
    ) -> Result<JitterCalculation, Failure> {
        let rounds = rounds.max(1);
        let test_url = custom_url.unwrap_or_else(|| DEFAULT_DELAY_TEST_URL.to_string());
        let timeout_ms = custom_timeout_ms.unwrap_or(DEFAULT_DELAY_TIMEOUT_MS);

        let mut samples = Vec::with_capacity(rounds);

        for _ in 0..rounds {
            if self.cancel_token.load(Ordering::Relaxed) {
                return Err(Failure::new(
                    infiltrator_contract::error::ErrorCode::Canceled,
                    "jitter probe cancelled",
                    false,
                ));
            }
            let res = self.gateway.test_delay(node, &test_url, timeout_ms).await;
            match res {
                Ok(rtt) => samples.push(Some(rtt)),
                Err(_) => samples.push(None),
            }
        }

        let calculation = JitterCalculation::from_samples(&samples);

        {
            let mut state = self.state.lock().unwrap();
            if let Some(entry) = state.node_results.get_mut(node) {
                entry.jitter = Some(calculation.clone());
                entry.packet_loss = calculation.loss_rating;
                entry.star_rating = calculation.star_rating;
                entry.delay_ms = calculation.min_rtt_ms;
                entry.is_alive = calculation.successful_probes > 0;
            } else {
                state.node_results.insert(
                    node.to_string(),
                    NodeSpeedtestResult {
                        node_name: node.to_string(),
                        group_name: None,
                        proxy_type: "Unknown".to_string(),
                        delay_ms: calculation.min_rtt_ms,
                        jitter: Some(calculation.clone()),
                        bandwidth_mbps: None,
                        packet_loss: calculation.loss_rating,
                        star_rating: calculation.star_rating,
                        outbound_ip: None,
                        outbound_country: extract_country_code(node).map(str::to_string),
                        is_alive: calculation.successful_probes > 0,
                        tested_at_epoch_ms: current_epoch_ms(),
                    },
                );
            }
            state.revision += 1;
        }

        Ok(calculation)
    }

    /// Record measured bandwidth throughput for a node.
    pub fn record_bandwidth(
        &self,
        node: &str,
        total_bytes: u64,
        duration_ms: u64,
    ) -> Result<f64, Failure> {
        let bandwidth_mbps =
            infiltrator_domain::diagnostics::SpeedtestCalculator::calculate_bandwidth(
                total_bytes,
                duration_ms,
            );

        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.node_results.get_mut(node) {
            entry.bandwidth_mbps = Some(bandwidth_mbps);
        } else {
            state.node_results.insert(
                node.to_string(),
                NodeSpeedtestResult {
                    node_name: node.to_string(),
                    group_name: None,
                    proxy_type: "Unknown".to_string(),
                    delay_ms: None,
                    jitter: None,
                    bandwidth_mbps: Some(bandwidth_mbps),
                    packet_loss: PacketLossRating::Excellent,
                    star_rating: if bandwidth_mbps >= 100.0 { 5 } else { 4 },
                    outbound_ip: None,
                    outbound_country: extract_country_code(node).map(str::to_string),
                    is_alive: true,
                    tested_at_epoch_ms: current_epoch_ms(),
                },
            );
        }
        state.revision += 1;
        Ok(bandwidth_mbps)
    }

    /// Retrieve the cached history of recent runs.
    pub fn get_history(&self) -> Vec<HistoricalSpeedtestRecord> {
        self.state.lock().unwrap().recent_history.clone()
    }
}

#[derive(Debug, Clone)]
struct CandidateNode {
    name: String,
    group_name: Option<String>,
    proxy_type: String,
}

fn resolve_candidates(
    proxies: &std::collections::HashMap<String, Proxy>,
    scope: &SpeedtestScope,
) -> Result<Vec<CandidateNode>, Failure> {
    match scope {
        SpeedtestScope::AllGroups => {
            let mut list = Vec::new();
            for (name, proxy) in proxies {
                if !proxy.is_group() && !name.is_empty() {
                    list.push(CandidateNode {
                        name: name.clone(),
                        group_name: None,
                        proxy_type: proxy.proxy_type().to_string(),
                    });
                }
            }
            list.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(list)
        }
        SpeedtestScope::SingleGroup(group_name) => {
            let group_proxy = proxies.get(group_name).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("proxy group {group_name} was not found"),
                    false,
                )
            })?;

            let member_names = group_proxy.all().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("{group_name} is not a valid proxy group"),
                    false,
                )
            })?;

            let mut candidates = Vec::new();
            let mut seen = HashSet::new();

            for member in member_names {
                if seen.insert(member.clone())
                    && let Some(node_proxy) = proxies.get(member)
                    && !node_proxy.is_group()
                {
                    candidates.push(CandidateNode {
                        name: member.clone(),
                        group_name: Some(group_name.clone()),
                        proxy_type: node_proxy.proxy_type().to_string(),
                    });
                }
            }

            Ok(candidates)
        }
        SpeedtestScope::SingleNode(node_name) => {
            let node_proxy = proxies.get(node_name).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("proxy node {node_name} was not found"),
                    false,
                )
            })?;

            Ok(vec![CandidateNode {
                name: node_name.clone(),
                group_name: None,
                proxy_type: node_proxy.proxy_type().to_string(),
            }])
        }
    }
}

fn build_historical_record(
    snapshot: &SpeedtestSnapshot,
    scope: &SpeedtestScope,
    test_url: &str,
) -> HistoricalSpeedtestRecord {
    let total_nodes = snapshot.node_results.len();
    let alive_nodes = snapshot
        .node_results
        .values()
        .filter(|n| n.is_alive)
        .count();

    let valid_delays: Vec<u32> = snapshot
        .node_results
        .values()
        .filter_map(|n| n.delay_ms)
        .collect();
    let avg_latency_ms = if !valid_delays.is_empty() {
        Some(valid_delays.iter().map(|&d| d as f64).sum::<f64>() / valid_delays.len() as f64)
    } else {
        None
    };

    let valid_jitters: Vec<f64> = snapshot
        .node_results
        .values()
        .filter_map(|n| n.jitter.as_ref().map(|j| j.jitter_ms))
        .collect();
    let avg_jitter_ms = if !valid_jitters.is_empty() {
        Some(valid_jitters.iter().sum::<f64>() / valid_jitters.len() as f64)
    } else {
        None
    };

    let valid_bandwidths: Vec<f64> = snapshot
        .node_results
        .values()
        .filter_map(|n| n.bandwidth_mbps)
        .collect();
    let avg_bandwidth_mbps = if !valid_bandwidths.is_empty() {
        Some(valid_bandwidths.iter().sum::<f64>() / valid_bandwidths.len() as f64)
    } else {
        None
    };

    let star_sum: u64 = snapshot
        .node_results
        .values()
        .map(|n| n.star_rating as u64)
        .sum();
    let overall_star_rating = if total_nodes > 0 {
        ((star_sum as f64 / total_nodes as f64).round() as u8).clamp(1, 5)
    } else {
        1
    };

    HistoricalSpeedtestRecord {
        run_id: snapshot.generation,
        timestamp_epoch_ms: current_epoch_ms(),
        scope: scope.clone(),
        target_url: test_url.to_string(),
        total_nodes,
        alive_nodes,
        avg_latency_ms,
        avg_jitter_ms,
        avg_bandwidth_mbps,
        overall_star_rating,
    }
}

fn push_history_record(
    history: &mut Vec<HistoricalSpeedtestRecord>,
    record: HistoricalSpeedtestRecord,
) {
    history.push(record);
    if history.len() > MAX_HISTORY_ENTRIES {
        history.remove(0);
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_domain::proxy::{ProxyBase, ProxyGroup, Shadowsocks, Vmess};
    use infiltrator_domain::runtime::{
        ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider,
    };
    use infiltrator_ports::error::PortError;
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
        async fn test_delay(
            &self,
            proxy: &str,
            url: &str,
            timeout_ms: u32,
        ) -> Result<u32, PortError> {
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
            infiltrator_ports::runtime_gateway::RuntimeStream<
                infiltrator_domain::runtime::TrafficData,
            >,
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
}
