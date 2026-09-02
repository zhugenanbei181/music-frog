use super::*;
use std::time::Duration;

#[test]
fn test_connection_rate_tracker() {
    let start = Instant::now();
    let mut tracker = ConnectionRateTracker::new_with_time(start);

    tracker.add_up(1000);
    tracker.add_down(2000);

    let t1 = start + Duration::from_secs(1);
    let snap1 = tracker.snapshot_with_time(t1);

    assert_eq!(snap1.total_up, 1000);
    assert_eq!(snap1.total_down, 2000);
    assert_eq!(snap1.up_speed, 1000);
    assert_eq!(snap1.down_speed, 2000);
    assert_eq!(snap1.peak_up_speed, 1000);
    assert_eq!(snap1.peak_down_speed, 2000);

    tracker.add_up(500);
    tracker.add_down(4000);

    let t2 = t1 + Duration::from_secs_f64(0.5);
    let snap2 = tracker.snapshot_with_time(t2);

    assert_eq!(snap2.total_up, 1500);
    assert_eq!(snap2.total_down, 6000);
    assert_eq!(snap2.up_speed, 1000);
    assert_eq!(snap2.down_speed, 8000);
    assert_eq!(snap2.peak_up_speed, 1000);
    assert_eq!(snap2.peak_down_speed, 8000);
}

#[test]
fn test_jitter_calculator() {
    let mut calc = JitterCalculator::new();
    calc.record_success(100.0);
    calc.record_success(110.0);
    calc.record_success(105.0);
    calc.record_success(120.0);

    let stats = calc.calculate();

    assert_eq!(stats.sample_count, 4);
    assert_eq!(stats.loss_rate_percent, 0.0);
    assert_eq!(stats.mean_latency_ms, 108.75);
    assert_eq!(stats.jitter_ms, 10.0);
    assert!((stats.std_dev_ms - 8.53912).abs() < 0.001);
}

#[test]
fn test_jitter_calculator_with_loss() {
    let mut calc = JitterCalculator::new();
    calc.record_success(50.0);
    calc.record_failure();
    calc.record_success(60.0);
    calc.record_failure();
    calc.record_failure();

    let stats = calc.calculate();
    assert_eq!(stats.sample_count, 5);
    assert_eq!(stats.loss_rate_percent, 60.0);
    assert_eq!(stats.mean_latency_ms, 55.0);
    assert_eq!(stats.jitter_ms, 10.0);
}

#[test]
fn test_dns_metrics_tracker() {
    let mut tracker = DnsMetricsTracker::new(1000);
    assert_eq!(tracker.fake_ip_capacity(), 1000);

    tracker.record_query(true, 10.0);
    tracker.record_query(false, 50.0);
    tracker.record_query(true, 5.0);
    tracker.record_query(false, 35.0);

    assert_eq!(tracker.hit_ratio(), 0.5);
    assert_eq!(tracker.average_response_time_ms(), 25.0);
}

#[test]
fn test_connection_timing_breakdown() {
    let mut timing = ConnectionTimingBreakdown::new("conn-1", "www.example.com");
    timing.dns_lookup_ms = Some(15);
    timing.tcp_handshake_ms = Some(35);
    timing.tls_handshake_ms = Some(50);
    timing.ttfb_ms = Some(120);
    timing.total_duration_ms = 450;
    timing.chain = vec!["Proxy-A".to_string(), "DIRECT".to_string()];

    assert_eq!(timing.host, "www.example.com");
    assert_eq!(timing.dns_lookup_ms, Some(15));
    assert_eq!(timing.chain.len(), 2);
}

#[test]
fn test_speedtest_calculator() {
    let bytes = 10 * 1024 * 1024;
    let mbps = SpeedtestCalculator::calculate_bandwidth(bytes, 2000);
    assert!((mbps - 40.0).abs() < 0.1);

    assert_eq!(SpeedtestCalculator::calculate_bandwidth(0, 1000), 0.0);
    assert_eq!(SpeedtestCalculator::calculate_bandwidth(1000, 0), 0.0);
}

