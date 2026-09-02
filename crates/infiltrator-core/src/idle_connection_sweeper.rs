use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Transport layer protocols for tracked network connections.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportProtocol {
    Tcp,
    Udp,
    Other,
}

impl TransportProtocol {
    /// Returns the canonical protocol identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Other => "other",
        }
    }
}

/// Lifecycle state for monitored network connections.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConnectionState {
    #[default]
    Active,
    Connecting,
    Idle,
    HalfClosed,
    Closing,
    Terminated,
}

/// Security audit classification tags for anomalous connection flows.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecurityAuditTag {
    SuspiciousDirectBypass,
    HighFrequencyChurn,
    QuotaViolation,
    ZombieLeak,
    PlaintextCredentials,
}

/// A monitored connection session with activity metrics, routing metadata, and real-time rate differentials.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TrackedConnection {
    pub id: String,
    pub protocol: TransportProtocol,
    pub src_addr: String,
    pub dst_addr: String,
    pub host: Option<String>,
    pub process_name: Option<String>,
    pub pid: Option<u32>,
    pub outbound_node: Option<String>,
    pub rule_group: Option<String>,
    pub state: ConnectionState,
    pub security_tags: Vec<SecurityAuditTag>,
    pub created_at_secs: u64,
    pub last_activity_secs: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub upload_rate_bps: f64,
    pub download_rate_bps: f64,
    pub peak_rate_bps: f64,
}

impl TrackedConnection {
    /// Constructs a basic tracked connection.
    pub fn new(
        id: impl Into<String>,
        protocol: TransportProtocol,
        src_addr: impl Into<String>,
        dst_addr: impl Into<String>,
        now_secs: u64,
    ) -> Self {
        Self {
            id: id.into(),
            protocol,
            src_addr: src_addr.into(),
            dst_addr: dst_addr.into(),
            host: None,
            process_name: None,
            pid: None,
            outbound_node: None,
            rule_group: None,
            state: ConnectionState::Active,
            security_tags: Vec::new(),
            created_at_secs: now_secs,
            last_activity_secs: now_secs,
            upload_bytes: 0,
            download_bytes: 0,
            upload_rate_bps: 0.0,
            download_rate_bps: 0.0,
            peak_rate_bps: 0.0,
        }
    }

    /// Attaches target hostname metadata.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Attaches originating process details.
    pub fn with_process(mut self, process_name: impl Into<String>, pid: Option<u32>) -> Self {
        self.process_name = Some(process_name.into());
        self.pid = pid;
        self
    }

    /// Attaches outbound node and rule group routing tags.
    pub fn with_routing(
        mut self,
        outbound_node: Option<String>,
        rule_group: Option<String>,
    ) -> Self {
        self.outbound_node = outbound_node;
        self.rule_group = rule_group;
        self
    }

    /// Attaches an initial security audit tag.
    pub fn with_security_tag(mut self, tag: SecurityAuditTag) -> Self {
        if !self.security_tags.contains(&tag) {
            self.security_tags.push(tag);
        }
        self
    }

    /// Returns the sum of uploaded and downloaded bytes.
    pub fn total_bytes(&self) -> u64 {
        self.upload_bytes.saturating_add(self.download_bytes)
    }

    /// Computes how many seconds have elapsed since the last recorded activity.
    pub fn idle_duration_secs(&self, now_secs: u64) -> u64 {
        now_secs.saturating_sub(self.last_activity_secs)
    }

    /// Returns `true` if this connection has zero bytes transferred since creation.
    pub fn is_zero_traffic(&self) -> bool {
        self.upload_bytes == 0 && self.download_bytes == 0
    }

    /// Returns `true` if this connection only has unidirectional traffic.
    pub fn is_half_closed(&self) -> bool {
        (self.upload_bytes > 0 && self.download_bytes == 0)
            || (self.upload_bytes == 0 && self.download_bytes > 0)
    }

    /// Updates activity timestamp, increments byte counters, and computes differential transfer rates.
    pub fn record_activity(&mut self, now_secs: u64, additional_up: u64, additional_down: u64) {
        let dt = now_secs.saturating_sub(self.last_activity_secs) as f64;
        if dt > 0.0 {
            self.upload_rate_bps = additional_up as f64 / dt;
            self.download_rate_bps = additional_down as f64 / dt;
            let current_total_bps = self.upload_rate_bps + self.download_rate_bps;
            if current_total_bps > self.peak_rate_bps {
                self.peak_rate_bps = current_total_bps;
            }
        }

        self.last_activity_secs = now_secs;
        self.upload_bytes = self.upload_bytes.saturating_add(additional_up);
        self.download_bytes = self.download_bytes.saturating_add(additional_down);
        self.state = ConnectionState::Active;
    }
}

