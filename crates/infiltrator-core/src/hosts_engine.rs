use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single entry parsed from a hosts file or added programmatically.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HostsEntry {
    pub domain_pattern: String,
    pub target_ip: String,
    pub is_wildcard: bool,
}

/// The engine responsible for static hosts mapping and wildcard domain resolution.
#[derive(Debug, Clone, Default)]
pub struct HostsEngine {
    /// All entries added to the engine
    entries: Vec<HostsEntry>,
    /// Exact domain to IP mapping
    exact_matches: HashMap<String, String>,
    /// Wildcard suffixes to IP mapping (suffix without `*.`, target_ip)
    wildcard_matches: Vec<(String, String)>,
    /// Reverse mapping from IP to domains
    reverse_map: HashMap<String, Vec<String>>,
}

impl HostsEngine {
    /// Creates a new, empty HostsEngine.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            exact_matches: HashMap::new(),
            wildcard_matches: Vec::new(),
            reverse_map: HashMap::new(),
        }
    }

    /// Parses the content of a hosts file and returns a vector of HostsEntry.
    /// Handles comments `#` and inline whitespace, as well as multiple domains per IP.
    pub fn parse_hosts_file(content: &str) -> Vec<HostsEntry> {
        let mut entries = Vec::new();
        for line in content.lines() {
            // Strip comments and trim whitespace
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.split_whitespace();
            if let Some(ip) = parts.next() {
                let target_ip = ip.to_string();
                for domain in parts {
                    let is_wildcard = domain.starts_with("*.");
                    entries.push(HostsEntry {
                        domain_pattern: domain.to_string(),
                        target_ip: target_ip.clone(),
                        is_wildcard,
                    });
                }
            }
        }
        entries
    }

    /// Adds a single HostsEntry to the engine for resolution.
    pub fn add_entry(&mut self, entry: HostsEntry) {
        self.entries.push(entry.clone());

        // Update reverse mapping
        let reverse_entry = self.reverse_map.entry(entry.target_ip.clone()).or_default();
        if !reverse_entry.contains(&entry.domain_pattern) {
            reverse_entry.push(entry.domain_pattern.clone());
        }

        // Update forward mappings
        if entry.is_wildcard {
            let suffix = entry.domain_pattern.trim_start_matches("*.").to_string();
            self.wildcard_matches.push((suffix, entry.target_ip));
            // Sort by descending suffix length for most specific match first
            self.wildcard_matches.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        } else {
            self.exact_matches.insert(entry.domain_pattern, entry.target_ip);
        }
    }

    /// Resolves a domain to its mapped IP address, checking exact matches first,
    /// then wildcard suffixes.
    pub fn resolve(&self, domain: &str) -> Option<&str> {
        if let Some(ip) = self.exact_matches.get(domain) {
            return Some(ip.as_str());
        }

        for (suffix, ip) in &self.wildcard_matches {
            if domain == suffix {
                return Some(ip.as_str());
            }
            if domain.ends_with(suffix) && domain.len() > suffix.len() && domain.as_bytes()[domain.len() - suffix.len() - 1] == b'.' {
                return Some(ip.as_str());
            }
        }

        None
    }

    /// Performs a reverse lookup from an IP address to all associated domain patterns.
    pub fn reverse_lookup(&self, ip: &str) -> Vec<&str> {
        self.reverse_map
            .get(ip)
            .map(|domains| domains.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Clears all entries and mappings from the engine.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.exact_matches.clear();
        self.wildcard_matches.clear();
        self.reverse_map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hosts_file() {
        let content = "\
# This is a sample hosts file
127.0.0.1 localhost
192.168.1.1 router.lan *.router.lan # This is a router
";
        let entries = HostsEngine::parse_hosts_file(content);
        assert_eq!(entries.len(), 3);
        
        assert_eq!(entries[0], HostsEntry {
            domain_pattern: "localhost".to_string(),
            target_ip: "127.0.0.1".to_string(),
            is_wildcard: false,
        });
        
        assert_eq!(entries[1], HostsEntry {
            domain_pattern: "router.lan".to_string(),
            target_ip: "192.168.1.1".to_string(),
            is_wildcard: false,
        });
        
        assert_eq!(entries[2], HostsEntry {
            domain_pattern: "*.router.lan".to_string(),
            target_ip: "192.168.1.1".to_string(),
            is_wildcard: true,
        });
    }

    #[test]
    fn test_resolve_exact() {
        let mut engine = HostsEngine::new();
        engine.add_entry(HostsEntry {
            domain_pattern: "example.com".to_string(),
            target_ip: "10.0.0.1".to_string(),
            is_wildcard: false,
        });
        
        assert_eq!(engine.resolve("example.com"), Some("10.0.0.1"));
        assert_eq!(engine.resolve("www.example.com"), None);
        assert_eq!(engine.resolve("other.com"), None);
    }

    #[test]
    fn test_resolve_wildcard() {
        let mut engine = HostsEngine::new();
        engine.add_entry(HostsEntry {
            domain_pattern: "*.example.com".to_string(),
            target_ip: "10.0.0.2".to_string(),
            is_wildcard: true,
        });
        
        assert_eq!(engine.resolve("test.example.com"), Some("10.0.0.2"));
        assert_eq!(engine.resolve("a.b.example.com"), Some("10.0.0.2"));
        assert_eq!(engine.resolve("example.com"), Some("10.0.0.2"));
        assert_eq!(engine.resolve("notexample.com"), None);
    }

    #[test]
    fn test_reverse_lookup() {
        let mut engine = HostsEngine::new();
        engine.add_entry(HostsEntry {
            domain_pattern: "host1.local".to_string(),
            target_ip: "192.168.0.5".to_string(),
            is_wildcard: false,
        });
        engine.add_entry(HostsEntry {
            domain_pattern: "host2.local".to_string(),
            target_ip: "192.168.0.5".to_string(),
            is_wildcard: false,
        });
        
        let mut domains = engine.reverse_lookup("192.168.0.5");
        domains.sort();
        
        assert_eq!(domains.len(), 2);
        assert_eq!(domains, vec!["host1.local", "host2.local"]);
        
        assert!(engine.reverse_lookup("192.168.0.6").is_empty());
    }

    #[test]
    fn test_clear() {
        let mut engine = HostsEngine::new();
        engine.add_entry(HostsEntry {
            domain_pattern: "localhost".to_string(),
            target_ip: "127.0.0.1".to_string(),
            is_wildcard: false,
        });
        
        assert_eq!(engine.resolve("localhost"), Some("127.0.0.1"));
        engine.clear();
        assert_eq!(engine.resolve("localhost"), None);
    }
}
