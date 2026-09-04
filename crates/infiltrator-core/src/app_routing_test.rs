use super::*;
use std::net::IpAddr;
use std::sync::Arc;
use std::thread;

#[test]
fn test_should_proxy() {
    let mut config = AppRoutingConfig::default();

    // ProxyAll mode
    assert!(config.should_proxy("com.example.app"));

    // ProxySelected mode
    config.mode = AppRoutingMode::ProxySelected;
    config.packages.insert("com.example.app".to_string());
    assert!(config.should_proxy("com.example.app"));
    assert!(!config.should_proxy("com.other.app"));

    // BypassSelected mode
    config.mode = AppRoutingMode::BypassSelected;
    assert!(!config.should_proxy("com.example.app"));
    assert!(config.should_proxy("com.other.app"));
}

#[test]
fn test_get_allowed_packages() {
    let mut config = AppRoutingConfig::default();

    // ProxyAll returns None
    assert!(config.get_allowed_packages().is_none());

    // ProxySelected with packages
    config.mode = AppRoutingMode::ProxySelected;
    config.packages.insert("com.example.app".to_string());
    let allowed = config.get_allowed_packages().unwrap();
    assert!(allowed.contains(&"com.example.app".to_string()));
}

#[test]
fn test_get_disallowed_packages() {
    let mut config = AppRoutingConfig {
        mode: AppRoutingMode::BypassSelected,
        ..AppRoutingConfig::default()
    };
    config.packages.insert("com.example.app".to_string());

    let disallowed = config.get_disallowed_packages().unwrap();
    assert_eq!(disallowed.len(), 1);
    assert_eq!(disallowed[0], "com.example.app");

    config.mode = AppRoutingMode::ProxyAll;
    assert!(config.get_disallowed_packages().is_none());
}

#[test]
fn test_app_routing_serialization() {
    let mut config = AppRoutingConfig {
        mode: AppRoutingMode::ProxySelected,
        ..AppRoutingConfig::default()
    };
    config.packages.insert("com.test".to_string());

    let toml_str = toml::to_string(&config).unwrap();
    assert!(toml_str.contains("proxy_selected"));
    assert!(toml_str.contains("com.test"));
}

// ============================================================================
// CorporateSubnetDetector Tests
// ============================================================================

#[test]
fn test_corporate_subnet_detector_defaults() {
    let detector = CorporateSubnetDetector::new();
    assert_eq!(CorporateSubnetDetector::DEFAULT_SUBNETS.len(), 6);
    assert!(detector.custom_subnets().is_empty());

    // Verify all 6 required default subnets
    let rules = detector.generate_direct_bypass_rules();
    assert_eq!(rules.len(), 6);

    let rule_strs: Vec<&str> = rules.iter().map(|r| r.rule.as_str()).collect();
    assert!(rule_strs.contains(&"IP-CIDR,10.0.0.0/8,DIRECT,no-resolve"));
    assert!(rule_strs.contains(&"IP-CIDR,172.16.0.0/12,DIRECT,no-resolve"));
    assert!(rule_strs.contains(&"IP-CIDR,192.168.0.0/16,DIRECT,no-resolve"));
    assert!(rule_strs.contains(&"IP-CIDR,100.64.0.0/10,DIRECT,no-resolve"));
    assert!(rule_strs.contains(&"IP-CIDR6,fd00::/8,DIRECT,no-resolve"));
    assert!(rule_strs.contains(&"IP-CIDR6,fe80::/10,DIRECT,no-resolve"));

    for rule in &rules {
        assert!(rule.enabled);
    }
}