#[test]
fn test_network_throttling_profiles() {
    let p2g = NetworkThrottlingProfile::Profile2G;
    assert_eq!(p2g.max_down_kbps(), 250);
    assert_eq!(p2g.max_up_kbps(), 50);
    assert_eq!(p2g.delay_ms(), 300);
    assert_eq!(p2g.jitter_ms(), 50);
    assert_eq!(p2g.loss_percent(), 3.0);
    assert_eq!(p2g.name(), "2G");

    let p3g = NetworkThrottlingProfile::Profile3G;
    assert_eq!(p3g.max_down_kbps(), 1_600);
    assert_eq!(p3g.max_up_kbps(), 750);
    assert_eq!(p3g.delay_ms(), 100);
    assert_eq!(p3g.jitter_ms(), 20);
    assert_eq!(p3g.loss_percent(), 1.0);
    assert_eq!(p3g.name(), "3G");

    let p4g = NetworkThrottlingProfile::Profile4G;
    assert_eq!(p4g.max_down_kbps(), 10_000);
    assert_eq!(p4g.max_up_kbps(), 3_000);
    assert_eq!(p4g.delay_ms(), 20);
    assert_eq!(p4g.jitter_ms(), 5);
    assert_eq!(p4g.loss_percent(), 0.0);
    assert_eq!(p4g.name(), "4G");

    let pdsl = NetworkThrottlingProfile::ProfileDSL;
    assert_eq!(pdsl.max_down_kbps(), 2_000);
    assert_eq!(pdsl.max_up_kbps(), 512);
    assert_eq!(pdsl.delay_ms(), 5);
    assert_eq!(pdsl.jitter_ms(), 1);
    assert_eq!(pdsl.loss_percent(), 0.0);
    assert_eq!(pdsl.name(), "DSL");

    let psat = NetworkThrottlingProfile::ProfileSatellite;
    assert_eq!(psat.max_down_kbps(), 2_000);
    assert_eq!(psat.max_up_kbps(), 512);
    assert_eq!(psat.delay_ms(), 600);
    assert_eq!(psat.jitter_ms(), 40);
    assert_eq!(psat.loss_percent(), 2.0);
    assert_eq!(psat.name(), "Satellite");

    let custom = NetworkThrottlingProfile::Custom {
        max_down_kbps: 50_000,
        max_up_kbps: 10_000,
        delay_ms: 15,
        jitter_ms: 2,
        loss_percent: 0.5,
    };
    assert_eq!(custom.max_down_kbps(), 50_000);
    assert_eq!(custom.max_up_kbps(), 10_000);
    assert_eq!(custom.delay_ms(), 15);
    assert_eq!(custom.jitter_ms(), 2);
    assert_eq!(custom.loss_percent(), 0.5);
    assert_eq!(custom.name(), "Custom");
}

#[test]
fn test_throttling_calculator_delays_and_loss() {
    let calc = ThrottlingCalculator::new(NetworkThrottlingProfile::Profile3G);

    // Profile3G: delay 100ms, jitter 20ms
    assert_eq!(calc.compute_injected_delay(0.0), 100);
    assert_eq!(calc.compute_injected_delay(1.0), 120);
    assert_eq!(calc.compute_injected_delay(-1.0), 80);
    assert_eq!(calc.compute_injected_delay(0.5), 110);
    assert_eq!(calc.compute_injected_delay(-0.5), 90);

    // Static calculate_delay clamping
    assert_eq!(ThrottlingCalculator::calculate_delay(10, 5, 2.0), 15);
    assert_eq!(ThrottlingCalculator::calculate_delay(10, 5, -2.0), 5);
    assert_eq!(ThrottlingCalculator::calculate_delay(5, 10, -1.0), 0);

    // Transmission delay: 1000 bytes over 8000 kbps (1 MB/s) = 8000 bits / 8000 kbps = 1 ms
    let trans_delay = ThrottlingCalculator::calculate_transmission_delay_ms(1000, 8000);
    assert!((trans_delay - 1.0).abs() < 1e-6);
    assert_eq!(ThrottlingCalculator::calculate_transmission_delay_ms(1000, 0), 0.0);

    // Packet loss checking
    assert!(ThrottlingCalculator::should_drop_packet(5.0, 4.9));
    assert!(!ThrottlingCalculator::should_drop_packet(5.0, 5.0));
    assert!(!ThrottlingCalculator::should_drop_packet(0.0, 0.0));
    assert!(calc.is_packet_dropped(0.5));
    assert!(!calc.is_packet_dropped(1.5));
}

