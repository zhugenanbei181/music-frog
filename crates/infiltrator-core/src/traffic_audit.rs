use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Route classification for audited traffic flows.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditRouteType {
    Proxied,
    DirectBypass,
    Reject,
    Other,
}

/// Detailed reject/block reason for traffic audit attribution.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditRejectReason {
    RuleMatch,
    DnsBlock,
    GeoBlock,
    AclDeny,
    QuotaExceeded,
    LoopDetected,
    Other,
}

/// Cumulative byte, packet, and flow metrics for a traffic aggregation bucket.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrafficStats {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub packets_count: u64,
    pub flow_count: u64,
}

impl TrafficStats {
    /// Returns the sum of upload and download bytes.
    pub fn total_bytes(&self) -> u64 {
        self.upload_bytes.saturating_add(self.download_bytes)
    }

    /// Records an incremental flow into these metrics.
    pub fn record(&mut self, up: u64, down: u64, packets: u64) {
        self.upload_bytes = self.upload_bytes.saturating_add(up);
        self.download_bytes = self.download_bytes.saturating_add(down);
        self.packets_count = self.packets_count.saturating_add(packets);
        self.flow_count = self.flow_count.saturating_add(1);
    }

    /// Computes delta between two stat points (`self - previous`).
    pub fn delta_from(&self, previous: &Self) -> Self {
        Self {
            upload_bytes: self.upload_bytes.saturating_sub(previous.upload_bytes),
            download_bytes: self.download_bytes.saturating_sub(previous.download_bytes),
            packets_count: self.packets_count.saturating_sub(previous.packets_count),
            flow_count: self.flow_count.saturating_sub(previous.flow_count),
        }
    }
}

/// Exponentially Weighted Moving Average (EWMA) and differential rate estimator.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EwmaRateEstimator {
    pub alpha: f64,
    pub last_update_ts: f64,
    pub last_bytes: u64,
    pub instant_rate_bps: f64,
    pub ewma_rate_bps: f64,
    pub peak_rate_bps: f64,
}

impl Default for EwmaRateEstimator {
    fn default() -> Self {
        Self::new(0.3)
    }
}

impl EwmaRateEstimator {
    /// Creates a new EWMA estimator with given smoothing factor `alpha` in `(0.0, 1.0]`.
    pub fn new(alpha: f64) -> Self {
        let alpha = alpha.clamp(0.01, 1.0);
        Self {
            alpha,
            last_update_ts: 0.0,
            last_bytes: 0,
            instant_rate_bps: 0.0,
            ewma_rate_bps: 0.0,
            peak_rate_bps: 0.0,
        }
    }

    /// Updates the rate estimator with current timestamp (seconds) and cumulative byte count.
    /// Returns the updated EWMA rate in bytes per second (Bps).
    pub fn update(&mut self, now_secs: f64, total_bytes: u64) -> f64 {
        if self.last_update_ts <= 0.0 {
            self.last_update_ts = now_secs;
            self.last_bytes = total_bytes;
            return 0.0;
        }

        let dt = now_secs - self.last_update_ts;
        if dt <= 0.0 {
            return self.ewma_rate_bps;
        }

        let delta_bytes = total_bytes.saturating_sub(self.last_bytes) as f64;
        let instant_bps = delta_bytes / dt;

        self.instant_rate_bps = instant_bps;
        if instant_bps > self.peak_rate_bps {
            self.peak_rate_bps = instant_bps;
        }

        if self.ewma_rate_bps == 0.0 {
            self.ewma_rate_bps = instant_bps;
        } else {
            self.ewma_rate_bps = self.alpha * instant_bps + (1.0 - self.alpha) * self.ewma_rate_bps;
        }

        self.last_update_ts = now_secs;
        self.last_bytes = total_bytes;
        self.ewma_rate_bps
    }

    /// Current rate in Megabits per second (Mbps).
    pub fn mbps(&self) -> f64 {
        (self.ewma_rate_bps * 8.0) / 1_000_000.0
    }

    /// Peak rate in Megabits per second (Mbps).
    pub fn peak_mbps(&self) -> f64 {
        (self.peak_rate_bps * 8.0) / 1_000_000.0
    }

    /// Resets the estimator state.
    pub fn reset(&mut self) {
        self.last_update_ts = 0.0;
        self.last_bytes = 0;
        self.instant_rate_bps = 0.0;
        self.ewma_rate_bps = 0.0;
        self.peak_rate_bps = 0.0;
    }
}

