//! Network latency, bandwidth diagnostics, rate tracking, network throttling simulation, and privacy leak detection.

use std::time::Instant;

#[cfg(test)]
#[path = "diagnostics_test.rs"]
mod tests;

/// Snapshot of connection transfer rates at a given time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnectionRateSnapshot {
    pub up_speed: u64,
    pub down_speed: u64,
    pub total_up: u64,
    pub total_down: u64,
    pub peak_up_speed: u64,
    pub peak_down_speed: u64,
}

/// Tracks per-connection upload/download byte counters over time and computes instantaneous rates.
pub struct ConnectionRateTracker {
    total_up: u64,
    total_down: u64,
    last_snapshot_time: Instant,
    last_snapshot_up: u64,
    last_snapshot_down: u64,
    peak_up_speed: u64,
    peak_down_speed: u64,
}

impl Default for ConnectionRateTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionRateTracker {
    pub fn new() -> Self {
        Self::new_with_time(Instant::now())
    }

    pub fn new_with_time(now: Instant) -> Self {
        Self {
            total_up: 0,
            total_down: 0,
            last_snapshot_time: now,
            last_snapshot_up: 0,
            last_snapshot_down: 0,
            peak_up_speed: 0,
            peak_down_speed: 0,
        }
    }

    pub fn add_up(&mut self, bytes: u64) {
        self.total_up += bytes;
    }

    pub fn add_down(&mut self, bytes: u64) {
        self.total_down += bytes;
    }

    pub fn snapshot(&mut self) -> ConnectionRateSnapshot {
        self.snapshot_with_time(Instant::now())
    }

    pub fn snapshot_with_time(&mut self, now: Instant) -> ConnectionRateSnapshot {
        let elapsed = now.saturating_duration_since(self.last_snapshot_time).as_secs_f64();
        let mut up_speed = 0;
        let mut down_speed = 0;
        if elapsed > 0.0 {
            let up_diff = self.total_up.saturating_sub(self.last_snapshot_up);
            let down_diff = self.total_down.saturating_sub(self.last_snapshot_down);
            up_speed = (up_diff as f64 / elapsed) as u64;
            down_speed = (down_diff as f64 / elapsed) as u64;
        }

        self.peak_up_speed = self.peak_up_speed.max(up_speed);
        self.peak_down_speed = self.peak_down_speed.max(down_speed);
        self.last_snapshot_time = now;
        self.last_snapshot_up = self.total_up;
        self.last_snapshot_down = self.total_down;

        ConnectionRateSnapshot {
            up_speed,
            down_speed,
            total_up: self.total_up,
            total_down: self.total_down,
            peak_up_speed: self.peak_up_speed,
            peak_down_speed: self.peak_down_speed,
        }
    }
}

/// Statistics calculated from a series of latency measurements.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JitterStats {
    pub mean_latency_ms: f64,
    pub jitter_ms: f64,
    pub std_dev_ms: f64,
    pub loss_rate_percent: f64,
    pub sample_count: usize,
}

/// Calculates latency jitter, standard deviation, and packet loss rate.
pub struct JitterCalculator {
    latencies: Vec<f64>,
    failures: usize,
    total_attempts: usize,
}

impl Default for JitterCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterCalculator {
    pub fn new() -> Self {
        Self { latencies: Vec::new(), failures: 0, total_attempts: 0 }
    }

    pub fn record_success(&mut self, latency_ms: f64) {
        self.latencies.push(latency_ms);
        self.total_attempts += 1;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.total_attempts += 1;
    }

