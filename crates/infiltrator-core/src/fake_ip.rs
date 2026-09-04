use crate::settings::app_config_manager;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FakeIpConfig {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub fake_ip_filter_mode: Option<String>,
    pub store_fake_ip: Option<bool>,
    pub ipv6_fake_ip_range: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FakeIpConfigPatch {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub fake_ip_filter_mode: Option<String>,
    pub store_fake_ip: Option<bool>,
    pub ipv6_fake_ip_range: Option<String>,
}

impl FakeIpConfig {
    pub fn apply_patch(&mut self, patch: FakeIpConfigPatch) {
        if let Some(value) = patch.fake_ip_range {
            self.fake_ip_range = Some(value);
        }
        if let Some(value) = patch.fake_ip_filter {
            self.fake_ip_filter = Some(value);
        }
        if let Some(value) = patch.fake_ip_filter_mode {
            self.fake_ip_filter_mode = Some(value);
        }
        if let Some(value) = patch.store_fake_ip {
            self.store_fake_ip = Some(value);
        }
        if let Some(value) = patch.ipv6_fake_ip_range {
            self.ipv6_fake_ip_range = Some(value);
        }
    }
}

/// Returns a comprehensive set of default anti-leak domain filter patterns.
///
/// These patterns cover:
/// - NTP / Time synchronization
/// - STUN / TURN VoIP protocols
/// - Local network host discovery (mDNS, NetBIOS, LAN)
/// - Captive portal and captive network detection
/// - Gaming and direct peer-to-peer bypass services
pub fn default_anti_leak_filters() -> Vec<String> {
    vec![
        // Local network & mDNS
        "*.lan".to_string(),
        "*.local".to_string(),
        "*.localhost".to_string(),
        "*.home.arpa".to_string(),
        "*.internal".to_string(),
        "*.corp".to_string(),
        // Network Time Protocol (NTP)
        "time.*.com".to_string(),
        "time.*.gov".to_string(),
        "time.*.edu.cn".to_string(),
        "time.*.apple.com".to_string(),
        "time1.cloud.tencent.com".to_string(),
        "ntp.*.com".to_string(),
        "*.pool.ntp.org".to_string(),
        // STUN / WebRTC Direct Audio & Video
        "stun.*.*".to_string(),
        "stun.*.*.*".to_string(),
        "*.stun.*.*".to_string(),
        "*.stun.*.*.*".to_string(),
        "turn.*.*".to_string(),
        // Captive portal & Connectivity checks
        "*.msftncsi.com".to_string(),
        "*.msftconnecttest.com".to_string(),
        "detectportal.firefox.com".to_string(),
        "captive.apple.com".to_string(),
        "connectivitycheck.gstatic.com".to_string(),
        // Local Router Management
        "router.asus.com".to_string(),
        "tplinkwifi.net".to_string(),
        "miwifi.com".to_string(),
        "tendawifi.com".to_string(),
    ]
}

/// Detects whether a given Fake-IP IPv4 CIDR overlaps with standard private/loopback/link-local IPv4 ranges.
///
/// Standard private ranges:
/// - 10.0.0.0/8 (RFC 1918)
/// - 172.16.0.0/12 (RFC 1918)
/// - 192.168.0.0/16 (RFC 1918)
/// - 127.0.0.0/8 (Loopback)
/// - 169.254.0.0/16 (Link-Local RFC 3927)
/// - 224.0.0.0/4 (Multicast)
/// - 240.0.0.0/4 (Reserved)
pub fn detect_private_ip_collision(fake_ip_cidr: &str) -> Vec<String> {
    let (cidr_start, cidr_end) = match parse_cidr_range_u32(fake_ip_cidr) {
        Some(range) => range,
        None => return vec!["Invalid CIDR string".to_string()],
    };

    let private_ranges = [
        ("10.0.0.0/8", "RFC 1918 Private Class A (10.0.0.0 - 10.255.255.255)"),
        ("172.16.0.0/12", "RFC 1918 Private Class B (172.16.0.0 - 172.31.255.255)"),
        ("192.168.0.0/16", "RFC 1918 Private Class C (192.168.0.0 - 192.168.255.255)"),
        ("127.0.0.0/8", "Loopback Space (127.0.0.0 - 127.255.255.255)"),
        ("169.254.0.0/16", "Link-Local Space (169.254.0.0 - 169.254.255.255)"),
        ("224.0.0.0/4", "Multicast Space (224.0.0.0 - 239.255.255.255)"),
    ];

    let mut collisions = Vec::new();
    for (priv_cidr, desc) in private_ranges {
        if let Some((p_start, p_end)) = parse_cidr_range_u32(priv_cidr) {
            // Overlap condition: max(start1, start2) <= min(end1, end2)
            if std::cmp::max(cidr_start, p_start) <= std::cmp::min(cidr_end, p_end) {
                collisions.push(format!("Overlaps with {} ({})", priv_cidr, desc));
            }
        }
    }

    collisions
}

fn parse_cidr_range_u32(cidr: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = cidr.trim().split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let ip: Ipv4Addr = parts[0].parse().ok()?;
    let prefix: u32 = parts[1].parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let ip_num = u32::from(ip);
    let mask = if prefix == 0 {
        0
    } else {
        !((1u32 << (32 - prefix)) - 1)
    };
    let start = ip_num & mask;
    let end = start | (!mask);
    Some((start, end))
}

/// Statistics for a Fake-IP Pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FakeIpPoolStats {
    pub total_allocated: usize,
    pub pool_capacity: usize,
    pub utilization_percent: f64,
    pub start_ip: String,
    pub end_ip: String,
}