/// Comprehensive RTT latency percentile and RFC 3550 Jitter tracker.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct LatencyDistributionTracker {
    samples: Vec<u32>,
    rfc3550_jitter: f64,
    last_sample_rtt: Option<f64>,
}

impl LatencyDistributionTracker {
    /// Creates an empty latency distribution tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an RTT latency sample in milliseconds.
    pub fn record_sample(&mut self, rtt_ms: u32) {
        let rtt_f64 = rtt_ms as f64;
        if let Some(prev) = self.last_sample_rtt {
            let diff = (rtt_f64 - prev).abs();
            // RFC 3550: J(i) = J(i-1) + (|D(i-1,i)| - J(i-1)) / 16
            self.rfc3550_jitter += (diff - self.rfc3550_jitter) / 16.0;
        }
        self.last_sample_rtt = Some(rtt_f64);
        self.samples.push(rtt_ms);
    }

    /// Computes summary statistics including min, max, average, median (P50), P90, P99, and jitter.
    pub fn compute_summary(&self) -> Option<LatencySummary> {
        if self.samples.is_empty() {
            return None;
        }

        let mut sorted = self.samples.clone();
        sorted.sort_unstable();

        let count = sorted.len();
        let min_ms = sorted[0];
        let max_ms = sorted[count - 1];
        let sum: u64 = sorted.iter().map(|&v| v as u64).sum();
        let avg_ms = sum as f64 / count as f64;

        let p50_ms = percentile_sorted(&sorted, 0.50);
        let p90_ms = percentile_sorted(&sorted, 0.90);
        let p95_ms = percentile_sorted(&sorted, 0.95);
        let p99_ms = percentile_sorted(&sorted, 0.99);

        // Calculate variance and standard deviation
        let variance = sorted
            .iter()
            .map(|&v| {
                let diff = v as f64 - avg_ms;
                diff * diff
            })
            .sum::<f64>()
            / count as f64;
        let std_dev_ms = variance.sqrt();

        Some(LatencySummary {
            sample_count: count,
            min_ms,
            max_ms,
            avg_ms,
            p50_ms,
            p90_ms,
            p95_ms,
            p99_ms,
            std_dev_ms,
            jitter_ms: self.rfc3550_jitter,
        })
    }

    /// Clears all recorded samples.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.rfc3550_jitter = 0.0;
        self.last_sample_rtt = None;
    }
}

fn percentile_sorted(sorted: &[u32], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0] as f64;
    }
    let rank = p * (sorted.len() - 1) as f64;
    let lower_idx = rank.floor() as usize;
    let upper_idx = rank.ceil() as usize;
    if lower_idx == upper_idx {
        sorted[lower_idx] as f64
    } else {
        let frac = rank - lower_idx as f64;
        (sorted[lower_idx] as f64) * (1.0 - frac) + (sorted[upper_idx] as f64) * frac
    }
}

/// Latency statistics summary for a monitored node or network path.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LatencySummary {
    pub sample_count: usize,
    pub min_ms: u32,
    pub max_ms: u32,
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub std_dev_ms: f64,
    pub jitter_ms: f64,
}

/// Event descriptor for a single audited network traffic flow.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FlowEvent {
    pub process: Option<String>,
    pub node: Option<String>,
    pub group: Option<String>,
    pub domain: Option<String>,
    pub country_code: Option<String>,
    pub route: AuditRouteType,
    pub reject_reason: Option<AuditRejectReason>,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub packets: u64,
    pub timestamp_secs: u64,
}

impl FlowEvent {
    /// Creates a basic flow event with given routing type and byte counts.
    pub fn new(route: AuditRouteType, upload_bytes: u64, download_bytes: u64) -> Self {
        Self {
            process: None,
            node: None,
            group: None,
            domain: None,
            country_code: None,
            route,
            reject_reason: None,
            upload_bytes,
            download_bytes,
            packets: 0,
            timestamp_secs: current_unix_secs(),
        }
    }

    /// Attaches originating process name.
    pub fn with_process(mut self, process: impl Into<String>) -> Self {
        self.process = Some(process.into());
        self
    }

