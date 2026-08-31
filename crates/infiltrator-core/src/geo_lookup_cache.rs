use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GeoLocationInfo {
    pub country_code: String,
    pub country_name: String,
    pub asn: Option<u32>,
    pub as_org: Option<String>,
}

pub struct GeoLookupCache {
    capacity: usize,
    ttl_secs: u64,
    map: HashMap<String, (GeoLocationInfo, u64, u64)>, // (info, timestamp_secs, access_id)
    order: VecDeque<(String, u64)>,                    // (ip, access_id)
    current_access_id: u64,
    hits: u64,
    misses: u64,
}

impl GeoLookupCache {
    pub fn new(capacity: usize, ttl_secs: u64) -> Self {
        Self {
            capacity,
            ttl_secs,
            map: HashMap::new(),
            order: VecDeque::new(),
            current_access_id: 0,
            hits: 0,
            misses: 0,
        }
    }

    pub fn insert(&mut self, ip: &str, info: GeoLocationInfo, timestamp_secs: u64) {
        self.current_access_id += 1;
        self.map.insert(
            ip.to_string(),
            (info, timestamp_secs, self.current_access_id),
        );
        self.order
            .push_back((ip.to_string(), self.current_access_id));
        self.evict_if_needed();
    }

    pub fn get(&mut self, ip: &str, now_secs: u64) -> Option<&GeoLocationInfo> {
        if let Some((_, timestamp, _)) = self.map.get(ip) {
            if now_secs.saturating_sub(*timestamp) > self.ttl_secs {
                self.map.remove(ip);
                self.misses += 1;
                return None;
            }
        } else {
            self.misses += 1;
            return None;
        }

        self.current_access_id += 1;
        let entry = self.map.get_mut(ip).unwrap();
        entry.2 = self.current_access_id;
        self.order
            .push_back((ip.to_string(), self.current_access_id));
        self.hits += 1;

        if self.order.len() > self.capacity.saturating_mul(2) {
            self.compact_order();
        }

        self.map.get(ip).map(|(info, _, _)| info)
    }

    pub fn hit_rate_percent(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }

    pub fn purge_expired(&mut self, now_secs: u64) -> usize {
        let initial_len = self.map.len();
        self.map
            .retain(|_, (_, timestamp, _)| now_secs.saturating_sub(*timestamp) <= self.ttl_secs);
        initial_len - self.map.len()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn evict_if_needed(&mut self) {
        while self.map.len() > self.capacity {
            while let Some((ip, access_id)) = self.order.pop_front() {
                if let Some((_, _, current_id)) = self.map.get(&ip)
                    && *current_id == access_id
                {
                    self.map.remove(&ip);
                    break;
                }
            }
        }
    }

    fn compact_order(&mut self) {
        let mut new_order = VecDeque::with_capacity(self.capacity);
        for (ip, access_id) in &self.order {
            if let Some((_, _, current_id)) = self.map.get(ip)
                && *current_id == *access_id
            {
                new_order.push_back((ip.clone(), *access_id));
            }
        }
        self.order = new_order;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_info(country_code: &str) -> GeoLocationInfo {
        GeoLocationInfo {
            country_code: country_code.to_string(),
            country_name: "TestCountry".to_string(),
            asn: Some(12345),
            as_org: Some("TestOrg".to_string()),
        }
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = GeoLookupCache::new(10, 60);
        cache.insert("1.1.1.1", create_info("US"), 100);

        let info = cache.get("1.1.1.1", 110);
        assert!(info.is_some());
        assert_eq!(info.unwrap().country_code, "US");
        assert_eq!(cache.hit_rate_percent(), 100.0);
    }

    #[test]
    fn test_ttl_expiration() {
        let mut cache = GeoLookupCache::new(10, 60);
        cache.insert("1.1.1.1", create_info("US"), 100);

        let info = cache.get("1.1.1.1", 170); // 170 - 100 = 70 > 60
        assert!(info.is_none());
        assert_eq!(cache.hit_rate_percent(), 0.0); // 1 miss
    }

    #[test]
    fn test_capacity_lru_eviction() {
        let mut cache = GeoLookupCache::new(2, 60);
        cache.insert("1.1.1.1", create_info("US"), 100);
        cache.insert("2.2.2.2", create_info("UK"), 100);

        // Access 1.1.1.1 so 2.2.2.2 becomes LRU
        cache.get("1.1.1.1", 110);

        // Insert 3.3.3.3 which should evict 2.2.2.2
        cache.insert("3.3.3.3", create_info("FR"), 120);

        assert_eq!(cache.len(), 2);
        assert!(cache.get("2.2.2.2", 120).is_none());
        assert!(cache.get("1.1.1.1", 120).is_some());
        assert!(cache.get("3.3.3.3", 120).is_some());
    }

    #[test]
    fn test_purge_expired() {
        let mut cache = GeoLookupCache::new(10, 60);
        cache.insert("1.1.1.1", create_info("US"), 100);
        cache.insert("2.2.2.2", create_info("UK"), 120);

        let purged = cache.purge_expired(170); // 170 - 100 = 70 (expired), 170 - 120 = 50 (valid)
        assert_eq!(purged, 1);
        assert_eq!(cache.len(), 1);
        assert!(cache.get("1.1.1.1", 170).is_none()); // already purged
        assert!(cache.get("2.2.2.2", 170).is_some());
    }

    #[test]
    fn test_hit_rate() {
        let mut cache = GeoLookupCache::new(10, 60);
        assert_eq!(cache.hit_rate_percent(), 0.0);

        cache.insert("1.1.1.1", create_info("US"), 100);

        cache.get("1.1.1.1", 110); // hit
        cache.get("2.2.2.2", 110); // miss

        assert_eq!(cache.hit_rate_percent(), 50.0);
    }
}