    pub fn calculate(&self) -> JitterStats {
        let sample_count = self.latencies.len();
        let loss_rate_percent = if self.total_attempts > 0 {
            (self.failures as f64 / self.total_attempts as f64) * 100.0
        } else {
            0.0
        };

        if sample_count == 0 {
            return JitterStats {
                mean_latency_ms: 0.0,
                jitter_ms: 0.0,
                std_dev_ms: 0.0,
                loss_rate_percent,
                sample_count: self.total_attempts,
            };
        }

        let mean_latency_ms = self.latencies.iter().sum::<f64>() / sample_count as f64;
        let mut jitter_ms = 0.0;
        if sample_count > 1 {
            let mut sum_diff = 0.0;
            for i in 1..sample_count {
                sum_diff += (self.latencies[i] - self.latencies[i - 1]).abs();
            }
            jitter_ms = sum_diff / (sample_count - 1) as f64;
        }

        let mut std_dev_ms = 0.0;
        if sample_count > 1 {
            let variance = self
                .latencies
                .iter()
                .map(|&x| (x - mean_latency_ms).powi(2))
                .sum::<f64>()
                / (sample_count - 1) as f64;
            std_dev_ms = variance.sqrt();
        }

        JitterStats {
            mean_latency_ms,
            jitter_ms,
            std_dev_ms,
            loss_rate_percent,
            sample_count: self.total_attempts,
        }
    }
}

/// Report containing metadata about an outbound IP address.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutboundIpReport {
    pub ip: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub asn: Option<u32>,
    pub isp: Option<String>,
    pub fraud_score: Option<u8>,
}

/// Tracks DNS queries, cache hits, fake-ip pool capacity and computes metrics.
pub struct DnsMetricsTracker {
    queries: usize,
    cache_hits: usize,
    fake_ip_capacity: usize,
    response_times: Vec<f64>,
}

impl DnsMetricsTracker {
    pub fn new(fake_ip_capacity: usize) -> Self {
        Self { queries: 0, cache_hits: 0, fake_ip_capacity, response_times: Vec::new() }
    }

    pub fn record_query(&mut self, is_hit: bool, response_time_ms: f64) {
        self.queries += 1;
        if is_hit { self.cache_hits += 1; }
        self.response_times.push(response_time_ms);
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.queries == 0 { 0.0 } else { self.cache_hits as f64 / self.queries as f64 }
    }

    pub fn average_response_time_ms(&self) -> f64 {
        if self.response_times.is_empty() { 0.0 } else { self.response_times.iter().sum::<f64>() / self.response_times.len() as f64 }
    }

    pub fn fake_ip_capacity(&self) -> usize { self.fake_ip_capacity }
}

/// Multi-dimensional connection timing breakdown (DNS, TCP, TLS, First Byte).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConnectionTimingBreakdown {
    pub connection_id: String,
    pub host: String,
    pub dns_lookup_ms: Option<u32>,
    pub tcp_handshake_ms: Option<u32>,
    pub tls_handshake_ms: Option<u32>,
    pub ttfb_ms: Option<u32>,
    pub total_duration_ms: u32,
    pub chain: Vec<String>,
}

impl ConnectionTimingBreakdown {
    pub fn new(connection_id: &str, host: &str) -> Self {
        Self {
            connection_id: connection_id.to_string(),
            host: host.to_string(),
            dns_lookup_ms: None,
            tcp_handshake_ms: None,
            tls_handshake_ms: None,
            ttfb_ms: None,
            total_duration_ms: 0,
            chain: Vec::new(),
        }
    }
}

/// Downstream bandwidth measurement result for node speedtesting.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpeedtestResult {
    pub proxy_name: String,
    pub target_url: String,
    pub duration_ms: u64,
    pub total_bytes: u64,
    pub bandwidth_mbps: f64,
}

pub struct SpeedtestCalculator;

impl SpeedtestCalculator {
    pub fn calculate_bandwidth(total_bytes: u64, duration_ms: u64) -> f64 {
        if duration_ms == 0 { return 0.0; }
        let duration_secs = duration_ms as f64 / 1000.0;
        let bits = total_bytes as f64 * 8.0;
        let mbps = (bits / (1024.0 * 1024.0)) / duration_secs;
        (mbps * 100.0).round() / 100.0
    }
}

/// Predefined or custom network condition profiles for traffic throttling simulation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkThrottlingProfile {
    Profile2G,
    Profile3G,
    Profile4G,
    ProfileDSL,
    ProfileSatellite,
    Custom {
        max_down_kbps: u64,
        max_up_kbps: u64,
        delay_ms: u64,
        jitter_ms: u64,
        loss_percent: f64,
    },
}