    /// Attaches outbound node name.
    pub fn with_node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    /// Attaches rule group name.
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Attaches target domain name.
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Attaches destination ISO country code (e.g. "US", "HK", "JP").
    pub fn with_country(mut self, country_code: impl Into<String>) -> Self {
        self.country_code = Some(country_code.into().to_uppercase());
        self
    }

    /// Attaches reject reason when route is Reject.
    pub fn with_reject_reason(mut self, reason: AuditRejectReason) -> Self {
        self.reject_reason = Some(reason);
        self
    }

    /// Attaches packet count.
    pub fn with_packets(mut self, packets: u64) -> Self {
        self.packets = packets;
        self
    }

    /// Overrides the event timestamp in unix epoch seconds.
    pub fn with_timestamp(mut self, timestamp_secs: u64) -> Self {
        self.timestamp_secs = timestamp_secs;
        self
    }
}

/// A timeseries traffic accumulator recording per-process, per-node, per-group,
/// per-domain, and per-country hourly and daily traffic statistics with comprehensive reporting.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TrafficAuditAccumulator {
    // Hourly buckets: ((Entity Name, Hour Bucket Timestamp), TrafficStats)
    process_hourly: HashMap<(String, u64), TrafficStats>,
    node_hourly: HashMap<(String, u64), TrafficStats>,
    group_hourly: HashMap<(String, u64), TrafficStats>,
    domain_hourly: HashMap<(String, u64), TrafficStats>,
    country_hourly: HashMap<(String, u64), TrafficStats>,

    // Daily buckets: ((Entity Name, Day Bucket Timestamp), TrafficStats)
    process_daily: HashMap<(String, u64), TrafficStats>,
    node_daily: HashMap<(String, u64), TrafficStats>,
    group_daily: HashMap<(String, u64), TrafficStats>,
    domain_daily: HashMap<(String, u64), TrafficStats>,
    country_daily: HashMap<(String, u64), TrafficStats>,

    // Lifetime entity summaries
    process_totals: HashMap<String, TrafficStats>,
    node_totals: HashMap<String, TrafficStats>,
    group_totals: HashMap<String, TrafficStats>,
    domain_totals: HashMap<String, TrafficStats>,
    country_totals: HashMap<String, TrafficStats>,
    route_totals: HashMap<AuditRouteType, TrafficStats>,
    reject_reason_totals: HashMap<AuditRejectReason, TrafficStats>,

    // Real-time rate estimator
    #[serde(skip)]
    rate_estimator: EwmaRateEstimator,
}

impl TrafficAuditAccumulator {
    /// Creates a new, empty traffic audit accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a traffic flow event across all relevant entity and timeseries dimensions.
    pub fn record_flow(&mut self, event: &FlowEvent) {
        let ts = event.timestamp_secs;
        let hour_bucket = ts - (ts % 3600);
        let day_bucket = ts - (ts % 86400);
        let up = event.upload_bytes;
        let down = event.download_bytes;
        let pkts = event.packets;

        // Record Route Totals
        self.route_totals
            .entry(event.route)
            .or_default()
            .record(up, down, pkts);

        // Record Reject Reason if present
        if let Some(reason) = event.reject_reason {
            self.reject_reason_totals
                .entry(reason)
                .or_default()
                .record(up, down, pkts);
        }

        // Record Process Dimensions
        if let Some(ref proc) = event.process {
            self.process_hourly
                .entry((proc.clone(), hour_bucket))
                .or_default()
                .record(up, down, pkts);
            self.process_daily
                .entry((proc.clone(), day_bucket))
                .or_default()
                .record(up, down, pkts);
            self.process_totals
                .entry(proc.clone())
                .or_default()
                .record(up, down, pkts);
        }

        // Record Node Dimensions
        if let Some(ref nd) = event.node {
            self.node_hourly
                .entry((nd.clone(), hour_bucket))
                .or_default()
                .record(up, down, pkts);
            self.node_daily
                .entry((nd.clone(), day_bucket))
                .or_default()
                .record(up, down, pkts);
            self.node_totals
                .entry(nd.clone())
                .or_default()
                .record(up, down, pkts);
        }

        // Record Group Dimensions
        if let Some(ref grp) = event.group {
            self.group_hourly
                .entry((grp.clone(), hour_bucket))
                .or_default()
                .record(up, down, pkts);
            self.group_daily
                .entry((grp.clone(), day_bucket))
                .or_default()
                .record(up, down, pkts);
            self.group_totals
                .entry(grp.clone())
                .or_default()
                .record(up, down, pkts);
        }

        // Record Domain Dimensions
        if let Some(ref dom) = event.domain {
            self.domain_hourly
                .entry((dom.clone(), hour_bucket))
                .or_default()
                .record(up, down, pkts);
            self.domain_daily
                .entry((dom.clone(), day_bucket))
                .or_default()
                .record(up, down, pkts);
            self.domain_totals
                .entry(dom.clone())
                .or_default()
                .record(up, down, pkts);
        }

        // Record Country Dimensions
        if let Some(ref cc) = event.country_code {
            self.country_hourly
                .entry((cc.clone(), hour_bucket))
                .or_default()
                .record(up, down, pkts);
            self.country_daily
                .entry((cc.clone(), day_bucket))
                .or_default()
                .record(up, down, pkts);
            self.country_totals
                .entry(cc.clone())
                .or_default()
                .record(up, down, pkts);
        }

        // Update real-time EWMA rate
        let total_traffic = self.total_traffic_bytes();
        self.rate_estimator.update(ts as f64, total_traffic);
    }

