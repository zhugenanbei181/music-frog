use super::*;
use crate::dns_topology::validate_dns_topology;
use serde_json::json;
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;

#[test]
fn test_extract_dns_default() {
    let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
    let config = extract_dns_config_from_doc(&doc).expect("dns config");
    assert!(config.enable.is_none());
    assert!(config.is_empty());
}

#[test]
fn test_apply_patch_and_validate() {
    let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
    let mut config = extract_dns_config_from_doc(&doc).expect("dns config");
    let patch = DnsConfigPayload {
        nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
        enhanced_mode: Some("fake-ip".to_string()),
        prefer_h3: Some(true),
        fake_ip_filter_mode: Some("blacklist".to_string()),
        cache_algorithm: Some("arc".to_string()),
        min_ttl: Some(60),
        max_ttl: Some(3600),
        ..DnsConfigPayload::default()
    };
    config.apply_patch(patch);
    validate_dns_config(&config).expect("valid dns config");
    assert_eq!(config.prefer_h3, Some(true));
    assert_eq!(config.fake_ip_filter_mode.as_deref(), Some("blacklist"));
    assert_eq!(config.cache_algorithm.as_deref(), Some("arc"));
    assert_eq!(config.min_ttl, Some(60));
    assert_eq!(config.max_ttl, Some(3600));
}

#[test]
fn test_apply_dns_config_removes_when_empty() {
    let mut doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
    let config = DnsConfig::default();
    apply_dns_config_to_doc(&mut doc, &config).expect("apply dns");
    let map = doc.as_mapping().expect("mapping");
    assert!(map.get(Value::String("dns".to_string())).is_none());
}

#[test]
fn test_apply_dns_config_writes_mapping() {
    let mut doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
    let config = DnsConfig {
        nameserver: Some(vec!["1.1.1.1".to_string()]),
        ..DnsConfig::default()
    };
    apply_dns_config_to_doc(&mut doc, &config).expect("apply dns");
    let map = doc.as_mapping().expect("mapping");
    assert!(map.get(Value::String("dns".to_string())).is_some());
}

#[test]
fn test_validate_rejects_empty_nameserver() {
    let config = DnsConfig {
        nameserver: Some(vec!["".to_string()]),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());
}