#[test]
fn test_corporate_subnet_detector_ipv4_private() {
    let detector = CorporateSubnetDetector::new();

    // 10.0.0.0/8
    let ip1: IpAddr = "10.0.0.1".parse().unwrap();
    let ip2: IpAddr = "10.255.255.254".parse().unwrap();
    assert!(detector.is_corporate_or_private(&ip1));
    assert!(detector.is_corporate_or_private(&ip2));
    assert_eq!(
        detector.classify_ip(&ip1),
        Some(SubnetCategory::PrivateIpv4)
    );
    assert_eq!(
        detector.matching_subnet(&ip1),
        Some("10.0.0.0/8".to_string())
    );

    // 172.16.0.0/12 (172.16.0.0 - 172.31.255.255)
    let ip3: IpAddr = "172.16.0.1".parse().unwrap();
    let ip4: IpAddr = "172.31.255.255".parse().unwrap();
    let ip_out1: IpAddr = "172.15.255.255".parse().unwrap();
    let ip_out2: IpAddr = "172.32.0.1".parse().unwrap();
    assert!(detector.is_corporate_or_private(&ip3));
    assert!(detector.is_corporate_or_private(&ip4));
    assert!(!detector.is_corporate_or_private(&ip_out1));
    assert!(!detector.is_corporate_or_private(&ip_out2));
    assert_eq!(
        detector.classify_ip(&ip3),
        Some(SubnetCategory::PrivateIpv4)
    );

    // 192.168.0.0/16
    let ip5: IpAddr = "192.168.1.1".parse().unwrap();
    let ip6: IpAddr = "192.168.254.254".parse().unwrap();
    let ip_out3: IpAddr = "192.169.1.1".parse().unwrap();
    assert!(detector.is_corporate_or_private(&ip5));
    assert!(detector.is_corporate_or_private(&ip6));
    assert!(!detector.is_corporate_or_private(&ip_out3));
    assert_eq!(
        detector.classify_ip(&ip5),
        Some(SubnetCategory::PrivateIpv4)
    );
}

#[test]
fn test_corporate_subnet_detector_cgnat() {
    let detector = CorporateSubnetDetector::new();

    // 100.64.0.0/10 (100.64.0.0 - 100.127.255.255)
    let cgnat1: IpAddr = "100.64.0.1".parse().unwrap();
    let cgnat2: IpAddr = "100.127.255.254".parse().unwrap();
    let cgnat_out1: IpAddr = "100.63.255.255".parse().unwrap();
    let cgnat_out2: IpAddr = "100.128.0.1".parse().unwrap();

    assert!(detector.is_corporate_or_private(&cgnat1));
    assert!(detector.is_corporate_or_private(&cgnat2));
    assert_eq!(detector.classify_ip(&cgnat1), Some(SubnetCategory::Cgnat));
    assert!(!detector.is_corporate_or_private(&cgnat_out1));
    assert!(!detector.is_corporate_or_private(&cgnat_out2));
}

#[test]
fn test_corporate_subnet_detector_ipv6() {
    let detector = CorporateSubnetDetector::new();

    // fd00::/8 (ULA)
    let ula1: IpAddr = "fd00::1".parse().unwrap();
    let ula2: IpAddr = "fd12:3456:789a::dead:beef".parse().unwrap();
    let non_ula: IpAddr = "2001:db8::1".parse().unwrap();

    assert!(detector.is_corporate_or_private(&ula1));
    assert!(detector.is_corporate_or_private(&ula2));
    assert_eq!(
        detector.classify_ip(&ula1),
        Some(SubnetCategory::PrivateIpv6)
    );
    assert!(!detector.is_corporate_or_private(&non_ula));

    // fe80::/10 (Link-Local)
    let ll1: IpAddr = "fe80::1".parse().unwrap();
    let ll2: IpAddr = "febf::ffff".parse().unwrap();
    let non_ll1: IpAddr = "fec0::1".parse().unwrap();
    let non_ll2: IpAddr = "fe7f::1".parse().unwrap();

    assert!(detector.is_corporate_or_private(&ll1));
    assert!(detector.is_corporate_or_private(&ll2));
    assert_eq!(
        detector.classify_ip(&ll1),
        Some(SubnetCategory::LinkLocalIpv6)
    );
    assert!(!detector.is_corporate_or_private(&non_ll1));
    assert!(!detector.is_corporate_or_private(&non_ll2));
}

#[test]
fn test_corporate_subnet_detector_custom_subnets() {
    let mut detector = CorporateSubnetDetector::new();
    assert!(detector.add_custom_subnet("198.51.100.0/24").is_ok());
    assert!(detector.add_custom_subnet("2001:db8:abcd::/48").is_ok());
    assert!(detector.add_custom_subnet("invalid-cidr").is_err());

    let test_ip1: IpAddr = "198.51.100.42".parse().unwrap();
    let test_ip2: IpAddr = "2001:db8:abcd::1".parse().unwrap();
    let test_ip_out: IpAddr = "198.51.101.1".parse().unwrap();

    assert!(detector.is_corporate_or_private(&test_ip1));
    assert!(detector.is_corporate_or_private(&test_ip2));
    assert!(!detector.is_corporate_or_private(&test_ip_out));

    assert_eq!(
        detector.classify_ip(&test_ip1),
        Some(SubnetCategory::CustomCorporate)
    );
    assert_eq!(
        detector.matching_subnet(&test_ip1),
        Some("198.51.100.0/24".to_string())
    );

    let rules = detector.generate_direct_bypass_rules();
    assert_eq!(rules.len(), 8);
    let rule_strs: Vec<&str> = rules.iter().map(|r| r.rule.as_str()).collect();
    assert!(rule_strs.contains(&"IP-CIDR,198.51.100.0/24,DIRECT,no-resolve"));
    assert!(rule_strs.contains(&"IP-CIDR6,2001:db8:abcd::/48,DIRECT,no-resolve"));
}