impl NetworkThrottlingProfile {
    pub fn max_down_kbps(&self) -> u64 {
        match self {
            Self::Profile2G => 250,
            Self::Profile3G => 1_600,
            Self::Profile4G => 10_000,
            Self::ProfileDSL | Self::ProfileSatellite => 2_000,
            Self::Custom { max_down_kbps, .. } => *max_down_kbps,
        }
    }

    pub fn max_up_kbps(&self) -> u64 {
        match self {
            Self::Profile2G => 50,
            Self::Profile3G => 750,
            Self::Profile4G => 3_000,
            Self::ProfileDSL | Self::ProfileSatellite => 512,
            Self::Custom { max_up_kbps, .. } => *max_up_kbps,
        }
    }

    pub fn delay_ms(&self) -> u64 {
        match self {
            Self::Profile2G => 300,
            Self::Profile3G => 100,
            Self::Profile4G => 20,
            Self::ProfileDSL => 5,
            Self::ProfileSatellite => 600,
            Self::Custom { delay_ms, .. } => *delay_ms,
        }
    }

    pub fn jitter_ms(&self) -> u64 {
        match self {
            Self::Profile2G => 50,
            Self::Profile3G => 20,
            Self::Profile4G => 5,
            Self::ProfileDSL => 1,
            Self::ProfileSatellite => 40,
            Self::Custom { jitter_ms, .. } => *jitter_ms,
        }
    }

    pub fn loss_percent(&self) -> f64 {
        match self {
            Self::Profile2G => 3.0,
            Self::Profile3G => 1.0,
            Self::Profile4G | Self::ProfileDSL => 0.0,
            Self::ProfileSatellite => 2.0,
            Self::Custom { loss_percent, .. } => *loss_percent,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Profile2G => "2G",
            Self::Profile3G => "3G",
            Self::Profile4G => "4G",
            Self::ProfileDSL => "DSL",
            Self::ProfileSatellite => "Satellite",
            Self::Custom { .. } => "Custom",
        }
    }

    pub fn custom(max_down_kbps: u64, max_up_kbps: u64, delay_ms: u64, jitter_ms: u64, loss_percent: f64) -> Self {
        Self::Custom { max_down_kbps, max_up_kbps, delay_ms, jitter_ms, loss_percent }
    }
}

/// Token bucket algorithm implementation for simulated bandwidth rate limiting.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenBucket {
    capacity_bytes: f64,
    available_tokens: f64,
    rate_bytes_per_sec: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(rate_kbps: u64, burst_capacity_bytes: u64) -> Self {
        Self::new_with_time(rate_kbps, burst_capacity_bytes, Instant::now())
    }

    pub fn new_with_time(rate_kbps: u64, burst_capacity_bytes: u64, now: Instant) -> Self {
        Self {
            capacity_bytes: burst_capacity_bytes as f64,
            available_tokens: burst_capacity_bytes as f64,
            rate_bytes_per_sec: (rate_kbps as f64 * 1000.0) / 8.0,
            last_refill: now,
        }
    }

    pub fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill).as_secs_f64();
        if elapsed > 0.0 {
            self.available_tokens = (self.available_tokens + elapsed * self.rate_bytes_per_sec).min(self.capacity_bytes);
            self.last_refill = now;
        }
    }

    pub fn try_consume(&mut self, bytes: u64, now: Instant) -> bool {
        self.refill(now);
        let required = bytes as f64;
        if self.available_tokens >= required {
            self.available_tokens -= required;
            true
        } else {
            false
        }
    }

    pub fn consume_or_wait(&mut self, bytes: u64, now: Instant) -> std::time::Duration {
        self.refill(now);
        let required = bytes as f64;
        if self.available_tokens >= required {
            self.available_tokens -= required;
            std::time::Duration::ZERO
        } else {
            let deficit = required - self.available_tokens;
            let wait_secs = if self.rate_bytes_per_sec > 0.0 { deficit / self.rate_bytes_per_sec } else { 0.0 };
            self.available_tokens = 0.0;
            std::time::Duration::from_secs_f64(wait_secs)
        }
    }

    pub fn available_tokens(&self) -> f64 { self.available_tokens }
    pub fn capacity_bytes(&self) -> f64 { self.capacity_bytes }
    pub fn rate_bytes_per_sec(&self) -> f64 { self.rate_bytes_per_sec }
}