/// In-memory Fake-IP address allocator and bidirectional mapping engine.
/// Simulates and validates core Fake-IP allocation, LRU eviction, and persistence behavior.
#[derive(Debug, Clone)]
pub struct FakeIpPool {
    cidr: String,
    start_ip_num: u32,
    end_ip_num: u32,
    current_cursor: u32,
    capacity: usize,
    domain_to_ip: HashMap<String, (u32, u64)>, // domain -> (ip_num, timestamp)
    ip_to_domain: HashMap<u32, String>,         // ip_num -> domain
    lru_queue: VecDeque<u32>,                   // for eviction
}

impl FakeIpPool {
    /// Creates a new FakeIpPool for the given IPv4 CIDR (e.g., `198.18.0.1/16`).
    pub fn new(cidr: &str) -> Result<Self> {
        let (start, end) = parse_cidr_range_u32(cidr)
            .ok_or_else(|| anyhow!("invalid CIDR for Fake-IP pool: {}", cidr))?;
        // Start from start + 1 (reserve network IP), up to end - 1 (reserve broadcast)
        let pool_start = start.saturating_add(1);
        let pool_end = end.saturating_sub(1);
        let capacity = if pool_end >= pool_start {
            (pool_end - pool_start + 1) as usize
        } else {
            0
        };

        if capacity == 0 {
            return Err(anyhow!("CIDR range too small for Fake-IP pool: {}", cidr));
        }

        Ok(Self {
            cidr: cidr.to_string(),
            start_ip_num: pool_start,
            end_ip_num: pool_end,
            current_cursor: pool_start,
            capacity,
            domain_to_ip: HashMap::new(),
            ip_to_domain: HashMap::new(),
            lru_queue: VecDeque::new(),
        })
    }

    /// Returns the managed CIDR prefix.
    pub fn cidr(&self) -> &str {
        &self.cidr
    }