#[test]
fn test_corporate_subnet_detector_string_helpers() {
    let detector = CorporateSubnetDetector::new();
    assert!(detector.is_corporate_or_private_str("192.168.1.1"));
    assert!(detector.is_corporate_or_private_str("10.50.0.1"));
    assert!(detector.is_corporate_or_private_str("fd00::1234"));
    assert!(!detector.is_corporate_or_private_str("8.8.8.8"));
    assert!(!detector.is_corporate_or_private_str("not-an-ip"));
}

// ============================================================================
// ProcessUsageTracker Tests
// ============================================================================

#[test]
fn test_process_usage_tracker_basic() {
    let tracker = ProcessUsageTracker::new();
    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);

    tracker.record_process_traffic("chrome.exe", 1000, 4000);
    assert_eq!(tracker.len(), 1);
    assert!(!tracker.is_empty());

    let snap = tracker.get_process("chrome").unwrap();
    assert_eq!(snap.process_name, "chrome");
    assert_eq!(snap.upload_bytes, 1000);
    assert_eq!(snap.download_bytes, 4000);
    assert_eq!(snap.total_bytes(), 5000);

    // Record additional traffic for another alias
    tracker.record_process_traffic("Google Chrome", 500, 1500);
    let snap2 = tracker.get_process("chrome").unwrap();
    assert_eq!(snap2.upload_bytes, 1500);
    assert_eq!(snap2.download_bytes, 5500);
    assert_eq!(snap2.total_bytes(), 7000);

    let (tot_up, tot_down) = tracker.total_traffic();
    assert_eq!(tot_up, 1500);
    assert_eq!(tot_down, 5500);
}

#[test]
fn test_process_usage_tracker_connections() {
    let tracker = ProcessUsageTracker::new();

    tracker.record_connection("firefox.exe");
    tracker.record_connection("firefox-bin");
    tracker.record_connection("firefox");

    let snap = tracker.get_process("firefox").unwrap();
    assert_eq!(snap.connection_count, 3);
    assert_eq!(snap.active_connections, 3);

    tracker.close_connection("firefox.exe");
    let snap_after = tracker.get_process("firefox").unwrap();
    assert_eq!(snap_after.connection_count, 3);
    assert_eq!(snap_after.active_connections, 2);

    tracker.close_connection("firefox");
    tracker.close_connection("firefox");
    tracker.close_connection("firefox"); // saturating sub to 0
    let snap_zero = tracker.get_process("firefox").unwrap();
    assert_eq!(snap_zero.active_connections, 0);
}

#[test]
fn test_process_usage_tracker_top_processes() {
    let tracker = ProcessUsageTracker::new();

    tracker.record_process_traffic("curl", 100, 200); // 300
    tracker.record_process_traffic("git", 1000, 2000); // 3000
    tracker.record_process_traffic("chrome.exe", 5000, 15000); // 20000
    tracker.record_process_traffic("code", 2000, 3000); // 5000

    let top2 = tracker.get_top_processes(2);
    assert_eq!(top2.len(), 2);
    assert_eq!(top2[0].process_name, "chrome");
    assert_eq!(top2[0].total_bytes, 20000);
    assert_eq!(top2[1].process_name, "code");
    assert_eq!(top2[1].total_bytes, 5000);

    let top10 = tracker.get_top_processes(10);
    assert_eq!(top10.len(), 4);
    assert_eq!(top10[0].process_name, "chrome");
    assert_eq!(top10[1].process_name, "code");
    assert_eq!(top10[2].process_name, "git");
    assert_eq!(top10[3].process_name, "curl");

    let top0 = tracker.get_top_processes(0);
    assert!(top0.is_empty());
}

#[test]
fn test_process_usage_tracker_clear_and_reset() {
    let tracker = ProcessUsageTracker::new();
    tracker.record_process_traffic("node", 100, 100);
    tracker.record_process_traffic("python", 200, 200);

    assert_eq!(tracker.len(), 2);
    assert!(tracker.reset_process("node"));
    assert_eq!(tracker.len(), 1);
    assert!(tracker.get_process("node").is_none());

    tracker.clear();
    assert!(tracker.is_empty());
}