    /// Convenience method to record simple process-level traffic.
    pub fn record_process_flow(
        &mut self,
        process: &str,
        up_bytes: u64,
        down_bytes: u64,
        packets: u64,
        route: AuditRouteType,
    ) {
        let event = FlowEvent::new(route, up_bytes, down_bytes)
            .with_process(process)
            .with_packets(packets);
        self.record_flow(&event);
    }

    /// Returns top processes sorted by total traffic volume in descending order.
    pub fn top_processes_by_traffic(&self, limit: usize) -> Vec<(String, u64)> {
        Self::rank_entities(&self.process_totals, limit)
    }

    /// Returns top outbound nodes sorted by total traffic volume in descending order.
    pub fn top_nodes_by_traffic(&self, limit: usize) -> Vec<(String, u64)> {
        Self::rank_entities(&self.node_totals, limit)
    }

    /// Returns top rule groups sorted by total traffic volume in descending order.
    pub fn top_groups_by_traffic(&self, limit: usize) -> Vec<(String, u64)> {
        Self::rank_entities(&self.group_totals, limit)
    }

    /// Returns top target domains sorted by total traffic volume in descending order.
    pub fn top_domains_by_traffic(&self, limit: usize) -> Vec<(String, u64)> {
        Self::rank_entities(&self.domain_totals, limit)
    }

    /// Returns top countries sorted by total traffic volume in descending order.
    pub fn top_countries_by_traffic(&self, limit: usize) -> Vec<(String, u64)> {
        Self::rank_entities(&self.country_totals, limit)
    }

    fn rank_entities(totals: &HashMap<String, TrafficStats>, limit: usize) -> Vec<(String, u64)> {
        let mut list: Vec<(String, u64)> = totals
            .iter()
            .map(|(k, stats)| (k.clone(), stats.total_bytes()))
            .collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        list.truncate(limit);
        list
    }

    /// Computes the ratio of Direct Bypass traffic vs Proxied traffic: `(bypass_ratio, proxied_ratio)`.
    pub fn bypass_vs_proxied_ratio(&self) -> (f64, f64) {
        let bypass_bytes = self.total_direct_bytes() as f64;
        let proxied_bytes = self.total_proxied_bytes() as f64;
        let total = bypass_bytes + proxied_bytes;
        if total <= 0.0 {
            (0.0, 0.0)
        } else {
            (bypass_bytes / total, proxied_bytes / total)
        }
    }

    /// Returns the fraction of direct bypass traffic relative to total proxied + direct traffic.
    pub fn direct_bypass_ratio(&self) -> f64 {
        self.bypass_vs_proxied_ratio().0
    }

    /// Returns total bytes transferred through proxied outbound connections.
    pub fn total_proxied_bytes(&self) -> u64 {
        self.route_totals
            .get(&AuditRouteType::Proxied)
            .map(|s| s.total_bytes())
            .unwrap_or(0)
    }

    /// Returns total bytes transferred directly bypassing proxy routing.
    pub fn total_direct_bytes(&self) -> u64 {
        self.route_totals
            .get(&AuditRouteType::DirectBypass)
            .map(|s| s.total_bytes())
            .unwrap_or(0)
    }