#[test]
fn test_apply_patch_full() {
    let mut config = DnsConfig::default();
    let mut policy = BTreeMap::new();
    policy.insert(
        "geosite:cn,private".to_string(),
        json!(["223.5.5.5", "119.29.29.29"]),
    );
    policy.insert(
        "geosite:geolocation-!cn".to_string(),
        json!("https://dns.cloudflare.com/dns-query"),
    );

    let patch = DnsConfigPayload {
        enable: Some(true),
        ipv6: Some(true),
        listen: Some("0.0.0.0:53".to_string()),
        default_nameserver: Some(vec!["223.5.5.5".to_string()]),
        nameserver: Some(vec!["8.8.8.8".to_string()]),
        fallback: Some(vec!["1.1.1.1".to_string()]),
        fallback_filter: Some(FallbackFilter {
            geoip: Some(true),
            geoip_code: Some("CN".to_string()),
            ipcidr: Some(vec!["240.0.0.0/4".to_string()]),
            domain: Some(vec!["+.google.com".to_string()]),
            domain_suffix: Some(vec!["google.com".to_string()]),
            geosite: Some(vec!["cn".to_string()]),
        }),
        enhanced_mode: Some("fake-ip".to_string()),
        fake_ip_range: Some("198.18.0.1/16".to_string()),
        fake_ip_filter: Some(vec!["*.lan".to_string()]),
        fake_ip_filter_mode: Some("blacklist".to_string()),
        use_hosts: Some(true),
        use_system_hosts: Some(false),
        respect_rules: Some(true),
        proxy_server_nameserver: Some(vec!["https://223.5.5.5/dns-query".to_string()]),
        direct_nameserver: Some(vec!["https://119.29.29.29/dns-query".to_string()]),
        nameserver_policy: Some(policy.clone()),
        cache: Some(true),
        prefer_h3: Some(true),
        edns_client_subnet: Some("101.0.0.0/24".to_string()),
        cache_algorithm: Some("arc".to_string()),
        max_ttl: Some(7200),
        min_ttl: Some(120),
        search_domains: Some(vec!["home.lan".to_string()]),
        ecs_override_policy: Some("strip".to_string()),
        bogus_nxdomain: Some(vec!["243.185.187.39".to_string()]),
        store_fake_ip: Some(true),
    };

    config.apply_patch(patch);
    validate_dns_config(&config).expect("valid full config");

    assert_eq!(config.enable, Some(true));
    assert_eq!(config.ipv6, Some(true));
    assert_eq!(config.listen, Some("0.0.0.0:53".to_string()));
    assert_eq!(
        config.default_nameserver,
        Some(vec!["223.5.5.5".to_string()])
    );
    assert_eq!(config.nameserver, Some(vec!["8.8.8.8".to_string()]));
    assert_eq!(config.fallback, Some(vec!["1.1.1.1".to_string()]));
    assert_eq!(config.enhanced_mode, Some("fake-ip".to_string()));
    assert_eq!(config.fake_ip_range, Some("198.18.0.1/16".to_string()));
    assert_eq!(config.fake_ip_filter_mode, Some("blacklist".to_string()));
    assert_eq!(config.use_hosts, Some(true));
    assert_eq!(config.use_system_hosts, Some(false));
    assert_eq!(config.respect_rules, Some(true));
    assert_eq!(
        config.proxy_server_nameserver,
        Some(vec!["https://223.5.5.5/dns-query".to_string()])
    );
    assert_eq!(
        config.direct_nameserver,
        Some(vec!["https://119.29.29.29/dns-query".to_string()])
    );
    assert_eq!(config.nameserver_policy, Some(policy));
    assert_eq!(config.cache, Some(true));
    assert_eq!(config.prefer_h3, Some(true));
    assert_eq!(config.edns_client_subnet, Some("101.0.0.0/24".to_string()));
    assert_eq!(config.cache_algorithm, Some("arc".to_string()));
    assert_eq!(config.max_ttl, Some(7200));
    assert_eq!(config.min_ttl, Some(120));
    assert_eq!(config.ecs_override_policy.as_deref(), Some("strip"));
    assert_eq!(config.store_fake_ip, Some(true));

    let filter = config.fallback_filter.as_ref().unwrap();
    assert_eq!(filter.geoip, Some(true));
    assert_eq!(filter.geoip_code, Some("CN".to_string()));
    assert_eq!(filter.geosite, Some(vec!["cn".to_string()]));
    assert_eq!(filter.domain_suffix, Some(vec!["google.com".to_string()]));
}

#[test]
fn test_apply_partial_patch_preserves_existing() {
    let mut config = DnsConfig {
        enable: Some(true),
        nameserver: Some(vec!["8.8.8.8".to_string()]),
        listen: Some("127.0.0.1:53".to_string()),
        prefer_h3: Some(true),
        direct_nameserver: Some(vec!["223.5.5.5".to_string()]),
        ..DnsConfig::default()
    };

    let patch = DnsConfigPayload {
        enable: Some(false),
        prefer_h3: Some(false),
        ..DnsConfigPayload::default()
    };

    config.apply_patch(patch);

    assert_eq!(config.enable, Some(false));
    assert_eq!(config.prefer_h3, Some(false));
    assert_eq!(config.nameserver, Some(vec!["8.8.8.8".to_string()]));
    assert_eq!(config.listen, Some("127.0.0.1:53".to_string()));
    assert_eq!(
        config.direct_nameserver,
        Some(vec!["223.5.5.5".to_string()])
    );
}

#[test]
fn test_apply_dns_config_preserves_other_sections() {
    let mut doc: Value =
        serde_yaml_ng::from_str("tun:\n  enable: true\nproxies:\n  - name: p1\n").expect("yaml");
    let config = DnsConfig {
        enable: Some(true),
        nameserver: Some(vec!["1.1.1.1".to_string()]),
        prefer_h3: Some(true),
        ..DnsConfig::default()
    };
    apply_dns_config_to_doc(&mut doc, &config).expect("apply dns");

    let map = doc.as_mapping().expect("mapping");
    assert!(map.get(Value::String("tun".to_string())).is_some());
    assert!(map.get(Value::String("proxies".to_string())).is_some());
    assert!(map.get(Value::String("dns".to_string())).is_some());
}