/// Errors returned by `IdleConnectionSweeper` operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SweeperError {
    #[error("Maximum connection capacity ({0}) reached")]
    CapacityExceeded(usize),
    #[error("Per-process quota ({limit}) exceeded for process '{process}'")]
    ProcessQuotaExceeded { process: String, limit: usize },
    #[error("Per-node quota ({limit}) exceeded for node '{node}'")]
    NodeQuotaExceeded { node: String, limit: usize },
    #[error("Connection ID '{0}' already exists")]
    DuplicateId(String),
}

/// Rich builder DSL for querying active connections across multiple criteria.
#[derive(Debug, Clone, Default)]
pub struct ConnectionQuery {
    pub protocol: Option<TransportProtocol>,
    pub process: Option<String>,
    pub host_suffix: Option<String>,
    pub node: Option<String>,
    pub group: Option<String>,
    pub pid: Option<u32>,
    pub state: Option<ConnectionState>,
    pub min_idle_secs: Option<u64>,
    pub min_total_bytes: Option<u64>,
    pub has_security_tag: Option<SecurityAuditTag>,
}

impl ConnectionQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_protocol(mut self, protocol: TransportProtocol) -> Self {
        self.protocol = Some(protocol);
        self
    }

    pub fn with_process(mut self, process: impl Into<String>) -> Self {
        self.process = Some(process.into());
        self
    }

    pub fn with_host_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.host_suffix = Some(suffix.into());
        self
    }

    pub fn with_node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn with_state(mut self, state: ConnectionState) -> Self {
        self.state = Some(state);
        self
    }

    pub fn with_min_idle_secs(mut self, secs: u64) -> Self {
        self.min_idle_secs = Some(secs);
        self
    }

    pub fn with_min_total_bytes(mut self, bytes: u64) -> Self {
        self.min_total_bytes = Some(bytes);
        self
    }

    pub fn with_security_tag(mut self, tag: SecurityAuditTag) -> Self {
        self.has_security_tag = Some(tag);
        self
    }

    pub fn matches(&self, conn: &TrackedConnection, now_secs: u64) -> bool {
        if let Some(proto) = self.protocol
            && conn.protocol != proto
        {
            return false;
        }

        if let Some(ref proc) = self.process {
            let matched = conn
                .process_name
                .as_deref()
                .map(|p| matches_process_name(p, proc))
                .unwrap_or(false);
            if !matched {
                return false;
            }
        }

        if let Some(ref suffix) = self.host_suffix {
            let matched = if let Some(ref h) = conn.host
                && matches_host_suffix(h, suffix)
            {
                true
            } else {
                matches_host_suffix(&conn.dst_addr, suffix)
            };
            if !matched {
                return false;
            }
        }

        if let Some(ref nd) = self.node {
            if conn.outbound_node.as_deref() != Some(nd.as_str()) {
                return false;
            }
        }

        if let Some(ref grp) = self.group {
            if conn.rule_group.as_deref() != Some(grp.as_str()) {
                return false;
            }
        }

        if let Some(pid) = self.pid {
            if conn.pid != Some(pid) {
                return false;
            }
        }

        if let Some(state) = self.state {
            if conn.state != state {
                return false;
            }
        }

        if let Some(min_idle) = self.min_idle_secs {
            if conn.idle_duration_secs(now_secs) < min_idle {
                return false;
            }
        }

        if let Some(min_bytes) = self.min_total_bytes {
            if conn.total_bytes() < min_bytes {
                return false;
            }
        }

        if let Some(tag) = self.has_security_tag {
            if !conn.security_tags.contains(&tag) {
                return false;
            }
        }

        true
    }
}

/// Fine-grained timeout thresholds for idle, zero-traffic, and half-closed connection cleanup.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SweeperTimeouts {
    pub tcp_idle_timeout_secs: u64,
    pub udp_idle_timeout_secs: u64,
    pub other_idle_timeout_secs: u64,
    pub zero_traffic_zombie_timeout_secs: u64,
    pub half_closed_timeout_secs: u64,
}