/// Throttling calculator managing delay injection, jitter modeling, and token-bucket bandwidth limiting.
#[derive(Debug, Clone)]
pub struct ThrottlingCalculator {
    profile: NetworkThrottlingProfile,
    down_bucket: TokenBucket,
    up_bucket: TokenBucket,
}

impl ThrottlingCalculator {
    pub fn new(profile: NetworkThrottlingProfile) -> Self {
        Self::new_with_time(profile, Instant::now())
    }

    pub fn new_with_time(profile: NetworkThrottlingProfile, now: Instant) -> Self {
        let down_rate = profile.max_down_kbps();
        let up_rate = profile.max_up_kbps();
        let down_burst = (down_rate * 1000 / 8).max(64 * 1024);
        let up_burst = (up_rate * 1000 / 8).max(32 * 1024);
        Self {
            down_bucket: TokenBucket::new_with_time(down_rate, down_burst, now),
            up_bucket: TokenBucket::new_with_time(up_rate, up_burst, now),
            profile,
        }
    }

    pub fn profile(&self) -> &NetworkThrottlingProfile { &self.profile }

    pub fn set_profile(&mut self, profile: NetworkThrottlingProfile, now: Instant) {
        let down_rate = profile.max_down_kbps();
        let up_rate = profile.max_up_kbps();
        let down_burst = (down_rate * 1000 / 8).max(64 * 1024);
        let up_burst = (up_rate * 1000 / 8).max(32 * 1024);
        self.profile = profile;
        self.down_bucket = TokenBucket::new_with_time(down_rate, down_burst, now);
        self.up_bucket = TokenBucket::new_with_time(up_rate, up_burst, now);
    }

    pub fn calculate_delay(base_delay_ms: u64, jitter_ms: u64, jitter_sample: f64) -> u64 {
        let clamped = jitter_sample.clamp(-1.0, 1.0);
        let offset = (jitter_ms as f64 * clamped).round() as i64;
        (base_delay_ms as i64).saturating_add(offset).max(0) as u64
    }

    pub fn compute_injected_delay(&self, jitter_sample: f64) -> u64 {
        Self::calculate_delay(self.profile.delay_ms(), self.profile.jitter_ms(), jitter_sample)
    }

    pub fn calculate_transmission_delay_ms(bytes: u64, rate_kbps: u64) -> f64 {
        if rate_kbps == 0 { 0.0 } else { (bytes as f64 * 8.0) / (rate_kbps as f64) }
    }

    pub fn calculate_downlink_transmission_delay_ms(&self, bytes: u64) -> f64 {
        Self::calculate_transmission_delay_ms(bytes, self.profile.max_down_kbps())
    }

    pub fn calculate_uplink_transmission_delay_ms(&self, bytes: u64) -> f64 {
        Self::calculate_transmission_delay_ms(bytes, self.profile.max_up_kbps())
    }

    pub fn should_drop_packet(loss_percent: f64, random_roll: f64) -> bool {
        loss_percent > 0.0 && random_roll < loss_percent
    }

    pub fn is_packet_dropped(&self, random_roll: f64) -> bool {
        Self::should_drop_packet(self.profile.loss_percent(), random_roll)
    }

    pub fn try_consume_downlink(&mut self, bytes: u64, now: Instant) -> bool {
        self.down_bucket.try_consume(bytes, now)
    }

    pub fn try_consume_uplink(&mut self, bytes: u64, now: Instant) -> bool {
        self.up_bucket.try_consume(bytes, now)
    }

    pub fn compute_downlink_wait(&mut self, bytes: u64, now: Instant) -> std::time::Duration {
        self.down_bucket.consume_or_wait(bytes, now)
    }