#[test]
fn test_mihomo_anti_leak_topology_roundtrip() {
    let yaml = r#"
dns:
  enable: true
  ipv6: false
  listen: 127.0.0.1:1053
  default-nameserver:
    - 223.5.5.5
    - 119.29.29.29
  proxy-server-nameserver:
    - https://223.5.5.5/dns-query
    - tls://1.12.12.12
  direct-nameserver:
    - https://223.5.5.5/dns-query
    - https://119.29.29.29/dns-query
  nameserver:
    - https://dns.google/dns-query#DNS
    - https://1.1.1.1/dns-query#DNS
  fallback:
    - https://8.8.8.8/dns-query#DNS
  fallback-filter:
    geoip: true
    geoip-code: CN
    geosite:
      - cn
      - geolocation-cn
    ipcidr:
      - 240.0.0.0/4
    domain:
      - "+.google.com"
      - "+.youtube.com"
    domain-suffix:
      - google.com
  nameserver-policy:
    "geosite:cn,private":
      - 223.5.5.5
      - 119.29.29.29
    "geosite:geolocation-!cn": https://dns.cloudflare.com/dns-query#DNS
    "rule-set:direct": 223.5.5.5
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16
  fake-ip-filter:
    - "*.lan"
    - "*.local"
  fake-ip-filter-mode: blacklist
  store-fake-ip: true
  use-hosts: true
  use-system-hosts: false
  respect-rules: true
  cache: true
  prefer-h3: true
  edns-client-subnet: 101.0.0.0/24
  ecs-override-policy: strip
  cache-algorithm: arc
  max-ttl: 3600
  min-ttl: 60
"#;

    let doc: Value = serde_yaml_ng::from_str(yaml).expect("parse yaml");
    let config = extract_dns_config_from_doc(&doc).expect("extract dns");
    validate_dns_config(&config).expect("validate");

    assert_eq!(config.enable, Some(true));
    assert_eq!(config.ipv6, Some(false));
    assert_eq!(config.listen.as_deref(), Some("127.0.0.1:1053"));
    assert_eq!(
        config.default_nameserver,
        Some(vec!["223.5.5.5".to_string(), "119.29.29.29".to_string()])
    );
    assert_eq!(
        config.proxy_server_nameserver,
        Some(vec![
            "https://223.5.5.5/dns-query".to_string(),
            "tls://1.12.12.12".to_string(),
        ])
    );
    assert_eq!(
        config.direct_nameserver,
        Some(vec![
            "https://223.5.5.5/dns-query".to_string(),
            "https://119.29.29.29/dns-query".to_string(),
        ])
    );
    assert_eq!(config.prefer_h3, Some(true));
    assert_eq!(config.fake_ip_filter_mode.as_deref(), Some("blacklist"));
    assert_eq!(config.store_fake_ip, Some(true));
    assert_eq!(config.cache_algorithm.as_deref(), Some("arc"));
    assert_eq!(config.edns_client_subnet.as_deref(), Some("101.0.0.0/24"));
    assert_eq!(config.ecs_override_policy.as_deref(), Some("strip"));
    assert_eq!(config.max_ttl, Some(3600));
    assert_eq!(config.min_ttl, Some(60));

    let filter = config.fallback_filter.as_ref().unwrap();
    assert_eq!(filter.geoip, Some(true));
    assert_eq!(filter.geoip_code.as_deref(), Some("CN"));
    assert_eq!(
        filter.geosite,
        Some(vec!["cn".to_string(), "geolocation-cn".to_string()])
    );

    let policy = config.nameserver_policy.as_ref().unwrap();
    assert_eq!(
        policy.get("geosite:cn,private"),
        Some(&json!(["223.5.5.5", "119.29.29.29"]))
    );
    assert_eq!(
        policy.get("geosite:geolocation-!cn"),
        Some(&json!("https://dns.cloudflare.com/dns-query#DNS"))
    );
    assert_eq!(policy.get("rule-set:direct"), Some(&json!("223.5.5.5")));

    // Re-apply to doc and check serialization
    let mut new_doc = Value::Mapping(Mapping::new());
    apply_dns_config_to_doc(&mut new_doc, &config).expect("apply to doc");
    let serialized = serde_yaml_ng::to_string(&new_doc).expect("serialize");

    assert!(serialized.contains("default-nameserver:"));
    assert!(serialized.contains("proxy-server-nameserver:"));
    assert!(serialized.contains("direct-nameserver:"));
    assert!(serialized.contains("nameserver-policy:"));
    assert!(serialized.contains("fake-ip-filter-mode: blacklist"));
    assert!(serialized.contains("prefer-h3: true"));
    assert!(serialized.contains("cache-algorithm: arc"));
    assert!(serialized.contains("max-ttl: 3600"));
    assert!(serialized.contains("min-ttl: 60"));
    assert!(serialized.contains("edns-client-subnet: 101.0.0.0/24"));
    assert!(serialized.contains("ecs-override-policy: strip"));
    assert!(serialized.contains("store-fake-ip: true"));
    assert!(serialized.contains("geosite:"));
}

