use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// GeoIP and Autonomous System Number (ASN) metadata for an IP address.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct GeoInfo {
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub asn: Option<u32>,
    pub as_org: Option<String>,
    pub is_private_ip: bool,
}

impl GeoInfo {
    /// Creates a new `GeoInfo` record with the specified fields.
    pub fn new(
        country_code: Option<String>,
        city: Option<String>,
        asn: Option<u32>,
        as_org: Option<String>,
        is_private_ip: bool,
    ) -> Self {
        Self {
            country_code,
            city,
            asn,
            as_org,
            is_private_ip,
        }
    }

    /// Constructs a `GeoInfo` representing a private/loopback/local IP address.
    pub fn for_private_ip(ip: &IpAddr) -> Self {
        Self {
            country_code: Some("PRIVATE".to_string()),
            city: None,
            asn: None,
            as_org: Some("Private Network".to_string()),
            is_private_ip: is_private_ip(ip),
        }
    }

    /// Returns a human-friendly display label (e.g., "US - San Jose (AS15169 GOOGLE)").
    pub fn display_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref cc) = self.country_code {
            parts.push(cc.clone());
        }
        if let Some(ref city) = self.city {
            parts.push(city.clone());
        }
        let mut asn_part = String::new();
        if let Some(asn) = self.asn {
            asn_part.push_str(&format!("AS{}", asn));
        }
        if let Some(ref org) = self.as_org {
            if !asn_part.is_empty() {
                asn_part.push(' ');
            }
            asn_part.push_str(org);
        }
        if !asn_part.is_empty() {
            parts.push(format!("({})", asn_part));
        }
        if parts.is_empty() {
            "Unknown".to_string()
        } else {
            parts.join(" - ")
        }
    }
}

/// Checks whether an `IpAddr` is private, loopback, link-local, carrier-grade NAT, or reserved.
pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            ipv4.is_loopback()
                || ipv4.is_unspecified()
                || octets[0] == 10
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 169 && octets[1] == 254)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
                || ipv4.is_broadcast()
                || ipv4.is_multicast()
                || octets[0] >= 240
        }
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || segments[0] == 0x0100
        }
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    info: GeoInfo,
    expires_at: Instant,
    prev: Option<IpAddr>,
    next: Option<IpAddr>,
}

/// An LRU cache with time-to-live (TTL) expiration for IP -> GeoIP/ASN lookups.
pub struct GeoLookupCache {
    capacity: usize,
    ttl: Duration,
    entries: HashMap<IpAddr, CacheEntry>,
    head: Option<IpAddr>,
    tail: Option<IpAddr>,
    hits: u64,
    misses: u64,
}