impl Default for SweeperTimeouts {
    fn default() -> Self {
        Self {
            tcp_idle_timeout_secs: 300,
            udp_idle_timeout_secs: 60,
            other_idle_timeout_secs: 300,
            zero_traffic_zombie_timeout_secs: 15,
            half_closed_timeout_secs: 30,
        }
    }
}

/// Sweeper that tracks active connection sessions, detects idle/zombie sessions across protocols,
/// enforces connection quotas, and facilitates rich DSL query teardown.
pub struct IdleConnectionSweeper {
    timeouts: SweeperTimeouts,
    max_connections: Option<usize>,
    max_per_process: Option<usize>,
    max_per_node: Option<usize>,
    connections: HashMap<String, TrackedConnection>,
}

impl IdleConnectionSweeper {
    /// Creates a sweeper with standard TCP (default 300s) and UDP (default 60s) idle timeouts.
    pub fn new(tcp_idle_timeout_secs: u64, udp_idle_timeout_secs: u64) -> Self {
        Self {
            timeouts: SweeperTimeouts {
                tcp_idle_timeout_secs,
                udp_idle_timeout_secs,
                other_idle_timeout_secs: tcp_idle_timeout_secs,
                ..Default::default()
            },
            max_connections: None,
            max_per_process: None,
            max_per_node: None,
            connections: HashMap::new(),
        }
    }

    /// Creates a sweeper with a maximum global connection concurrency limit.
    pub fn with_capacity(
        tcp_idle_timeout_secs: u64,
        udp_idle_timeout_secs: u64,
        max_connections: usize,
    ) -> Self {
        Self {
            timeouts: SweeperTimeouts {
                tcp_idle_timeout_secs,
                udp_idle_timeout_secs,
                other_idle_timeout_secs: tcp_idle_timeout_secs,
                ..Default::default()
            },
            max_connections: Some(max_connections),
            max_per_process: None,
            max_per_node: None,
            connections: HashMap::with_capacity(max_connections),
        }
    }

    /// Returns a sweeper initialized with default production timeouts (TCP: 300s, UDP: 60s).
    pub fn default_config() -> Self {
        Self::new(300, 60)
    }

    /// Sets per-process and per-node maximum connection quotas.
    pub fn set_quotas(&mut self, max_per_process: Option<usize>, max_per_node: Option<usize>) {
        self.max_per_process = max_per_process;
        self.max_per_node = max_per_node;
    }

    /// Registers a new connection session with capacity and quota enforcement.
    pub fn register_connection(
        &mut self,
        connection: TrackedConnection,
    ) -> Result<(), SweeperError> {
        let id = connection.id.clone();
        if !self.connections.contains_key(&id) {
            // Check global capacity
            if let Some(limit) = self.max_connections
                && self.connections.len() >= limit
            {
                return Err(SweeperError::CapacityExceeded(limit));
            }

            // Check per-process quota
            if let Some(proc_limit) = self.max_per_process
                && let Some(ref proc) = connection.process_name
            {
                let count = self
                    .connections
                    .values()
                    .filter(|c| c.process_name.as_deref() == Some(proc.as_str()))
                    .count();
                if count >= proc_limit {
                    return Err(SweeperError::ProcessQuotaExceeded {
                        process: proc.clone(),
                        limit: proc_limit,
                    });
                }
            }

            // Check per-node quota
            if let Some(node_limit) = self.max_per_node
                && let Some(ref node) = connection.outbound_node
            {
                let count = self
                    .connections
                    .values()
                    .filter(|c| c.outbound_node.as_deref() == Some(node.as_str()))
                    .count();
                if count >= node_limit {
                    return Err(SweeperError::NodeQuotaExceeded {
                        node: node.clone(),
                        limit: node_limit,
                    });
                }
            }
        }

        self.connections.insert(id, connection);
        Ok(())
    }

    /// Updates activity timestamp and adds transferred bytes for an existing connection.
    pub fn touch_connection(
        &mut self,
        id: &str,
        now_secs: u64,
        upload_bytes: u64,
        download_bytes: u64,
    ) -> bool {
        if let Some(conn) = self.connections.get_mut(id) {
            conn.record_activity(now_secs, upload_bytes, download_bytes);
            true
        } else {
            false
        }
    }

    /// Queries connections matching the provided DSL query criteria.
    pub fn query_connections(&self, query: &ConnectionQuery) -> Vec<&TrackedConnection> {
        let now = current_unix_secs();
        self.connections
            .values()
            .filter(|conn| query.matches(conn, now))
            .collect()
    }

