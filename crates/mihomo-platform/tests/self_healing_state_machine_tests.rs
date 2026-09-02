use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

use mihomo_platform::crash_reporter::{
    DnsCrashWatchdog, DnsStateSentinel, StandaloneDnsWatchdog,
};
use mihomo_platform::interface_watcher::{
    GatewayHotplugArbiter, GatewayMigrationAction,
    HotplugDebouncer, InterfaceType, NetworkInterfaceSnapshot,
};
use mihomo_platform::power::{
    SelfHealingPipeline, SelfHealingTier,
};

fn create_test_iface(
    name: &str,
    is_up: bool,
    is_default_gw: bool,
    ips: Vec<&str>,
    iface_type: InterfaceType,
    metric: u32,
) -> NetworkInterfaceSnapshot {
    NetworkInterfaceSnapshot::new(
        name,
        is_up,
        is_default_gw,
        ips.into_iter().map(|s| s.to_string()).collect(),
    )
    .with_interface_type(iface_type)
    .with_metric(metric)
}

// -----------------------------------------------------------------------------
// 1. DNS State Sentinel & Standalone Watchdog Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_dns_sentinel_e2e_crash_recovery_state_machine() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home = temp_dir.path();

    // Step 1: Daemon starts up and writes active sentinel with PID 99999991
    let dead_daemon_pid = 99999991;
    let original_dns = vec!["1.1.1.1".to_string(), "1.0.0.1".to_string()];
    let intercepted_dns = vec!["127.0.0.1".to_string()];
    let original_proxy = Some("127.0.0.1:7890".to_string());

    let sentinel = DnsStateSentinel::new(
        dead_daemon_pid,
        original_dns.clone(),
        intercepted_dns.clone(),
        original_proxy.clone(),
    )
    .with_interface("eth0")
    .with_heartbeat_timeout(10)
    .with_current_proxy(Some("127.0.0.1:7890".to_string()));

    let sentinel_path = DnsCrashWatchdog::write_sentinel(home, &sentinel).unwrap();
    assert!(sentinel_path.exists());

    // Step 2: Standalone Watchdog initializes
    let watchdog = StandaloneDnsWatchdog::new(home.to_path_buf())
        .with_poll_interval(Duration::from_millis(50))
        .with_heartbeat_timeout(5);

    let dns_restored = Arc::new(AtomicU32::new(0));
    let proxy_restored = Arc::new(AtomicU32::new(0));

    let d_c = dns_restored.clone();
    let p_c = proxy_restored.clone();

    // Step 3: Run one recovery pass
    let result = watchdog.check_and_recover(
        move |dns| {
            assert_eq!(dns, &["1.1.1.1", "1.0.0.1"]);
            d_c.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
        move |proxy| {
            assert_eq!(proxy, Some("127.0.0.1:7890"));
            p_c.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    );

    assert!(result.is_some());
    let recovery = result.unwrap();
    assert_eq!(recovery.sentinel.daemon_pid, dead_daemon_pid);
    assert_eq!(recovery.sentinel.interface_name.as_deref(), Some("eth0"));
    assert_eq!(dns_restored.load(Ordering::SeqCst), 1);
    assert_eq!(proxy_restored.load(Ordering::SeqCst), 1);

    // Sentinel file should be cleanly unlinked after recovery
    assert!(DnsCrashWatchdog::read_sentinel(home).unwrap().is_none());

    // Second recovery pass should find nothing
    let second_pass = watchdog.check_and_recover(|_| Ok(()), |_| Ok(()));
    assert!(second_pass.is_none());
}

// -----------------------------------------------------------------------------
// 2. Power Resume 5-Tier Self-Healing Pipeline Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_five_tier_self_healing_pipeline_wake_normal_flow() {
    let rst_purges = Arc::new(AtomicU32::new(0));
    let fake_ip_probes = Arc::new(AtomicU32::new(0));
    let node_retests = Arc::new(AtomicU32::new(0));
    let config_reloads = Arc::new(AtomicU32::new(0));
    let process_respawns = Arc::new(AtomicU32::new(0));

    let r_c = rst_purges.clone();
    let f_c = fake_ip_probes.clone();
    let n_c = node_retests.clone();
    let c_c = config_reloads.clone();
    let p_c = process_respawns.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_zombie_purge(Arc::new(move || {
            r_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }))
        .with_fake_ip_probe(Arc::new(move || {
            f_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(true) })
        }))
        .with_node_retest(Arc::new(move || {
            n_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(8) }) // 8 responsive nodes
        }))
        .with_config_reload(Arc::new(move || {
            c_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }))
        .with_process_respawn(Arc::new(move || {
            p_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("System wake from sleep").await;

    assert!(report.success);
    assert!(!report.safe_mode_tripped);
    assert_eq!(report.highest_tier_reached, SelfHealingTier::Tier3NodeDelayRetest);
    assert_eq!(rst_purges.load(Ordering::SeqCst), 1);
    assert_eq!(fake_ip_probes.load(Ordering::SeqCst), 1);
    assert_eq!(node_retests.load(Ordering::SeqCst), 1);
    assert_eq!(config_reloads.load(Ordering::SeqCst), 0);
    assert_eq!(process_respawns.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_five_tier_self_healing_pipeline_dns_corruption_escalation() {
    let fake_ip_probes = Arc::new(AtomicU32::new(0));
    let config_reloads = Arc::new(AtomicU32::new(0));

    let f_c = fake_ip_probes.clone();
    let c_c = config_reloads.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_fake_ip_probe(Arc::new(move || {
            let attempt = f_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt == 0 {
                    // First probe after wake fails (DNS corrupted during suspend)
                    Ok(false)
                } else {
                    // Post-reload probe succeeds
                    Ok(true)
                }
            })
        }))
        .with_config_reload(Arc::new(move || {
            c_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("Wake with corrupted Fake-IP pool").await;

    assert!(report.success);
    assert_eq!(report.highest_tier_reached, SelfHealingTier::Tier4ConfigReload);
    assert_eq!(fake_ip_probes.load(Ordering::SeqCst), 2);
    assert_eq!(config_reloads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_five_tier_self_healing_pipeline_core_deadlock_respawn() {
    let config_reloads = Arc::new(AtomicU32::new(0));
    let process_respawns = Arc::new(AtomicU32::new(0));

    let c_c = config_reloads.clone();
    let p_c = process_respawns.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_fake_ip_probe(Arc::new(|| Box::pin(async { Ok(false) })))
        .with_config_reload(Arc::new(move || {
            c_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err("Controller HTTP port not responding".to_string()) })
        }))
        .with_process_respawn(Arc::new(move || {
            p_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("Core deadlock on resume").await;

    assert!(report.success);
    assert_eq!(report.highest_tier_reached, SelfHealingTier::Tier5ProcessRespawnAndSafeMode);
    assert_eq!(config_reloads.load(Ordering::SeqCst), 1);
    assert_eq!(process_respawns.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_five_tier_self_healing_safe_mode_escalation() {
    let pipeline = SelfHealingPipeline::new()
        .with_safe_mode_threshold(3)
        .with_fake_ip_probe(Arc::new(|| Box::pin(async { Ok(false) })))
        .with_config_reload(Arc::new(|| Box::pin(async { Err("API dead".to_string()) })))
        .with_process_respawn(Arc::new(|| Box::pin(async { Ok(()) })));

    let r1 = pipeline.execute("Wake pass 1").await;
    assert!(r1.success);
    assert!(!r1.safe_mode_tripped);

    let r2 = pipeline.execute("Wake pass 2").await;
    assert!(r2.success);
    assert!(!r2.safe_mode_tripped);

    let r3 = pipeline.execute("Wake pass 3").await;
    assert!(!r3.success);
    assert!(r3.safe_mode_tripped);
}

// -----------------------------------------------------------------------------
// 3. Gateway Hot-Plug Arbiter & 1000ms Debouncer Tests
// -----------------------------------------------------------------------------

#[test]
fn test_gateway_hotplug_arbiter_failover_hierarchy() {
    let eth = create_test_iface("eth0", true, false, vec!["192.168.1.100"], InterfaceType::Ethernet, 100);
    let wifi = create_test_iface("wlan0", true, true, vec!["192.168.2.100"], InterfaceType::WiFi, 200);
    let cellular = create_test_iface("rmnet0", true, false, vec!["10.0.0.5"], InterfaceType::Cellular, 300);
    let tun = create_test_iface("Meta", true, false, vec!["198.18.0.1"], InterfaceType::Tun, 500);

    // Case 1: All available -> Ethernet wins over Wi-Fi and Cellular
    let all = vec![cellular.clone(), wifi.clone(), tun.clone(), eth.clone()];
    let d1 = GatewayHotplugArbiter::arbitrate(&all, Some("wlan0"), Some("Meta"));
    assert_eq!(d1.selected_interface.as_deref(), Some("eth0"));
    assert_eq!(d1.fallback_interfaces, vec!["wlan0", "rmnet0"]);
    assert!(matches!(d1.action, GatewayMigrationAction::UpdateTunRoutes { .. }));

    // Case 2: Ethernet goes down -> Failover to Wi-Fi
    let eth_down = create_test_iface("eth0", false, false, vec![], InterfaceType::Ethernet, 100);
    let without_eth = vec![cellular.clone(), wifi.clone(), tun.clone(), eth_down];
    let d2 = GatewayHotplugArbiter::arbitrate(&without_eth, Some("eth0"), Some("Meta"));
    assert_eq!(d2.selected_interface.as_deref(), Some("wlan0"));
    assert_eq!(d2.fallback_interfaces, vec!["rmnet0"]);

    // Case 3: Wi-Fi also drops -> Failover to Cellular
    let wifi_down = create_test_iface("wlan0", false, false, vec![], InterfaceType::WiFi, 200);
    let cellular_only = vec![cellular.clone(), wifi_down, tun.clone()];
    let d3 = GatewayHotplugArbiter::arbitrate(&cellular_only, Some("wlan0"), Some("Meta"));
    assert_eq!(d3.selected_interface.as_deref(), Some("rmnet0"));
    assert!(d3.fallback_interfaces.is_empty());

    // Case 4: Optimal MTU calculation on cellular
    let (tun_mtu, mss) = GatewayHotplugArbiter::calculate_optimal_mtu(InterfaceType::Cellular.standard_mtu());
    assert_eq!(tun_mtu, 1340); // 1420 - 80
    assert_eq!(mss, 1300);     // 1340 - 40
}

#[test]
fn test_gateway_hotplug_debouncer_1000ms_stabilization() {
    let mut debouncer = HotplugDebouncer::with_duration(Duration::from_millis(1000));
    let t0 = Instant::now();

    let s0 = vec![create_test_iface("eth0", false, false, vec![], InterfaceType::Ethernet, 100)];
    let s1 = vec![create_test_iface("eth0", true, false, vec!["169.254.1.1"], InterfaceType::Ethernet, 100)];
    let s2 = vec![create_test_iface("eth0", true, true, vec!["192.168.1.50"], InterfaceType::Ethernet, 100)];

    // Initialize baseline s0
    let _ = debouncer.ingest(s0.clone(), t0);

    // Link UP with link-local IP at +100ms
    let t1 = t0 + Duration::from_millis(100);
    assert!(debouncer.ingest(s1.clone(), t1).is_none());
    assert!(debouncer.is_debouncing());

    // DHCP finishes at +500ms -> changes to s2 -> timer resets to +500ms
    let t2 = t0 + Duration::from_millis(500);
    assert!(debouncer.ingest(s2.clone(), t2).is_none());
    assert!(debouncer.is_debouncing());

    // Check at +1300ms (800ms after s2) -> not settled yet
    let t3 = t0 + Duration::from_millis(1300);
    assert!(debouncer.poll_settled(t3).is_none());

    // Check at +1550ms (1050ms after s2) -> settled!
    let t4 = t0 + Duration::from_millis(1550);
    let settled = debouncer.poll_settled(t4);
    assert!(settled.is_some());
    assert_eq!(settled.unwrap(), s2);
    assert!(!debouncer.is_debouncing());
}