    pub fn compute_uplink_wait(&mut self, bytes: u64, now: Instant) -> std::time::Duration {
        self.up_bucket.consume_or_wait(bytes, now)
    }

    pub fn down_bucket(&self) -> &TokenBucket { &self.down_bucket }
    pub fn up_bucket(&self) -> &TokenBucket { &self.up_bucket }
    pub fn down_bucket_mut(&mut self) -> &mut TokenBucket { &mut self.down_bucket }
    pub fn up_bucket_mut(&mut self) -> &mut TokenBucket { &mut self.up_bucket }
}

/// Comprehensive outcome of privacy leak evaluation across all vectors.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeakTestOutcome {
    pub dns_leak: bool,
    pub webrtc_leak: bool,
    pub ipv6_leak: bool,
    pub fake_ip_bypass: bool,
    pub details: Vec<String>,
}

impl LeakTestOutcome {
    pub fn new() -> Self { Self::default() }
    pub fn is_clean(&self) -> bool { !self.dns_leak && !self.webrtc_leak && !self.ipv6_leak && !self.fake_ip_bypass }
    pub fn has_any_leak(&self) -> bool { !self.is_clean() }
    pub fn total_leaks_count(&self) -> usize {
        let mut count = 0;
        if self.dns_leak { count += 1; }
        if self.webrtc_leak { count += 1; }
        if self.ipv6_leak { count += 1; }
        if self.fake_ip_bypass { count += 1; }
        count
    }
}

/// Audited network connection descriptor for privacy leak analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticConnection {
    pub id: String,
    pub network: String,
    pub source_ip: String,
    pub destination_ip: String,
    pub destination_port: u16,
    pub host: String,
    pub rule: String,
    pub chains: Vec<String>,
    pub process_path: Option<String>,
}

impl DiagnosticConnection {
    pub fn new(id: impl Into<String>, destination_ip: impl Into<String>, destination_port: u16, rule: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            network: "tcp".to_string(),
            source_ip: "127.0.0.1".to_string(),
            destination_ip: destination_ip.into(),
            destination_port,
            host: String::new(),
            rule: rule.into(),
            chains: Vec::new(),
            process_path: None,
        }
    }

    pub fn with_network(mut self, network: impl Into<String>) -> Self { self.network = network.into(); self }
    pub fn with_host(mut self, host: impl Into<String>) -> Self { self.host = host.into(); self }
    pub fn with_chains(mut self, chains: Vec<String>) -> Self { self.chains = chains; self }
    pub fn with_process_path(mut self, path: impl Into<String>) -> Self { self.process_path = Some(path.into()); self }
}

impl From<&mihomo_api::types::Connection> for DiagnosticConnection {
    fn from(c: &mihomo_api::types::Connection) -> Self {
        Self {
            id: c.id.clone(),
            network: c.metadata.network.clone(),
            source_ip: c.metadata.source_ip.clone(),
            destination_ip: c.metadata.destination_ip.clone(),
            destination_port: c.metadata.destination_port.parse().unwrap_or(0),
            host: c.metadata.host.clone(),
            rule: c.rule.clone(),
            chains: c.chains.clone(),
            process_path: if c.metadata.process_path.trim().is_empty() { None } else { Some(c.metadata.process_path.clone()) },
        }
    }
}

/// Recorded DNS resolution log event for leak detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DnsResolutionLog {
    pub query_domain: String,
    pub query_type: String,
    pub upstream_server: String,
    pub resolved_ips: Vec<String>,
    pub is_direct: bool,
    pub is_encrypted: bool,
    pub process_name: Option<String>,
}

impl DnsResolutionLog {
    pub fn new(domain: impl Into<String>, query_type: impl Into<String>, upstream_server: impl Into<String>) -> Self {
        Self {
            query_domain: domain.into(),
            query_type: query_type.into(),
            upstream_server: upstream_server.into(),
            resolved_ips: Vec::new(),
            is_direct: false,
            is_encrypted: false,
            process_name: None,
        }
    }