#[test]
fn test_process_usage_tracker_concurrent_threads() {
    let tracker = Arc::new(ProcessUsageTracker::new());
    let mut handles = Vec::new();

    for i in 0..10 {
        let t = Arc::clone(&tracker);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                t.record_process_traffic("chrome.exe", 10, 20);
                if i % 2 == 0 {
                    t.record_connection("chrome.exe");
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let snap = tracker.get_process("chrome").unwrap();
    assert_eq!(snap.upload_bytes, 10 * 100 * 10);
    assert_eq!(snap.download_bytes, 20 * 100 * 10);
    assert_eq!(snap.connection_count, 5 * 100);
}

// ============================================================================
// ProcessAliasRegistry Tests
// ============================================================================

#[test]
fn test_process_alias_registry_builtins() {
    let registry = ProcessAliasRegistry::default();
    assert!(!registry.is_empty());

    // Browsers
    assert_eq!(registry.canonicalize("Google Chrome"), "chrome");
    assert_eq!(registry.canonicalize("chrome.exe"), "chrome");
    assert_eq!(registry.canonicalize("google-chrome-stable"), "chrome");
    assert_eq!(registry.canonicalize("google-chrome-beta"), "chrome");
    assert_eq!(registry.canonicalize("com.android.chrome"), "chrome");
    assert_eq!(registry.canonicalize("/usr/bin/google-chrome"), "chrome");
    assert_eq!(
        registry.canonicalize("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
        "chrome"
    );

    assert_eq!(registry.canonicalize("Firefox"), "firefox");
    assert_eq!(registry.canonicalize("firefox.exe"), "firefox");
    assert_eq!(registry.canonicalize("firefox-bin"), "firefox");
    assert_eq!(registry.canonicalize("Microsoft Edge"), "msedge");
    assert_eq!(registry.canonicalize("msedge.exe"), "msedge");
    assert_eq!(registry.canonicalize("microsoft-edge-stable"), "msedge");
    assert_eq!(registry.canonicalize("Brave Browser.app"), "brave");

    // IDEs & Dev
    assert_eq!(registry.canonicalize("Visual Studio Code"), "code");
    assert_eq!(registry.canonicalize("Code.exe"), "code");
    assert_eq!(registry.canonicalize("com.microsoft.VSCode"), "code");
    assert_eq!(registry.canonicalize("idea64.exe"), "intellij");
    assert_eq!(registry.canonicalize("pycharm64.exe"), "pycharm");
    assert_eq!(registry.canonicalize("git.exe"), "git");
    assert_eq!(registry.canonicalize("docker.exe"), "docker");

    // IM & Media
    assert_eq!(registry.canonicalize("Telegram.exe"), "telegram");
    assert_eq!(registry.canonicalize("telegram-desktop"), "telegram");
    assert_eq!(registry.canonicalize("Discord.exe"), "discord");
    assert_eq!(registry.canonicalize("Spotify.exe"), "spotify");
    assert_eq!(
        registry.canonicalize("com.google.android.youtube"),
        "youtube"
    );
    assert_eq!(registry.canonicalize("Netflix.exe"), "netflix");
}

#[test]
fn test_process_alias_registry_custom_aliases() {
    let mut registry = ProcessAliasRegistry::empty();
    assert!(registry.is_empty());

    registry.register_alias("MyCustomBrowser", "mybrowser");
    registry.register_aliases(["mb.exe", "mybrowser-bin"], "mybrowser");

    assert_eq!(registry.canonicalize("MyCustomBrowser"), "mybrowser");
    assert_eq!(registry.canonicalize("mb.exe"), "mybrowser");
    assert_eq!(registry.canonicalize("mybrowser-bin"), "mybrowser");
    assert_eq!(registry.get_canonical("mycustombrowser"), Some("mybrowser"));
}

#[test]
fn test_process_alias_registry_fallback_heuristics() {
    let registry = ProcessAliasRegistry::empty();

    // Strips path and .exe
    assert_eq!(
        registry.canonicalize("/opt/custom_tools/my_worker.exe"),
        "my_worker"
    );
    assert_eq!(
        registry.canonicalize("C:\\Users\\admin\\Downloads\\special_tool.app"),
        "special_tool"
    );
    assert_eq!(registry.canonicalize("   "), "");
}

#[test]
fn test_canonicalize_name_global_helper() {
    assert_eq!(
        ProcessAliasRegistry::canonicalize_name("chrome.exe"),
        "chrome"
    );
    assert_eq!(
        ProcessAliasRegistry::canonicalize_name("Google Chrome"),
        "chrome"
    );
}