#[test]
fn test_token_bucket_rate_limiting() {
    let start = Instant::now();
    // 800 kbps = 100,000 bytes/sec, capacity 100,000 bytes
    let mut bucket = TokenBucket::new_with_time(800, 100_000, start);
    assert_eq!(bucket.capacity_bytes(), 100_000.0);
    assert_eq!(bucket.rate_bytes_per_sec(), 100_000.0);
    assert_eq!(bucket.available_tokens(), 100_000.0);

    // Consume 60,000 bytes immediately
    assert!(bucket.try_consume(60_000, start));
    assert_eq!(bucket.available_tokens(), 40_000.0);

    // Cannot consume 50,000 bytes right now
    assert!(!bucket.try_consume(50_000, start));

    // Wait 0.1s -> refilled 10,000 bytes -> total 50,000 bytes
    let t1 = start + Duration::from_millis(100);
    assert!(bucket.try_consume(50_000, t1));
    assert_eq!(bucket.available_tokens(), 0.0);

    // consume_or_wait: needs 20,000 bytes when 0 available -> wait 0.2s
    let wait = bucket.consume_or_wait(20_000, t1);
    assert_eq!(wait, Duration::from_millis(200));
}

#[test]
fn test_throttling_calculator_bucket_simulation() {
    let start = Instant::now();
    let profile = NetworkThrottlingProfile::Custom {
        max_down_kbps: 8_000, // 1,000,000 bytes/sec
        max_up_kbps: 4_000,   // 500,000 bytes/sec
        delay_ms: 10,
        jitter_ms: 2,
        loss_percent: 0.0,
    };
    let mut calc = ThrottlingCalculator::new_with_time(profile, start);

    assert!(calc.try_consume_downlink(50_000, start));
    assert!(calc.try_consume_uplink(25_000, start));

    let t1 = start + Duration::from_millis(50);
    let wait_down = calc.compute_downlink_wait(10_000, t1);
    assert_eq!(wait_down, Duration::ZERO);
}

#[test]
fn test_privacy_leak_dns_detection() {
    let conns = vec![
        // Direct unencrypted DNS query to public DNS 8.8.8.8 -> Leak!
        DiagnosticConnection::new("c-1", "8.8.8.8", 53, "DIRECT")
            .with_network("udp")
            .with_process_path("/usr/bin/curl"),
        // Local DNS query to 127.0.0.1 -> Not a leak
        DiagnosticConnection::new("c-2", "127.0.0.1", 53, "DIRECT"),
        // Proxied DNS query -> Not a leak
        DiagnosticConnection::new("c-3", "1.1.1.1", 53, "Proxy"),
    ];

    let dns_logs = vec![
        // Unencrypted direct resolution -> Leak!
        DnsResolutionLog::new("leaky.com", "A", "8.8.4.4:53")
            .with_direct(true)
            .with_encrypted(false)
            .with_process("firefox"),
        // Encrypted DoH direct resolution -> Safe
        DnsResolutionLog::new("safe.com", "A", "https://dns.google/dns-query")
            .with_direct(true)
            .with_encrypted(true),
    ];

    let outcome = PrivacyLeakDetectionSuite::evaluate(&conns, &dns_logs);
    assert!(outcome.dns_leak);
    assert!(!outcome.webrtc_leak);
    assert!(!outcome.ipv6_leak);
    assert!(!outcome.fake_ip_bypass);
    assert_eq!(outcome.details.len(), 2);
}

#[test]
fn test_privacy_leak_webrtc_detection() {
    let conns = vec![
        // Direct STUN port connection -> Leak!
        DiagnosticConnection::new("c-stun-1", "142.250.180.127", 3478, "DIRECT")
            .with_host("stun.l.google.com")
            .with_network("udp"),
        // Proxied STUN connection -> Safe
        DiagnosticConnection::new("c-stun-2", "142.250.180.127", 3478, "Proxy")
            .with_host("stun.l.google.com"),
    ];

    let dns_logs = vec![
        // Direct DNS resolution for STUN host -> Leak!
        DnsResolutionLog::new("stun.l.google.com", "A", "127.0.0.1:53")
            .with_direct(true)
            .with_encrypted(false),
    ];

    let outcome = PrivacyLeakDetectionSuite::evaluate(&conns, &dns_logs);
    assert!(outcome.webrtc_leak);
    assert!(outcome.has_any_leak());
}