    /// Allocates or retrieves an existing Fake-IP for the specified domain.
    pub fn allocate(&mut self, domain: &str) -> Result<String> {
        let domain_lower = domain.trim().to_ascii_lowercase();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check if already allocated
        if let Some((ip_num, _)) = self.domain_to_ip.get_mut(&domain_lower) {
            let allocated = *ip_num;
            self.domain_to_ip.insert(domain_lower, (allocated, now));
            return Ok(Ipv4Addr::from(allocated).to_string());
        }

        // Allocate a new IP
        let ip_num = if self.domain_to_ip.len() < self.capacity {
            let next = self.current_cursor;
            if self.current_cursor >= self.end_ip_num {
                self.current_cursor = self.start_ip_num;
            } else {
                self.current_cursor += 1;
            }
            next
        } else {
            // Evict oldest from LRU queue
            let evicted_ip = self
                .lru_queue
                .pop_front()
                .ok_or_else(|| anyhow!("fake-ip pool exhaustion and no LRU candidate"))?;
            if let Some(old_domain) = self.ip_to_domain.remove(&evicted_ip) {
                self.domain_to_ip.remove(&old_domain);
            }
            evicted_ip
        };

        self.domain_to_ip
            .insert(domain_lower.clone(), (ip_num, now));
        self.ip_to_domain.insert(ip_num, domain_lower);
        self.lru_queue.push_back(ip_num);

        Ok(Ipv4Addr::from(ip_num).to_string())
    }

    /// Performs reverse lookup: Fake-IP string -> Domain name.
    pub fn reverse_lookup(&self, ip_str: &str) -> Option<&str> {
        let ip: Ipv4Addr = ip_str.parse().ok()?;
        let ip_num = u32::from(ip);
        self.ip_to_domain.get(&ip_num).map(|s| s.as_str())
    }

    /// Checks if a given IP string is within this pool's managed range.
    pub fn contains_ip(&self, ip_str: &str) -> bool {
        let ip: Ipv4Addr = match ip_str.parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let num = u32::from(ip);
        num >= self.start_ip_num && num <= self.end_ip_num
    }

    /// Evaluates whether a domain should be filtered (bypassed or included)
    /// based on the filter mode (`blacklist` or `whitelist`) and filter patterns.
    pub fn should_filter(domain: &str, filter_mode: &str, filters: &[String]) -> bool {
        let domain_lower = domain.trim().to_ascii_lowercase();
        let matches = filters.iter().any(|pattern| match_domain_pattern(&domain_lower, pattern));

        if filter_mode.eq_ignore_ascii_case("whitelist") {
            // Whitelist: ONLY domains matching the filter list receive Fake-IP.
            matches
        } else {
            // Blacklist (default): All domains receive Fake-IP EXCEPT those matching filter list.
            matches
        }
    }

    /// Returns pool metrics.
    pub fn stats(&self) -> FakeIpPoolStats {
        let total = self.domain_to_ip.len();
        let util = if self.capacity > 0 {
            (total as f64 / self.capacity as f64) * 100.0
        } else {
            0.0
        };
        FakeIpPoolStats {
            total_allocated: total,
            pool_capacity: self.capacity,
            utilization_percent: (util * 100.0).round() / 100.0,
            start_ip: Ipv4Addr::from(self.start_ip_num).to_string(),
            end_ip: Ipv4Addr::from(self.end_ip_num).to_string(),
        }
    }

    /// Purges entries older than `max_age_secs`.
    pub fn purge_stale_entries(&mut self, max_age_secs: u64) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut purged = 0;
        let stale_domains: Vec<String> = self
            .domain_to_ip
            .iter()
            .filter(|(_, (_, ts))| now.saturating_sub(*ts) > max_age_secs)
            .map(|(d, _)| d.clone())
            .collect();