#[test]
fn test_pure_ip_bootstrap_nameserver_enforcement() {
    // Valid pure IPs
    assert!(is_pure_ip_server("223.5.5.5"));
    assert!(is_pure_ip_server("119.29.29.29:53"));
    assert!(is_pure_ip_server("udp://1.1.1.1:53"));
    assert!(is_pure_ip_server("2400:3200::1"));
    assert!(is_pure_ip_server("[2400:3200::1]:53"));

    // Invalid non-IP domain bootstrap
    assert!(!is_pure_ip_server("dns.alidns.com"));
    assert!(!is_pure_ip_server("https://dns.google/dns-query"));
    assert!(!is_pure_ip_server("tls://1dot1dot1dot1.cloudflare-dns.com"));

    // Validation fails if default-nameserver contains domain
    let config = DnsConfig {
        default_nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());
}

#[test]
fn test_parse_upstream_uri_protocols() {
    // UDP
    let u1 = parse_upstream_uri("1.1.1.1").expect("udp");
    assert_eq!(u1.protocol, DnsUpstreamProtocol::Udp);
    assert_eq!(u1.host, "1.1.1.1");
    assert_eq!(u1.port, 53);

    // TCP with port and tag
    let u2 = parse_upstream_uri("tcp://8.8.8.8:5353#DIRECT").expect("tcp");
    assert_eq!(u2.protocol, DnsUpstreamProtocol::Tcp);
    assert_eq!(u2.host, "8.8.8.8");
    assert_eq!(u2.port, 5353);
    assert_eq!(u2.outbound_tag.as_deref(), Some("DIRECT"));

    // DoT
    let u3 = parse_upstream_uri("tls://1.1.1.1:853#DNS").expect("dot");
    assert_eq!(u3.protocol, DnsUpstreamProtocol::DoT);
    assert_eq!(u3.host, "1.1.1.1");
    assert_eq!(u3.port, 853);
    assert_eq!(u3.outbound_tag.as_deref(), Some("DNS"));

    // DoH
    let u4 = parse_upstream_uri("https://dns.google/dns-query#Proxy").expect("doh");
    assert_eq!(u4.protocol, DnsUpstreamProtocol::DoH);
    assert_eq!(u4.host, "dns.google");
    assert_eq!(u4.port, 443);
    assert_eq!(u4.path.as_deref(), Some("/dns-query"));
    assert_eq!(u4.outbound_tag.as_deref(), Some("Proxy"));

    // DoH3 with query param
    let u5 = parse_upstream_uri("https://cloudflare-dns.com/dns-query?h3=true#Proxy").expect("doh3");
    assert_eq!(u5.protocol, DnsUpstreamProtocol::DoH3);
    assert_eq!(u5.host, "cloudflare-dns.com");
    assert_eq!(u5.port, 443);
    assert_eq!(u5.params.get("h3").map(|s| s.as_str()), Some("true"));

    // DoQ (RFC 9250)
    let u6 = parse_upstream_uri("quic://dns.adguard.com:853#Proxy").expect("doq");
    assert_eq!(u6.protocol, DnsUpstreamProtocol::DoQ);
    assert_eq!(u6.host, "dns.adguard.com");
    assert_eq!(u6.port, 853);

    // DNSCrypt
    let u7 = parse_upstream_uri("sdns://AQMAAAAAAAAADDk0LjE0MC4xNC4xNA").expect("dnscrypt");
    assert_eq!(u7.protocol, DnsUpstreamProtocol::DnsCrypt);
}