#[test]
fn test_privacy_leak_ipv6_detection() {
    let conns = vec![
        // Direct public IPv6 connection -> Leak!
        DiagnosticConnection::new("c-ipv6-1", "2606:4700:4700::1111", 443, "DIRECT"),
        // Direct link-local / loopback IPv6 -> Safe
        DiagnosticConnection::new("c-ipv6-2", "::1", 8080, "DIRECT"),
        DiagnosticConnection::new("c-ipv6-3", "fe80::1", 80, "DIRECT"),
        DiagnosticConnection::new("c-ipv6-4", "fc00::1", 80, "DIRECT"),
        // Proxied public IPv6 -> Safe
        DiagnosticConnection::new("c-ipv6-5", "2001:4860:4860::8888", 443, "Proxy"),
    ];

    let dns_logs = vec![
        // Direct AAAA query resolution -> Leak!
        DnsResolutionLog::new("ipv6.example.com", "AAAA", "1.1.1.1:53")
            .with_direct(true)
            .with_resolved_ips(vec!["2606:2800:220:1:248:1893:25c8:1946".to_string()]),
    ];

    let outcome = PrivacyLeakDetectionSuite::evaluate(&conns, &dns_logs);
    assert!(outcome.ipv6_leak);
}

#[test]
fn test_privacy_leak_fake_ip_bypass() {
    let conns = vec![
        // Direct route to Fake-IP (198.18.0.0/15) -> Leak!
        DiagnosticConnection::new("c-fake-1", "198.18.0.25", 443, "DIRECT")
            .with_process_path("browser"),
        // Proxied route to Fake-IP -> Safe
        DiagnosticConnection::new("c-fake-2", "198.18.0.26", 443, "Proxy"),
    ];

    let dns_logs = vec![
        // Direct DNS resolution returning Fake-IP -> Leak!
        DnsResolutionLog::new("internal.pool", "A", "127.0.0.1:53")
            .with_direct(true)
            .with_resolved_ips(vec!["198.19.1.5".to_string()]),
    ];

    let outcome = PrivacyLeakDetectionSuite::evaluate(&conns, &dns_logs);
    assert!(outcome.fake_ip_bypass);
    assert_eq!(outcome.total_leaks_count(), 1);
}

#[test]
fn test_privacy_leak_mihomo_conversion() {
    let mut mihomo_conn = mihomo_api::types::Connection {
        id: "mihomo-c1".to_string(),
        metadata: mihomo_api::types::ConnectionMetadata {
            network: "udp".to_string(),
            connection_type: "TUN".to_string(),
            source_ip: "198.18.0.1".to_string(),
            destination_ip: "8.8.8.8".to_string(),
            source_port: "54321".to_string(),
            destination_port: "53".to_string(),
            host: "".to_string(),
            dns_mode: "fake-ip".to_string(),
            process_path: "/usr/bin/nslookup".to_string(),
            special_proxy: "".to_string(),
        },
        upload: 100,
        download: 200,
        start: "2026-09-01T00:00:00Z".to_string(),
        rule: "DIRECT".to_string(),
        rule_payload: "".to_string(),
        chains: vec!["DIRECT".to_string()],
    };

    let outcome = PrivacyLeakDetectionSuite::evaluate_mihomo_connections(&[mihomo_conn.clone()], &[]);
    assert!(outcome.dns_leak);

    // When proxied, no leak
    mihomo_conn.rule = "ProxyGroup".to_string();
    mihomo_conn.chains = vec!["ProxyGroup".to_string(), "US-Node".to_string()];
    let clean_outcome = PrivacyLeakDetectionSuite::evaluate_mihomo_connections(&[mihomo_conn], &[]);
    assert!(clean_outcome.is_clean());
}