        for domain in stale_domains {
            if let Some((ip_num, _)) = self.domain_to_ip.remove(&domain) {
                self.ip_to_domain.remove(&ip_num);
                self.lru_queue.retain(|&x| x != ip_num);
                purged += 1;
            }
        }
        purged
    }

    /// Exports active mappings as a serialized JSON string.
    pub fn export_mappings_json(&self) -> Result<String> {
        let map: HashMap<String, String> = self
            .domain_to_ip
            .iter()
            .map(|(d, (ip, _))| (d.clone(), Ipv4Addr::from(*ip).to_string()))
            .collect();
        serde_json::to_string_pretty(&map).context("serialize fake-ip mappings")
    }

    /// Imports mappings from JSON, merging with current pool without reallocating duplicate domains.
    pub fn import_mappings_json(&mut self, json_str: &str) -> Result<usize> {
        let map: HashMap<String, String> =
            serde_json::from_str(json_str).context("parse fake-ip mappings JSON")?;
        let mut count = 0;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for (domain, ip_str) in map {
            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                let ip_num = u32::from(ip);
                if ip_num >= self.start_ip_num && ip_num <= self.end_ip_num {
                    self.domain_to_ip.insert(domain.clone(), (ip_num, now));
                    self.ip_to_domain.insert(ip_num, domain);
                    self.lru_queue.push_back(ip_num);
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}

/// Match domain against wildcard / prefix patterns.
///
/// Supported patterns:
/// - `*.example.com` -> Matches `sub.example.com`, `a.b.example.com`
/// - `+.example.com` -> Matches `example.com`, `sub.example.com`
/// - `example.com` -> Exact match
fn match_domain_pattern(domain: &str, pattern: &str) -> bool {
    let p = pattern.trim().to_ascii_lowercase();
    let d = domain.trim().to_ascii_lowercase();

    if p == d {
        return true;
    }

    if let Some(suffix) = p.strip_prefix("*.")
        && d.ends_with(suffix) && d.len() > suffix.len() && d[..d.len() - suffix.len()].ends_with('.') {
            return true;
        }

    if let Some(suffix) = p.strip_prefix("+.") {
        if d == suffix {
            return true;
        }
        if d.ends_with(suffix) && d.len() > suffix.len() && d[..d.len() - suffix.len()].ends_with('.') {
            return true;
        }
    }

    // General wildcard matching for pattern containing '*'
    if p.contains('*') {
        let parts: Vec<&str> = p.split('*').collect();
        let mut cur_d = d.as_str();

        // Check first part (must be prefix)
        if let Some(first) = parts.first()
            && !first.is_empty() {
                if !cur_d.starts_with(first) {
                    return false;
                }
                cur_d = &cur_d[first.len()..];
            }

        // Check last part (must be suffix)
        if let Some(last) = parts.last()
            && !last.is_empty() {
                if !cur_d.ends_with(last) {
                    return false;
                }
                cur_d = &cur_d[..cur_d.len() - last.len()];
            }

        // Check middle parts in sequence
        for part in &parts[1..parts.len().saturating_sub(1)] {
            if part.is_empty() {
                continue;
            }
            if let Some(pos) = cur_d.find(part) {
                cur_d = &cur_d[pos + part.len()..];
            } else {
                return false;
            }
        }
        return true;
    }

    false
}

pub async fn load_fake_ip_config() -> Result<FakeIpConfig> {
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let doc: Value = serde_yaml_ng::from_str(&content).context("parse profile yaml")?;
    extract_fake_ip_config_from_doc(&doc)
}

pub async fn save_fake_ip_config(patch: FakeIpConfigPatch) -> Result<FakeIpConfig> {
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let mut doc: Value = serde_yaml_ng::from_str(&content).context("parse profile yaml")?;

    let mut config = extract_fake_ip_config_from_doc(&doc)?;
    config.apply_patch(patch);
    validate_fake_ip_config(&config)?;
    apply_fake_ip_config(&mut doc, &config)?;

    let updated = serde_yaml_ng::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

/// Apply a Fake-IP patch to an in-memory profile document for the shared
/// atomic Apply transaction.
pub fn apply_fake_ip_patch_to_yaml(content: &str, patch: FakeIpConfigPatch) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    let mut config = extract_fake_ip_config_from_doc(&doc)?;
    config.apply_patch(patch);
    validate_fake_ip_config(&config)?;
    apply_fake_ip_config(&mut doc, &config)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

pub async fn clear_fake_ip_cache() -> Result<bool> {
    let manager = app_config_manager().await.context("init config manager")?;
    let profile_path = manager
        .get_current_path()
        .await
        .context("load current profile path")?;
    let config_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("profile path has no parent directory"))?;
    let cache_path = config_dir.join("fake-ip-cache");
    if fs::try_exists(&cache_path)
        .await
        .context("check fake-ip cache")?
    {
        fs::remove_file(&cache_path)
            .await
            .context("remove fake-ip cache")?;
        return Ok(true);
    }
    Ok(false)
}

pub fn extract_fake_ip_config_from_doc(doc: &Value) -> Result<FakeIpConfig> {
    let dns_value = doc
        .get("dns")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_yaml_ng::from_value(dns_value).context("decode fake-ip config")?;
    Ok(config)
}

fn apply_fake_ip_config(doc: &mut Value, config: &FakeIpConfig) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    let dns_entry = map
        .entry(Value::String("dns".to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let dns_map = dns_entry
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("dns section is not a mapping"))?;

    if let Some(range) = config.fake_ip_range.as_ref() {
        dns_map.insert(
            Value::String("fake-ip-range".to_string()),
            Value::String(range.clone()),
        );
    }
    if let Some(filter) = config.fake_ip_filter.as_ref() {
        let value = serde_yaml_ng::to_value(filter).context("encode fake-ip-filter")?;
        dns_map.insert(Value::String("fake-ip-filter".to_string()), value);
    }
    if let Some(mode) = config.fake_ip_filter_mode.as_ref() {
        dns_map.insert(
            Value::String("fake-ip-filter-mode".to_string()),
            Value::String(mode.clone()),
        );
    }
    if let Some(store) = config.store_fake_ip {
        dns_map.insert(
            Value::String("store-fake-ip".to_string()),
            Value::Bool(store),
        );
    }
    if let Some(ipv6_range) = config.ipv6_fake_ip_range.as_ref() {
        dns_map.insert(
            Value::String("ipv6-fake-ip-range".to_string()),
            Value::String(ipv6_range.clone()),
        );
    }
    Ok(())
}

fn validate_fake_ip_config(config: &FakeIpConfig) -> Result<()> {
    if let Some(range) = config.fake_ip_range.as_ref() {
        if range.trim().is_empty() {
            return Err(anyhow!("fake-ip-range is empty"));
        }
        if parse_cidr_range_u32(range).is_none() {
            return Err(anyhow!("invalid fake-ip-range CIDR format: {}", range));
        }
    }
    if let Some(mode) = config.fake_ip_filter_mode.as_ref() {
        let m = mode.trim().to_ascii_lowercase();
        if m != "blacklist" && m != "whitelist" {
            return Err(anyhow!("unsupported fake-ip-filter-mode: {}", mode));
        }
    }
    if let Some(filter) = config.fake_ip_filter.as_ref() {
        for entry in filter {
            if entry.trim().is_empty() {
                return Err(anyhow!("fake-ip-filter contains empty entry"));
            }
        }
    }
    if let Some(ipv6_range) = config.ipv6_fake_ip_range.as_ref() {
        if ipv6_range.trim().is_empty() {
            return Err(anyhow!("ipv6-fake-ip-range is empty"));
        }
        // Basic IPv6 CIDR validation
        let parts: Vec<&str> = ipv6_range.trim().split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow!("invalid ipv6-fake-ip-range format: {}", ipv6_range));
        }
        if parts[0].parse::<Ipv6Addr>().is_err() {
            return Err(anyhow!("invalid IPv6 in ipv6-fake-ip-range: {}", parts[0]));
        }
        let prefix: u32 = parts[1]
            .parse()
            .map_err(|_| anyhow!("invalid IPv6 prefix length in ipv6-fake-ip-range"))?;
        if prefix > 128 {
            return Err(anyhow!("IPv6 prefix length exceeds 128: {}", prefix));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fake_ip_default() {
        let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let config = extract_fake_ip_config_from_doc(&doc).expect("fake ip config");
        assert!(config.fake_ip_range.is_none());
    }

    #[test]
    fn test_apply_patch_and_validate() {
        let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let mut config = extract_fake_ip_config_from_doc(&doc).expect("fake ip config");
        let patch = FakeIpConfigPatch {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            fake_ip_filter: Some(vec!["*.lan".to_string()]),
            fake_ip_filter_mode: Some("blacklist".to_string()),
            store_fake_ip: Some(true),
            ipv6_fake_ip_range: Some("fc00::/18".to_string()),
        };
        config.apply_patch(patch);
        validate_fake_ip_config(&config).expect("valid fake-ip config");
        assert_eq!(config.fake_ip_filter_mode.as_deref(), Some("blacklist"));
        assert_eq!(config.ipv6_fake_ip_range.as_deref(), Some("fc00::/18"));
    }

    #[test]
    fn test_apply_fake_ip_config_updates_dns() {
        let mut doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let config = FakeIpConfig {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            fake_ip_filter_mode: Some("whitelist".to_string()),
            ..FakeIpConfig::default()
        };
        apply_fake_ip_config(&mut doc, &config).expect("apply fake ip");
        let map = doc.as_mapping().expect("mapping");
        let dns = map.get(Value::String("dns".to_string())).expect("dns section");
        assert_eq!(
            dns.get("fake-ip-filter-mode"),
            Some(&Value::String("whitelist".to_string()))
        );
    }

    #[test]
    fn test_validate_rejects_empty_filter_entry() {
        let config = FakeIpConfig {
            fake_ip_filter: Some(vec!["".to_string()]),
            ..FakeIpConfig::default()
        };
        assert!(validate_fake_ip_config(&config).is_err());
    }

    #[test]
    fn test_apply_patch() {
        let mut config = FakeIpConfig::default();
        let patch = FakeIpConfigPatch {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            store_fake_ip: Some(true),
            ..FakeIpConfigPatch::default()
        };
        config.apply_patch(patch);
        assert_eq!(config.fake_ip_range, Some("198.18.0.1/16".to_string()));
        assert_eq!(config.store_fake_ip, Some(true));
    }

    #[test]
    fn test_apply_fake_ip_preserves_other_dns_settings() {
        let mut doc: Value =
            serde_yaml_ng::from_str("dns:\n  enable: true\n  nameserver:\n    - 8.8.8.8\n")
                .expect("yaml");
        let config = FakeIpConfig {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            ..FakeIpConfig::default()
        };
        apply_fake_ip_config(&mut doc, &config).expect("apply fake ip");

        let dns = doc.get("dns").expect("dns should exist");
        assert_eq!(dns.get("enable"), Some(&Value::Bool(true)));
        assert!(dns.get("nameserver").is_some());
        assert_eq!(
            dns.get("fake-ip-range"),
            Some(&Value::String("198.18.0.1/16".to_string()))
        );
    }

    #[test]
    fn test_validate_fake_ip_config_errors() {
        let config = FakeIpConfig {
            fake_ip_range: Some(" ".to_string()),
            ..FakeIpConfig::default()
        };
        assert!(validate_fake_ip_config(&config).is_err());

        let invalid_mode = FakeIpConfig {
            fake_ip_filter_mode: Some("unknown".to_string()),
            ..FakeIpConfig::default()
        };
        assert!(validate_fake_ip_config(&invalid_mode).is_err());
    }

    #[test]
    fn test_private_ip_collision_detector() {
        // Standard non-colliding fake-ip range (198.18.0.0/15 RFC 2544)
        let collisions = detect_private_ip_collision("198.18.0.1/16");
        assert!(collisions.is_empty(), "198.18.0.1/16 should not collide with private LAN");

        // Colliding with 192.168.0.0/16
        let collisions = detect_private_ip_collision("192.168.1.0/24");
        assert_eq!(collisions.len(), 1);
        assert!(collisions[0].contains("192.168.0.0/16"));

        // Colliding with 10.0.0.0/8
        let collisions = detect_private_ip_collision("10.10.0.0/16");
        assert_eq!(collisions.len(), 1);
        assert!(collisions[0].contains("10.0.0.0/8"));
    }

    #[test]
    fn test_fake_ip_pool_allocation_and_reverse_lookup() {
        let mut pool = FakeIpPool::new("198.18.0.0/28").expect("pool creation");
        assert_eq!(pool.capacity, 14); // /28 has 16 IPs, minus 2 = 14

        let ip1 = pool.allocate("google.com").expect("allocate ip1");
        assert!(pool.contains_ip(&ip1));
        assert_eq!(pool.reverse_lookup(&ip1), Some("google.com"));

        // Idempotent allocation for same domain
        let ip1_repeat = pool.allocate("google.com").expect("allocate repeat");
        assert_eq!(ip1, ip1_repeat);

        let ip2 = pool.allocate("youtube.com").expect("allocate ip2");
        assert_ne!(ip1, ip2);
        assert_eq!(pool.reverse_lookup(&ip2), Some("youtube.com"));

        let stats = pool.stats();
        assert_eq!(stats.total_allocated, 2);
        assert_eq!(stats.pool_capacity, 14);
    }

    #[test]
    fn test_fake_ip_pool_lru_eviction() {
        // Very small pool: /30 -> 4 IPs total, usable 2
        let mut pool = FakeIpPool::new("198.18.0.0/30").expect("small pool");
        assert_eq!(pool.capacity, 2);

        let ip1 = pool.allocate("domain1.com").expect("alloc 1");
        let _ip2 = pool.allocate("domain2.com").expect("alloc 2");

        // Allocating third domain should evict domain1.com (oldest)
        let ip3 = pool.allocate("domain3.com").expect("alloc 3");
        assert_eq!(ip3, ip1); // Re-used ip1
        assert_eq!(pool.reverse_lookup(&ip3), Some("domain3.com"));
        assert_eq!(pool.reverse_lookup(&ip1), Some("domain3.com"));
        assert!(!pool.domain_to_ip.contains_key("domain1.com"));
        assert!(pool.domain_to_ip.contains_key("domain2.com"));
    }

    #[test]
    fn test_domain_filtering_patterns() {
        let filters = default_anti_leak_filters();

        // Should match wildcard *.lan
        assert!(FakeIpPool::should_filter("myserver.lan", "blacklist", &filters));
        assert!(FakeIpPool::should_filter("nas.home.arpa", "blacklist", &filters));
        // Should match NTP
        assert!(FakeIpPool::should_filter("time.apple.com", "blacklist", &filters));
        assert!(FakeIpPool::should_filter("0.pool.ntp.org", "blacklist", &filters));
        // Should match STUN
        assert!(FakeIpPool::should_filter("stun.l.google.com", "blacklist", &filters));
        // Regular domains should NOT match
        assert!(!FakeIpPool::should_filter("github.com", "blacklist", &filters));
        assert!(!FakeIpPool::should_filter("rust-lang.org", "blacklist", &filters));

        // Test prefix match `+.example.com`
        let custom_filters = vec!["+.google.com".to_string()];
        assert!(FakeIpPool::should_filter("google.com", "blacklist", &custom_filters));
        assert!(FakeIpPool::should_filter("mail.google.com", "blacklist", &custom_filters));
        assert!(!FakeIpPool::should_filter("notgoogle.com", "blacklist", &custom_filters));
    }

    #[test]
    fn test_fake_ip_export_import_roundtrip() {
        let mut pool = FakeIpPool::new("198.18.0.0/24").expect("pool");
        pool.allocate("example.com").unwrap();
        pool.allocate("rust-lang.org").unwrap();

        let json = pool.export_mappings_json().expect("export");
        let mut restored_pool = FakeIpPool::new("198.18.0.0/24").expect("pool");
        let imported = restored_pool.import_mappings_json(&json).expect("import");
        assert_eq!(imported, 2);
        assert!(restored_pool.domain_to_ip.contains_key("example.com"));
        assert!(restored_pool.domain_to_ip.contains_key("rust-lang.org"));
    }
}