    /// Identifies all connection IDs exceeding protocol-specific idle timeouts.
    pub fn find_idle_connections(&self) -> Vec<String> {
        let now = current_unix_secs();
        self.find_idle_connections_at(now)
    }

    /// Identifies all connection IDs exceeding idle timeouts at a specified timestamp.
    pub fn find_idle_connections_at(&self, now_secs: u64) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                let timeout = match conn.protocol {
                    TransportProtocol::Tcp => self.timeouts.tcp_idle_timeout_secs,
                    TransportProtocol::Udp => self.timeouts.udp_idle_timeout_secs,
                    TransportProtocol::Other => self.timeouts.other_idle_timeout_secs,
                };
                conn.idle_duration_secs(now_secs) > timeout
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Identifies all zero-traffic zombie connections exceeding zombie timeout.
    pub fn find_zombies_at(&self, now_secs: u64) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                conn.is_zero_traffic()
                    && conn.idle_duration_secs(now_secs)
                        > self.timeouts.zero_traffic_zombie_timeout_secs
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Identifies all half-closed connections exceeding half-closed timeout.
    pub fn find_half_closed_at(&self, now_secs: u64) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                conn.is_half_closed()
                    && conn.idle_duration_secs(now_secs)
                        > self.timeouts.half_closed_timeout_secs
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Finds all connection IDs belonging to a given process name.
    pub fn find_connections_by_process(&self, process: &str) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                conn.process_name
                    .as_deref()
                    .map(|p| matches_process_name(p, process))
                    .unwrap_or(false)
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Finds all connection IDs targeting a given host suffix.
    pub fn find_connections_by_host(&self, host_suffix: &str) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                if let Some(ref host) = conn.host
                    && matches_host_suffix(host, host_suffix)
                {
                    return true;
                }
                matches_host_suffix(&conn.dst_addr, host_suffix)
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Finds all connection IDs routed through a specific outbound node.
    pub fn find_connections_by_node(&self, outbound_node: &str) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                conn.outbound_node
                    .as_deref()
                    .map(|n| n == outbound_node)
                    .unwrap_or(false)
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Finds all connection IDs governed by a specific rule group.
    pub fn find_connections_by_group(&self, rule_group: &str) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| {
                conn.rule_group
                    .as_deref()
                    .map(|g| g == rule_group)
                    .unwrap_or(false)
            })
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Finds all connection IDs associated with a specific Process ID (PID).
    pub fn find_connections_by_pid(&self, pid: u32) -> Vec<String> {
        self.connections
            .values()
            .filter(|conn| conn.pid == Some(pid))
            .map(|conn| conn.id.clone())
            .collect()
    }

    /// Sweeps and terminates all idle connections exceeding protocol timeouts.
    pub fn sweep_idle(&mut self) -> Vec<String> {
        let now = current_unix_secs();
        self.sweep_idle_at(now)
    }

    /// Sweeps and terminates all idle connections exceeding protocol timeouts at the given timestamp.
    pub fn sweep_idle_at(&mut self, now_secs: u64) -> Vec<String> {
        let idle_ids = self.find_idle_connections_at(now_secs);
        for id in &idle_ids {
            self.connections.remove(id);
        }
        idle_ids
    }

    /// Sweeps and terminates all zero-traffic zombie connections.
    pub fn sweep_zombies_at(&mut self, now_secs: u64) -> Vec<String> {
        let zombie_ids = self.find_zombies_at(now_secs);
        for id in &zombie_ids {
            self.connections.remove(id);
        }
        zombie_ids
    }

    /// Terminates connections matching the DSL query criteria.
    pub fn terminate_by_query(&mut self, query: &ConnectionQuery) -> Vec<TrackedConnection> {
        let now = current_unix_secs();
        let matching_ids: Vec<String> = self
            .connections
            .values()
            .filter(|conn| query.matches(conn, now))
            .map(|conn| conn.id.clone())
            .collect();

        let mut terminated = Vec::with_capacity(matching_ids.len());
        for id in matching_ids {
            if let Some(conn) = self.connections.remove(&id) {
                terminated.push(conn);
            }
        }
        terminated
    }

    /// Terminates a single connection by ID and returns its record if it was present.
    pub fn terminate_connection(&mut self, id: &str) -> Option<TrackedConnection> {
        self.connections.remove(id)
    }

    /// Terminates a batch of connections by their IDs. Returns the list of removed records.
    pub fn terminate_connections(&mut self, ids: &[&str]) -> Vec<TrackedConnection> {
        let mut removed = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(conn) = self.connections.remove(*id) {
                removed.push(conn);
            }
        }
        removed
    }

    /// Terminates all connections matching the specified process name.
    pub fn terminate_by_process(&mut self, process: &str) -> Vec<TrackedConnection> {
        let ids = self.find_connections_by_process(process);
        let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        self.terminate_connections(&id_refs)
    }

    /// Terminates all connections matching the specified host suffix.
    pub fn terminate_by_host(&mut self, host_suffix: &str) -> Vec<TrackedConnection> {
        let ids = self.find_connections_by_host(host_suffix);
        let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        self.terminate_connections(&id_refs)
    }

    /// Returns the number of currently active monitored connections.
    pub fn active_connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Returns `true` if there are no monitored connections.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Retrieves an immutable reference to a connection by ID.
    pub fn get_connection(&self, id: &str) -> Option<&TrackedConnection> {
        self.connections.get(id)
    }

    /// Retrieves a mutable reference to a connection by ID.
    pub fn get_connection_mut(&mut self, id: &str) -> Option<&mut TrackedConnection> {
        self.connections.get_mut(id)
    }

    /// Lists references to all currently active connections.
    pub fn list_connections(&self) -> Vec<&TrackedConnection> {
        self.connections.values().collect()
    }

    /// Clears all tracked connections.
    pub fn clear(&mut self) {
        self.connections.clear();
    }
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn matches_host_suffix(target: &str, suffix: &str) -> bool {
    let clean_target = target.trim().trim_end_matches('.').to_ascii_lowercase();
    let host_part = clean_target.split(':').next().unwrap_or(&clean_target);
    let clean_suffix = suffix
        .trim()
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_ascii_lowercase();

    if host_part.is_empty() || clean_suffix.is_empty() {
        return false;
    }
    if host_part == clean_suffix {
        return true;
    }
    let dot_suffix = format!(".{}", clean_suffix);
    host_part.ends_with(&dot_suffix)
}