    /// Returns total bytes blocked or rejected.
    pub fn total_reject_bytes(&self) -> u64 {
        self.route_totals
            .get(&AuditRouteType::Reject)
            .map(|s| s.total_bytes())
            .unwrap_or(0)
    }

    /// Returns total recorded traffic across all routes.
    pub fn total_traffic_bytes(&self) -> u64 {
        self.route_totals.values().map(|s| s.total_bytes()).sum()
    }

    /// Current real-time transfer rate in Megabits per second (Mbps).
    pub fn current_mbps(&self) -> f64 {
        self.rate_estimator.mbps()
    }

    /// Peak transfer rate observed in Megabits per second (Mbps).
    pub fn peak_mbps(&self) -> f64 {
        self.rate_estimator.peak_mbps()
    }

    /// Retrieves lifetime traffic metrics for a specific process.
    pub fn get_process_traffic(&self, process: &str) -> Option<TrafficStats> {
        self.process_totals.get(process).copied()
    }

    /// Retrieves lifetime traffic metrics for a specific outbound node.
    pub fn get_node_traffic(&self, node: &str) -> Option<TrafficStats> {
        self.node_totals.get(node).copied()
    }

    /// Retrieves lifetime traffic metrics for a specific rule group.
    pub fn get_group_traffic(&self, group: &str) -> Option<TrafficStats> {
        self.group_totals.get(group).copied()
    }

    /// Retrieves lifetime traffic metrics for a specific domain.
    pub fn get_domain_traffic(&self, domain: &str) -> Option<TrafficStats> {
        self.domain_totals.get(domain).copied()
    }

    /// Retrieves lifetime traffic metrics for a specific country code.
    pub fn get_country_traffic(&self, country: &str) -> Option<TrafficStats> {
        self.country_totals.get(country).copied()
    }

    /// Returns hourly timeseries metrics for a process within `[start_hour, end_hour]`.
    pub fn get_process_hourly_series(
        &self,
        process: &str,
        start_hour: u64,
        end_hour: u64,
    ) -> Vec<(u64, TrafficStats)> {
        Self::filter_series(&self.process_hourly, process, start_hour, end_hour)
    }

    /// Returns daily timeseries metrics for an outbound node within `[start_day, end_day]`.
    pub fn get_node_daily_series(
        &self,
        node: &str,
        start_day: u64,
        end_day: u64,
    ) -> Vec<(u64, TrafficStats)> {
        Self::filter_series(&self.node_daily, node, start_day, end_day)
    }

    /// Returns hourly timeseries metrics for a domain within `[start_hour, end_hour]`.
    pub fn get_domain_hourly_series(
        &self,
        domain: &str,
        start_hour: u64,
        end_hour: u64,
    ) -> Vec<(u64, TrafficStats)> {
        Self::filter_series(&self.domain_hourly, domain, start_hour, end_hour)
    }

    fn filter_series(
        buckets: &HashMap<(String, u64), TrafficStats>,
        entity: &str,
        start: u64,
        end: u64,
    ) -> Vec<(u64, TrafficStats)> {
        let mut series: Vec<_> = buckets
            .iter()
            .filter(|((e, ts), _)| e == entity && *ts >= start && *ts <= end)
            .map(|((_, ts), stats)| (*ts, *stats))
            .collect();
        series.sort_by_key(|(ts, _)| *ts);
        series
    }

    /// Prunes timeseries buckets older than the specified timestamp cutoff.
    pub fn prune_older_than(&mut self, cutoff_secs: u64) -> usize {
        let hour_cutoff = cutoff_secs - (cutoff_secs % 3600);
        let day_cutoff = cutoff_secs - (cutoff_secs % 86400);

        let initial = self.process_hourly.len()
            + self.node_hourly.len()
            + self.group_hourly.len()
            + self.domain_hourly.len()
            + self.country_hourly.len()
            + self.process_daily.len()
            + self.node_daily.len()
            + self.group_daily.len()
            + self.domain_daily.len()
            + self.country_daily.len();

        self.process_hourly.retain(|(_, h), _| *h >= hour_cutoff);
        self.node_hourly.retain(|(_, h), _| *h >= hour_cutoff);
        self.group_hourly.retain(|(_, h), _| *h >= hour_cutoff);
        self.domain_hourly.retain(|(_, h), _| *h >= hour_cutoff);
        self.country_hourly.retain(|(_, h), _| *h >= hour_cutoff);

        self.process_daily.retain(|(_, d), _| *d >= day_cutoff);
        self.node_daily.retain(|(_, d), _| *d >= day_cutoff);
        self.group_daily.retain(|(_, d), _| *d >= day_cutoff);
        self.domain_daily.retain(|(_, d), _| *d >= day_cutoff);
        self.country_daily.retain(|(_, d), _| *d >= day_cutoff);

        let remaining = self.process_hourly.len()
            + self.node_hourly.len()
            + self.group_hourly.len()
            + self.domain_hourly.len()
            + self.country_hourly.len()
            + self.process_daily.len()
            + self.node_daily.len()
            + self.group_daily.len()
            + self.domain_daily.len()
            + self.country_daily.len();

        initial.saturating_sub(remaining)
    }

