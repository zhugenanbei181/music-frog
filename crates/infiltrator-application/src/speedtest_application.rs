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
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
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
    history_store: Option<Arc<dyn SpeedtestHistoryStore>>,
}

impl SpeedtestApplication {
    /// Create a new speedtest application with default Semaphore(30) concurrency.
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            state: Arc::new(Mutex::new(SpeedtestSnapshot::default())),
            cancel_token: Arc::new(AtomicBool::new(false)),
            concurrency_limit: DEFAULT_CONCURRENCY_LIMIT,
            history_store: None,
        }
    }

    /// Set a custom concurrency limit for batch speedtesting.
    pub fn with_concurrency(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit.max(1);
        self
    }

    /// Attach a durable history store and hydrate the bounded run history from
    /// any records persisted by a previous process.
    pub fn with_history_store(mut self, store: Arc<dyn SpeedtestHistoryStore>) -> Self {
        if let Ok(records) = store.load() {
            let mut state = self.state.lock().unwrap();
            state.recent_history = bounded_history(records);
        }
        self.history_store = Some(store);
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
            self.persist_history(&final_state);
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
        self.persist_history(&state);
        Ok(bandwidth_mbps)
    }

    /// Retrieve the cached history of recent runs.
    pub fn get_history(&self) -> Vec<HistoricalSpeedtestRecord> {
        self.state.lock().unwrap().recent_history.clone()
    }

    /// Best-effort write-through of the bounded history to the host store.
    fn persist_history(&self, state: &SpeedtestSnapshot) {
        if let Some(store) = &self.history_store {
            let _ = store.save(&state.recent_history);
        }
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

/// Clamp persisted records to the in-memory cap, keeping the newest entries.
fn bounded_history(mut history: Vec<HistoricalSpeedtestRecord>) -> Vec<HistoricalSpeedtestRecord> {
    if history.len() > MAX_HISTORY_ENTRIES {
        history.drain(0..history.len() - MAX_HISTORY_ENTRIES);
    }
    history
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
#[path = "speedtest_application_tests.rs"]
mod tests;