fn matches_process_name(actual: &str, query: &str) -> bool {
    let actual_clean = actual.trim();
    let query_clean = query.trim();
    if actual_clean.is_empty() || query_clean.is_empty() {
        return false;
    }
    if actual_clean.eq_ignore_ascii_case(query_clean) {
        return true;
    }
    let base_name = actual_clean
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(actual_clean);
    if base_name.eq_ignore_ascii_case(query_clean) {
        return true;
    }
    let base_without_exe = base_name.strip_suffix(".exe").unwrap_or(base_name);
    base_without_exe.eq_ignore_ascii_case(query_clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn(
        id: &str,
        proto: TransportProtocol,
        process: Option<&str>,
        host: Option<&str>,
        last_activity: u64,
    ) -> TrackedConnection {
        let mut conn =
            TrackedConnection::new(id, proto, "127.0.0.1:12345", "93.184.216.34:80", 100);
        conn.last_activity_secs = last_activity;
        if let Some(p) = process {
            conn = conn.with_process(p, Some(1001));
        }
        if let Some(h) = host {
            conn = conn.with_host(h);
        }
        conn
    }

    #[test]
    fn test_register_and_capacity() {
        let mut sweeper = IdleConnectionSweeper::with_capacity(300, 60, 2);
        let c1 = make_conn(
            "c1",
            TransportProtocol::Tcp,
            Some("curl"),
            Some("example.com"),
            100,
        );
        let c2 = make_conn(
            "c2",
            TransportProtocol::Udp,
            Some("dns"),
            Some("1.1.1.1"),
            100,
        );
        let c3 = make_conn(
            "c3",
            TransportProtocol::Tcp,
            Some("node"),
            Some("api.com"),
            100,
        );

        assert!(sweeper.register_connection(c1).is_ok());
        assert!(sweeper.register_connection(c2).is_ok());
        assert_eq!(sweeper.active_connection_count(), 2);

        // Third should fail due to capacity
        let res = sweeper.register_connection(c3);
        assert!(matches!(res, Err(SweeperError::CapacityExceeded(2))));
    }

    #[test]
    fn test_process_and_node_quotas() {
        let mut sweeper = IdleConnectionSweeper::default_config();
        sweeper.set_quotas(Some(2), Some(1));

        let c1 = make_conn("c1", TransportProtocol::Tcp, Some("curl"), None, 100)
            .with_routing(Some("node-us".to_string()), None);
        let c2 = make_conn("c2", TransportProtocol::Tcp, Some("curl"), None, 100)
            .with_routing(Some("node-hk".to_string()), None);
        let c3 = make_conn("c3", TransportProtocol::Tcp, Some("curl"), None, 100)
            .with_routing(Some("node-jp".to_string()), None);

        assert!(sweeper.register_connection(c1).is_ok());
        assert!(sweeper.register_connection(c2).is_ok());

        // Process quota exceeded (limit = 2)
        let res = sweeper.register_connection(c3);
        assert!(matches!(res, Err(SweeperError::ProcessQuotaExceeded { .. })));

        // Node quota exceeded (node-us limit = 1)
        let c4 = make_conn("c4", TransportProtocol::Tcp, Some("chrome"), None, 100)
            .with_routing(Some("node-us".to_string()), None);
        let res_node = sweeper.register_connection(c4);
        assert!(matches!(res_node, Err(SweeperError::NodeQuotaExceeded { .. })));
    }

    #[test]
    fn test_touch_and_rate_differential() {
        let mut sweeper = IdleConnectionSweeper::new(300, 60);
        let c1 = make_conn(
            "c1",
            TransportProtocol::Tcp,
            Some("curl"),
            Some("example.com"),
            100,
        );
        sweeper.register_connection(c1).unwrap();

        // Touch with 2 seconds delta and 2000 up, 4000 down
        assert!(sweeper.touch_connection("c1", 102, 2000, 4000));
        let conn = sweeper.get_connection("c1").unwrap();
        assert_eq!(conn.last_activity_secs, 102);
        assert_eq!(conn.upload_bytes, 2000);
        assert_eq!(conn.download_bytes, 4000);
        assert_eq!(conn.total_bytes(), 6000);
        assert_eq!(conn.upload_rate_bps, 1000.0);
        assert_eq!(conn.download_rate_bps, 2000.0);
        assert_eq!(conn.peak_rate_bps, 3000.0);

        assert!(!sweeper.touch_connection("non-existent", 160, 10, 10));
    }

    #[test]
    fn test_sweep_zombies_and_differentiated_timeouts() {
        let mut sweeper = IdleConnectionSweeper::new(300, 60);
        let mut tcp_conn = make_conn("tcp-1", TransportProtocol::Tcp, None, None, 1000);
        tcp_conn.upload_bytes = 100;
        let mut udp_conn = make_conn("udp-1", TransportProtocol::Udp, None, None, 1000);
        udp_conn.download_bytes = 100;
        let zombie_conn = make_conn("zombie-1", TransportProtocol::Tcp, None, None, 1000);

        sweeper.register_connection(tcp_conn).unwrap();
        sweeper.register_connection(udp_conn).unwrap();
        sweeper.register_connection(zombie_conn).unwrap();

        // At now = 1020 (20s > 15s zombie timeout): zombie is swept
        let swept_zombies = sweeper.sweep_zombies_at(1020);
        assert_eq!(swept_zombies, vec!["zombie-1"]);
        assert_eq!(sweeper.active_connection_count(), 2);

        // At now = 1070 (idle 70s): UDP expired (70 > 60)
        let swept = sweeper.sweep_idle_at(1070);
        assert_eq!(swept, vec!["udp-1"]);
        assert_eq!(sweeper.active_connection_count(), 1);
        assert!(sweeper.get_connection("tcp-1").is_some());
    }

    #[test]
    fn test_query_dsl_and_terminate() {
        let mut sweeper = IdleConnectionSweeper::default_config();
        let c1 = make_conn("c1", TransportProtocol::Tcp, Some("curl"), Some("api.google.com"), 100)
            .with_routing(Some("node-us".to_string()), Some("group-1".to_string()))
            .with_security_tag(SecurityAuditTag::SuspiciousDirectBypass);
        let c2 = make_conn("c2", TransportProtocol::Tcp, Some("node"), Some("api.google.com"), 100)
            .with_routing(Some("node-hk".to_string()), Some("group-1".to_string()));

        sweeper.register_connection(c1).unwrap();
        sweeper.register_connection(c2).unwrap();

        let q = ConnectionQuery::new()
            .with_process("curl")
            .with_host_suffix("google.com");
        let results = sweeper.query_connections(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "c1");

        let tag_query = ConnectionQuery::new()
            .with_security_tag(SecurityAuditTag::SuspiciousDirectBypass);
        let terminated = sweeper.terminate_by_query(&tag_query);
        assert_eq!(terminated.len(), 1);
        assert_eq!(terminated[0].id, "c1");
        assert_eq!(sweeper.active_connection_count(), 1);
    }
}