    pub fn with_resolved_ips(mut self, ips: Vec<String>) -> Self { self.resolved_ips = ips; self }
    pub fn with_direct(mut self, is_direct: bool) -> Self { self.is_direct = is_direct; self }
    pub fn with_encrypted(mut self, is_encrypted: bool) -> Self { self.is_encrypted = is_encrypted; self }
    pub fn with_process(mut self, process: impl Into<String>) -> Self { self.process_name = Some(process.into()); self }
}

/// Static evaluator analyzing connection traffic and DNS logs for privacy leaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyLeakDetectionSuite {
    fake_ip_cidr: String,
    stun_ports: Vec<u16>,
}

impl Default for PrivacyLeakDetectionSuite {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyLeakDetectionSuite {
    pub fn new() -> Self {
        Self {
            fake_ip_cidr: "198.18.0.0/15".to_string(),
            stun_ports: vec![3478, 5349, 19302, 19305],
        }
    }

    pub fn with_fake_ip_cidr(mut self, cidr: impl Into<String>) -> Self { self.fake_ip_cidr = cidr.into(); self }
    pub fn with_stun_ports(mut self, ports: Vec<u16>) -> Self { self.stun_ports = ports; self }

    pub fn evaluate(connections: &[DiagnosticConnection], dns_logs: &[DnsResolutionLog]) -> LeakTestOutcome {
        Self::new().evaluate_suite(connections, dns_logs)
    }

    pub fn evaluate_mihomo_connections(connections: &[mihomo_api::types::Connection], dns_logs: &[DnsResolutionLog]) -> LeakTestOutcome {
        let converted: Vec<DiagnosticConnection> = connections.iter().map(Into::into).collect();
        Self::evaluate(&converted, dns_logs)
    }

    pub fn evaluate_suite(&self, connections: &[DiagnosticConnection], dns_logs: &[DnsResolutionLog]) -> LeakTestOutcome {
        let mut outcome = LeakTestOutcome::default();
        for conn in connections { self.check_connection(conn, &mut outcome); }
        for log in dns_logs { self.check_dns_log(log, &mut outcome); }
        outcome
    }

    pub fn check_connection(&self, conn: &DiagnosticConnection, outcome: &mut LeakTestOutcome) {
        let is_direct = is_direct_routing(&conn.rule, &conn.chains);
        let proc_label = conn.process_path.as_deref().unwrap_or("unknown");

        // 1. Fake-IP Bypass check: connection to a Fake-IP address routed DIRECT
        if is_direct && !conn.destination_ip.is_empty() && crate::dns_tester::DnsTester::check_fake_ip_range(&conn.destination_ip, &self.fake_ip_cidr) {
            outcome.fake_ip_bypass = true;
            outcome.details.push(format!(
                "Fake-IP Bypass: Connection '{}' to Fake-IP {} from '{}' was routed DIRECT instead of through proxy tunnel",
                conn.id, conn.destination_ip, proc_label
            ));
        }

        // 2. DNS Leak check: plaintext DNS (port 53) routed DIRECT to external IP
        let is_dns_port = conn.destination_port == 53;
        let is_external = !is_loopback_or_empty(&conn.destination_ip);
        if is_direct && is_dns_port && is_external {
            outcome.dns_leak = true;
            outcome.details.push(format!(
                "DNS Leak: Plaintext DNS query on port 53 to {} from '{}' bypassed proxy via DIRECT route",
                conn.destination_ip, proc_label
            ));
        }

        // 3. WebRTC Leak check: STUN/TURN traffic routed DIRECT
        let is_stun_port = self.stun_ports.contains(&conn.destination_port);
        let is_stun_host = is_webrtc_stun_host(&conn.host);
        if is_direct && (is_stun_port || is_stun_host) {
            outcome.webrtc_leak = true;
            outcome.details.push(format!(
                "WebRTC Leak: STUN/TURN candidate traffic to {}:{} (host: '{}') from '{}' routed DIRECT, exposing real IP",
                conn.destination_ip, conn.destination_port, conn.host, proc_label
            ));
        }

        // 4. IPv6 Leak check: public IPv6 address routed DIRECT
        if is_direct && is_public_ipv6(&conn.destination_ip) {
            outcome.ipv6_leak = true;
            outcome.details.push(format!(
                "IPv6 Leak: Unproxied connection to public IPv6 {} from process '{}' routed DIRECT",
                conn.destination_ip, proc_label
            ));
        }
    }