#[test]
fn test_sanitize_ecs_subnet() {
    // IPv4 host to network sanitization
    assert_eq!(
        sanitize_ecs_subnet("101.10.20.30/24").unwrap(),
        "101.10.20.0/24"
    );
    assert_eq!(
        sanitize_ecs_subnet("1.2.3.4/32").unwrap(),
        "1.2.3.4/32"
    );
    assert_eq!(
        sanitize_ecs_subnet("0.0.0.0/0").unwrap(),
        "0.0.0.0/0"
    );

    // IPv6 host to network sanitization
    assert_eq!(
        sanitize_ecs_subnet("2400:3200:100:234::1/56").unwrap(),
        "2400:3200:100:200::/56"
    );

    // Invalid subnet formats
    assert!(sanitize_ecs_subnet("101.10.20.30/33").is_err());
    assert!(sanitize_ecs_subnet("invalid/24").is_err());
    assert!(sanitize_ecs_subnet("101.10.20.30").is_err());
}

#[test]
fn test_topology_diagnostics() {
    // Config with missing bootstrap
    let config = DnsConfig {
        enable: Some(true),
        nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
        enhanced_mode: Some("fake-ip".to_string()),
        fallback_filter: Some(FallbackFilter {
            geoip: Some(true),
            ..FallbackFilter::default()
        }),
        edns_client_subnet: Some("101.0.0.0/24".to_string()),
        ..DnsConfig::default()
    };

    let diags = validate_dns_topology(&config);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();

    assert!(codes.contains(&"BOOTSTRAP_DNS_MISSING"));
    assert!(codes.contains(&"FALLBACK_FILTER_WITHOUT_FALLBACK"));
    assert!(codes.contains(&"ECS_PRIVACY_LEAK_RISK"));
    assert!(codes.contains(&"FAKE_IP_PERSISTENCE_DISABLED"));
}