impl GeoLookupCache {
    /// Creates a new `GeoLookupCache` with given maximum capacity and TTL duration.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity,
            ttl,
            entries: HashMap::with_capacity(capacity),
            head: None,
            tail: None,
            hits: 0,
            misses: 0,
        }
    }

    /// Creates a new cache using seconds for the TTL.
    pub fn from_secs(capacity: usize, ttl_secs: u64) -> Self {
        Self::new(capacity, Duration::from_secs(ttl_secs))
    }

    /// Looks up GeoIP information for the given IP address.
    /// Returns `Some(GeoInfo)` on cache hit if not expired, or `None` on cache miss/expiration.
    pub fn lookup(&mut self, ip: IpAddr) -> Option<GeoInfo> {
        self.lookup_at(ip, Instant::now())
    }

    /// Looks up GeoIP information with an explicit current timestamp (useful for deterministic tests).
    pub fn lookup_at(&mut self, ip: IpAddr, now: Instant) -> Option<GeoInfo> {
        if let Some(entry) = self.entries.get(&ip) {
            if now >= entry.expires_at {
                self.detach(&ip);
                self.entries.remove(&ip);
                self.misses += 1;
                None
            } else {
                self.move_to_head(&ip);
                self.hits += 1;
                Some(
                    self.entries
                        .get(&ip)
                        .expect("entry must exist")
                        .info
                        .clone(),
                )
            }
        } else {
            self.misses += 1;
            None
        }
    }

    /// Inserts or updates GeoIP information for the given IP address.
    pub fn insert(&mut self, ip: IpAddr, info: GeoInfo) {
        self.insert_at(ip, info, Instant::now());
    }

    /// Inserts or updates GeoIP information with an explicit current timestamp.
    pub fn insert_at(&mut self, ip: IpAddr, info: GeoInfo, now: Instant) {
        if self.capacity == 0 {
            return;
        }

        let expires_at = now + self.ttl;

        if self.entries.contains_key(&ip) {
            if let Some(entry) = self.entries.get_mut(&ip) {
                entry.info = info;
                entry.expires_at = expires_at;
            }
            self.move_to_head(&ip);
        } else {
            if self.entries.len() >= self.capacity {
                self.evict_lru();
            }
            let entry = CacheEntry {
                info,
                expires_at,
                prev: None,
                next: None,
            };
            self.entries.insert(ip, entry);
            self.attach_head(ip);
        }
    }

    /// Returns the cache hit rate as a fraction between `0.0` and `1.0`.
    /// Returns `0.0` if no lookups have been performed.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Returns the cache hit rate as a percentage between `0.0` and `100.0`.
    pub fn hit_rate_percent(&self) -> f64 {
        self.hit_rate() * 100.0
    }

    /// Purges all expired entries based on current system time. Returns number of purged entries.
    pub fn purge_expired(&mut self) -> usize {
        self.purge_expired_at(Instant::now())
    }

    /// Purges all expired entries based on a given timestamp.
    pub fn purge_expired_at(&mut self, now: Instant) -> usize {
        let expired_keys: Vec<IpAddr> = self
            .entries
            .iter()
            .filter_map(|(&ip, entry)| {
                if now >= entry.expires_at {
                    Some(ip)
                } else {
                    None
                }
            })
            .collect();

        let count = expired_keys.len();
        for ip in expired_keys {
            self.detach(&ip);
            self.entries.remove(&ip);
        }
        count
    }

    /// Returns the number of items currently stored in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache contains no items.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the configured capacity of the cache.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the total number of cache hits recorded.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Returns the total number of cache misses recorded.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Resets hit and miss counters.
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }

    /// Clears all entries from the cache while preserving capacity and stats.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.head = None;
        self.tail = None;
    }

    /// Helper to look up or lazily compute a `GeoInfo` record.
    pub fn get_or_insert_with<F>(&mut self, ip: IpAddr, fetcher: F) -> GeoInfo
    where
        F: FnOnce(IpAddr) -> GeoInfo,
    {
        if let Some(info) = self.lookup(ip) {
            info
        } else {
            let info = fetcher(ip);
            self.insert(ip, info.clone());
            info
        }
    }

    // --- Internal LRU Doubly-Linked List helpers ---

    fn detach(&mut self, ip: &IpAddr) {
        let (prev, next) = match self.entries.get(ip) {
            Some(e) => (e.prev, e.next),
            None => return,
        };

        if let Some(prev_ip) = prev {
            if let Some(prev_entry) = self.entries.get_mut(&prev_ip) {
                prev_entry.next = next;
            }
        } else {
            self.head = next;
        }

        if let Some(next_ip) = next {
            if let Some(next_entry) = self.entries.get_mut(&next_ip) {
                next_entry.prev = prev;
            }
        } else {
            self.tail = prev;
        }

        if let Some(entry) = self.entries.get_mut(ip) {
            entry.prev = None;
            entry.next = None;
        }
    }

    fn attach_head(&mut self, ip: IpAddr) {
        let old_head = self.head;
        if let Some(old_head_ip) = old_head {
            if let Some(old_head_entry) = self.entries.get_mut(&old_head_ip) {
                old_head_entry.prev = Some(ip);
            }
        } else {
            self.tail = Some(ip);
        }

        if let Some(entry) = self.entries.get_mut(&ip) {
            entry.prev = None;
            entry.next = old_head;
        }
        self.head = Some(ip);
    }

    fn move_to_head(&mut self, ip: &IpAddr) {
        if self.head == Some(*ip) {
            return;
        }
        let key = *ip;
        self.detach(&key);
        self.attach_head(key);
    }

    fn evict_lru(&mut self) {
        if let Some(tail_ip) = self.tail {
            self.detach(&tail_ip);
            self.entries.remove(&tail_ip);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn test_ip(d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, d))
    }

    fn sample_info(cc: &str, asn: u32) -> GeoInfo {
        GeoInfo {
            country_code: Some(cc.to_string()),
            city: Some("TestCity".to_string()),
            asn: Some(asn),
            as_org: Some("TestISP".to_string()),
            is_private_ip: false,
        }
    }

    #[test]
    fn test_insert_and_lookup_basic() {
        let mut cache = GeoLookupCache::from_secs(10, 60);
        let ip = test_ip(1);
        let info = sample_info("US", 15169);

        assert!(cache.lookup(ip).is_none());
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 0);

        cache.insert(ip, info.clone());
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        let retrieved = cache.lookup(ip);
        assert_eq!(retrieved, Some(info));
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.hit_rate(), 0.5);
        assert_eq!(cache.hit_rate_percent(), 50.0);
    }

    #[test]
    fn test_ttl_expiration_with_mock_time() {
        let mut cache = GeoLookupCache::from_secs(10, 60);
        let ip = test_ip(1);
        let info = sample_info("US", 15169);
        let start = Instant::now();

        cache.insert_at(ip, info.clone(), start);

        // Within TTL (30s elapsed)
        let active = cache.lookup_at(ip, start + Duration::from_secs(30));
        assert_eq!(active, Some(info));

        // After TTL (61s elapsed)
        let expired = cache.lookup_at(ip, start + Duration::from_secs(61));
        assert!(expired.is_none());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_lru_eviction_ordering() {
        let mut cache = GeoLookupCache::from_secs(3, 300);
        let ip1 = test_ip(1);
        let ip2 = test_ip(2);
        let ip3 = test_ip(3);
        let ip4 = test_ip(4);

        cache.insert(ip1, sample_info("US", 101));
        cache.insert(ip2, sample_info("DE", 102));
        cache.insert(ip3, sample_info("JP", 103));

        // Access ip1 so ordering becomes: ip1 (MRU), ip3, ip2 (LRU)
        assert!(cache.lookup(ip1).is_some());

        // Insert ip4, should evict ip2
        cache.insert(ip4, sample_info("SG", 104));

        assert_eq!(cache.len(), 3);
        assert!(cache.lookup(ip2).is_none());
        assert!(cache.lookup(ip1).is_some());
        assert!(cache.lookup(ip3).is_some());
        assert!(cache.lookup(ip4).is_some());
    }

    #[test]
    fn test_purge_expired() {
        let mut cache = GeoLookupCache::from_secs(10, 60);
        let start = Instant::now();

        cache.insert_at(test_ip(1), sample_info("US", 100), start);
        cache.insert_at(
            test_ip(2),
            sample_info("UK", 200),
            start + Duration::from_secs(30),
        );

        // At start + 65s: ip1 is expired (age 65s > 60s), ip2 is valid (age 35s <= 60s)
        let purged = cache.purge_expired_at(start + Duration::from_secs(65));
        assert_eq!(purged, 1);
        assert_eq!(cache.len(), 1);
        assert!(
            cache
                .lookup_at(test_ip(1), start + Duration::from_secs(65))
                .is_none()
        );
        assert!(
            cache
                .lookup_at(test_ip(2), start + Duration::from_secs(65))
                .is_some()
        );
    }

    #[test]
    fn test_private_ip_detection() {
        // IPv4 private/loopback
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"172.20.10.2".parse().unwrap()));
        assert!(is_private_ip(&"192.168.1.1".parse().unwrap()));
        assert!(is_private_ip(&"100.64.0.5".parse().unwrap()));
        assert!(is_private_ip(&"169.254.1.1".parse().unwrap()));
        assert!(is_private_ip(&"0.0.0.0".parse().unwrap()));
        assert!(is_private_ip(&"224.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"255.255.255.255".parse().unwrap()));

        // IPv4 public
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip(&"142.250.190.46".parse().unwrap()));

        // IPv6
        assert!(is_private_ip(&"::1".parse().unwrap()));
        assert!(is_private_ip(&"::".parse().unwrap()));
        assert!(is_private_ip(&"fe80::1".parse().unwrap()));
        assert!(is_private_ip(&"fd00::1".parse().unwrap()));
        assert!(!is_private_ip(&"2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn test_display_label_and_private_constructor() {
        let private_ip = "192.168.1.50".parse().unwrap();
        let p_info = GeoInfo::for_private_ip(&private_ip);
        assert!(p_info.is_private_ip);
        assert_eq!(p_info.display_label(), "PRIVATE - (Private Network)");

        let public_info = GeoInfo::new(
            Some("JP".to_string()),
            Some("Tokyo".to_string()),
            Some(2497),
            Some("IIJ".to_string()),
            false,
        );
        assert_eq!(public_info.display_label(), "JP - Tokyo - (AS2497 IIJ)");
    }

    #[test]
    fn test_get_or_insert_with() {
        let mut cache = GeoLookupCache::from_secs(10, 60);
        let ip = test_ip(42);
        let mut called = 0;

        let info1 = cache.get_or_insert_with(ip, |_| {
            called += 1;
            sample_info("HK", 999)
        });
        assert_eq!(info1.country_code.as_deref(), Some("HK"));
        assert_eq!(called, 1);

        // Second call should fetch from cache
        let info2 = cache.get_or_insert_with(ip, |_| {
            called += 1;
            sample_info("FR", 888)
        });
        assert_eq!(info2.country_code.as_deref(), Some("HK"));
        assert_eq!(called, 1);
    }

    #[test]
    fn test_zero_capacity_and_clear() {
        let mut cache = GeoLookupCache::from_secs(0, 60);
        cache.insert(test_ip(1), sample_info("US", 100));
        assert_eq!(cache.len(), 0);
        assert!(cache.lookup(test_ip(1)).is_none());

        let mut normal_cache = GeoLookupCache::from_secs(5, 60);
        normal_cache.insert(test_ip(1), sample_info("US", 100));
        normal_cache.clear();
        assert_eq!(normal_cache.len(), 0);
        assert!(normal_cache.lookup(test_ip(1)).is_none());
    }
}