    pub fn check_dns_log(&self, log: &DnsResolutionLog, outcome: &mut LeakTestOutcome) {
        let proc_label = log.process_name.as_deref().unwrap_or("unknown");

        // 1. DNS Leak: unencrypted direct query to external server
        if log.is_direct && !log.is_encrypted && !is_loopback_or_empty(&log.upstream_server) {
            outcome.dns_leak = true;
            outcome.details.push(format!(
                "DNS Leak: Unencrypted direct query for domain '{}' via external server '{}' from '{}'",
                log.query_domain, log.upstream_server, proc_label
            ));
        }

        // 2. WebRTC Leak: direct resolution of STUN server
        if log.is_direct && is_webrtc_stun_host(&log.query_domain) {
            outcome.webrtc_leak = true;
            outcome.details.push(format!(
                "WebRTC Leak: Direct DNS query for STUN host '{}' via upstream '{}'",
                log.query_domain, log.upstream_server
            ));
        }

        // 3. IPv6 Leak: AAAA query resolved directly or returning public IPv6
        if log.is_direct && (log.query_type.eq_ignore_ascii_case("AAAA") || log.resolved_ips.iter().any(|ip| is_public_ipv6(ip))) {
            outcome.ipv6_leak = true;
            outcome.details.push(format!(
                "IPv6 Leak: Direct DNS resolution for domain '{}' (type: '{}') exposing IPv6 addresses {:?}",
                log.query_domain, log.query_type, log.resolved_ips
            ));
        }

        // 4. Fake-IP Bypass in DNS log: domain resolved directly while answers are Fake-IP
        if log.is_direct && log.resolved_ips.iter().any(|ip| crate::dns_tester::DnsTester::check_fake_ip_range(ip, &self.fake_ip_cidr)) {
            outcome.fake_ip_bypass = true;
            outcome.details.push(format!(
                "Fake-IP Bypass: Direct resolution log for '{}' returned Fake-IP answers {:?}",
                log.query_domain, log.resolved_ips
            ));
        }
    }
}

fn is_direct_routing(rule: &str, chains: &[String]) -> bool {
    rule.eq_ignore_ascii_case("DIRECT") || chains.iter().any(|c| c.eq_ignore_ascii_case("DIRECT"))
}

fn is_loopback_or_empty(ip_or_addr: &str) -> bool {
    let host_part = if let Some(stripped) = ip_or_addr.strip_prefix("udp://") {
        stripped
    } else if let Some(stripped) = ip_or_addr.strip_prefix("tcp://") {
        stripped
    } else {
        ip_or_addr
    };
    let ip = host_part.split(':').next().unwrap_or(host_part);
    ip.is_empty() || ip == "127.0.0.1" || ip == "::1" || ip.starts_with("127.")
}

fn is_public_ipv6(ip_str: &str) -> bool {
    if let Ok(ip) = ip_str.parse::<std::net::Ipv6Addr>() {
        let segs = ip.segments();
        let is_link_local = (segs[0] & 0xffc0) == 0xfe80;
        let is_unique_local = (segs[0] & 0xfe00) == 0xfc00;
        let is_v4_mapped = segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0 && segs[4] == 0 && segs[5] == 0xffff;
        let is_doc = segs[0] == 0x2001 && segs[1] == 0x0db8;

        !ip.is_loopback() && !ip.is_unspecified() && !ip.is_multicast() && !is_link_local && !is_unique_local && !is_v4_mapped && !is_doc
    } else {
        false
    }
}

fn is_webrtc_stun_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    lower.starts_with("stun.")
        || lower.starts_with("turn.")
        || lower.contains(".stun.")
        || lower.contains(".turn.")
        || lower.ends_with(".stun.google.com")
        || lower.ends_with(".twilio.com")
        || lower == "stun.l.google.com"
}