#[test]
fn test_validate_dns_config_all_errors() {
    // Empty listen
    let config = DnsConfig {
        listen: Some(" ".to_string()),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid enhanced_mode
    let config = DnsConfig {
        enhanced_mode: Some("vpn".to_string()),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Empty fake_ip_range
    let config = DnsConfig {
        fake_ip_range: Some("".to_string()),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid fake_ip_filter_mode
    let config = DnsConfig {
        fake_ip_filter_mode: Some("grey".to_string()),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid cache_algorithm
    let config = DnsConfig {
        cache_algorithm: Some("lfu".to_string()),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Empty edns_client_subnet
    let config = DnsConfig {
        edns_client_subnet: Some("   ".to_string()),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // min_ttl > max_ttl
    let config = DnsConfig {
        min_ttl: Some(300),
        max_ttl: Some(60),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Empty entry in default_nameserver
    let config = DnsConfig {
        default_nameserver: Some(vec!["1.1.1.1".to_string(), " ".to_string()]),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Empty entry in proxy_server_nameserver
    let config = DnsConfig {
        proxy_server_nameserver: Some(vec!["".to_string()]),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Empty entry in direct_nameserver
    let config = DnsConfig {
        direct_nameserver: Some(vec![" ".to_string()]),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Empty entry in fake_ip_filter
    let config = DnsConfig {
        fake_ip_filter: Some(vec!["".to_string()]),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid nameserver-policy empty key
    let mut policy = BTreeMap::new();
    policy.insert("".to_string(), json!("8.8.8.8"));
    let config = DnsConfig {
        nameserver_policy: Some(policy),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid nameserver-policy empty string target
    let mut policy = BTreeMap::new();
    policy.insert("geosite:cn".to_string(), json!("  "));
    let config = DnsConfig {
        nameserver_policy: Some(policy),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid nameserver-policy empty array target
    let mut policy = BTreeMap::new();
    policy.insert("geosite:cn".to_string(), json!([]));
    let config = DnsConfig {
        nameserver_policy: Some(policy),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid nameserver-policy array with empty entry
    let mut policy = BTreeMap::new();
    policy.insert("geosite:cn".to_string(), json!(["223.5.5.5", ""]));
    let config = DnsConfig {
        nameserver_policy: Some(policy),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid nameserver-policy null target
    let mut policy = BTreeMap::new();
    policy.insert("geosite:cn".to_string(), json!(null));
    let config = DnsConfig {
        nameserver_policy: Some(policy),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Invalid nameserver-policy numeric target
    let mut policy = BTreeMap::new();
    policy.insert("geosite:cn".to_string(), json!(1234));
    let config = DnsConfig {
        nameserver_policy: Some(policy),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Fallback filter empty geoip-code
    let config = DnsConfig {
        fallback_filter: Some(FallbackFilter {
            geoip_code: Some("  ".to_string()),
            ..FallbackFilter::default()
        }),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());

    // Fallback filter empty geosite entry
    let config = DnsConfig {
        fallback_filter: Some(FallbackFilter {
            geosite: Some(vec!["".to_string()]),
            ..FallbackFilter::default()
        }),
        ..DnsConfig::default()
    };
    assert!(validate_dns_config(&config).is_err());
}

#[test]
fn test_apply_dns_patch_to_yaml() {
    let yaml = "port: 7890\ndns:\n  enable: true\n  nameserver:\n    - 8.8.8.8\n";
    let patch = DnsConfigPayload {
        prefer_h3: Some(true),
        cache_algorithm: Some("lru".to_string()),
        min_ttl: Some(30),
        max_ttl: Some(300),
        ..DnsConfigPayload::default()
    };

    let updated = apply_dns_patch_to_yaml(yaml, patch).expect("apply patch");
    let doc: Value = serde_yaml_ng::from_str(&updated).expect("parse updated");
    let config = extract_dns_config_from_doc(&doc).expect("extract config");

    assert_eq!(config.enable, Some(true));
    assert_eq!(config.nameserver, Some(vec!["8.8.8.8".to_string()]));
    assert_eq!(config.prefer_h3, Some(true));
    assert_eq!(config.cache_algorithm, Some("lru".to_string()));
    assert_eq!(config.min_ttl, Some(30));
    assert_eq!(config.max_ttl, Some(300));
}

#[test]
fn test_apply_dns_config_to_yaml() {
    let yaml = "port: 7890\n";
    let patch = DnsConfigPayload {
        enable: Some(true),
        nameserver: Some(vec!["1.1.1.1".to_string()]),
        direct_nameserver: Some(vec!["223.5.5.5".to_string()]),
        ..DnsConfigPayload::default()
    };

    let updated = apply_dns_patch_to_yaml(yaml, patch).expect("apply config");
    assert!(updated.contains("direct-nameserver:"));
    assert!(updated.contains("223.5.5.5"));
}

#[test]
fn test_payload_config_conversions() {
    let config = DnsConfig {
        enable: Some(true),
        prefer_h3: Some(true),
        cache_algorithm: Some("arc".to_string()),
        ..DnsConfig::default()
    };
    let payload: DnsConfigPayload = config.clone().into();
    assert_eq!(payload.enable, Some(true));
    assert_eq!(payload.prefer_h3, Some(true));
    assert_eq!(payload.cache_algorithm, Some("arc".to_string()));

    let converted_back: DnsConfig = payload.into();
    assert_eq!(converted_back, config);
}

#[tokio::test]
async fn test_dns_io_follows_configs_dir_redirect() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().to_path_buf();
    let cloud = home.join("cloud-sync").join("profiles");
    std::fs::create_dir_all(&cloud).unwrap();
    let guard = crate::settings::test_support::RedirectGuard::acquire(home.clone()).await;
    guard
        .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
        .await;

    let seed = crate::settings::app_config_manager().await.unwrap();
    seed.save("main", "port: 7890\n").await.unwrap();
    seed.set_current("main").await.unwrap();

    let saved = save_dns_config(DnsConfigPayload {
        enable: Some(true),
        prefer_h3: Some(true),
        direct_nameserver: Some(vec!["223.5.5.5".to_string()]),
        ..DnsConfigPayload::default()
    })
    .await
    .unwrap();
    assert_eq!(saved.enable, Some(true));
    assert_eq!(saved.prefer_h3, Some(true));
    assert_eq!(saved.direct_nameserver, Some(vec!["223.5.5.5".to_string()]));

    let loaded = load_dns_config().await.unwrap();
    assert_eq!(loaded.enable, Some(true));
    assert_eq!(loaded.prefer_h3, Some(true));
    assert_eq!(
        loaded.direct_nameserver,
        Some(vec!["223.5.5.5".to_string()])
    );
    assert!(cloud.join("main.yaml").is_file());
    assert!(!home.join("configs").exists());
}