    /// Generates standard Prometheus metrics string for telemetry monitoring.
    pub fn export_prometheus_metrics(&self) -> String {
        let mut out = String::new();
        out.push_str("# HELP infiltrator_traffic_bytes_total Total bytes transferred by route\n");
        out.push_str("# TYPE infiltrator_traffic_bytes_total counter\n");
        for (route, stats) in &self.route_totals {
            let label = match route {
                AuditRouteType::Proxied => "proxied",
                AuditRouteType::DirectBypass => "direct",
                AuditRouteType::Reject => "reject",
                AuditRouteType::Other => "other",
            };
            out.push_str(&format!(
                "infiltrator_traffic_bytes_total{{route=\"{}\",direction=\"upload\"}} {}\n",
                label, stats.upload_bytes
            ));
            out.push_str(&format!(
                "infiltrator_traffic_bytes_total{{route=\"{}\",direction=\"download\"}} {}\n",
                label, stats.download_bytes
            ));
            out.push_str(&format!(
                "infiltrator_traffic_flows_total{{route=\"{}\"}} {}\n",
                label, stats.flow_count
            ));
        }

        out.push_str("# HELP infiltrator_bandwidth_rate_mbps Instantaneous bandwidth estimate\n");
        out.push_str("# TYPE infiltrator_bandwidth_rate_mbps gauge\n");
        out.push_str(&format!(
            "infiltrator_bandwidth_rate_mbps {:.4}\n",
            self.current_mbps()
        ));
        out.push_str(&format!(
            "infiltrator_bandwidth_peak_mbps {:.4}\n",
            self.peak_mbps()
        ));

        out
    }

    /// Captures a point-in-time snapshot of the audit accumulator state.
    pub fn snapshot(&self) -> TrafficAuditSnapshot {
        TrafficAuditSnapshot {
            timestamp_secs: current_unix_secs(),
            total_traffic_bytes: self.total_traffic_bytes(),
            total_proxied_bytes: self.total_proxied_bytes(),
            total_direct_bytes: self.total_direct_bytes(),
            total_reject_bytes: self.total_reject_bytes(),
            top_processes: self.top_processes_by_traffic(10),
            top_nodes: self.top_nodes_by_traffic(10),
            top_domains: self.top_domains_by_traffic(10),
            top_countries: self.top_countries_by_traffic(10),
        }
    }

    /// Clears all recorded statistics.
    pub fn clear(&mut self) {
        self.process_hourly.clear();
        self.node_hourly.clear();
        self.group_hourly.clear();
        self.domain_hourly.clear();
        self.country_hourly.clear();
        self.process_daily.clear();
        self.node_daily.clear();
        self.group_daily.clear();
        self.domain_daily.clear();
        self.country_daily.clear();
        self.process_totals.clear();
        self.node_totals.clear();
        self.group_totals.clear();
        self.domain_totals.clear();
        self.country_totals.clear();
        self.route_totals.clear();
        self.reject_reason_totals.clear();
        self.rate_estimator.reset();
    }
}

/// Point-in-time summary snapshot for client / API consumers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TrafficAuditSnapshot {
    pub timestamp_secs: u64,
    pub total_traffic_bytes: u64,
    pub total_proxied_bytes: u64,
    pub total_direct_bytes: u64,
    pub total_reject_bytes: u64,
    pub top_processes: Vec<(String, u64)>,
    pub top_nodes: Vec<(String, u64)>,
    pub top_domains: Vec<(String, u64)>,
    pub top_countries: Vec<(String, u64)>,
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
#[path = "traffic_audit_test.rs"]
mod traffic_audit_test;
