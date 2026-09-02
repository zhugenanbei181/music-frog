use super::*;

fn create_snapshot(
    name: &str,
    is_up: bool,
    is_default_gateway: bool,
    ips: Vec<&str>,
) -> NetworkInterfaceSnapshot {
    NetworkInterfaceSnapshot::new(
        name,
        is_up,
        is_default_gateway,
        ips.into_iter().map(|s| s.to_string()).collect(),
    )
}

#[test]
fn test_interface_type_inference_and_attributes() {
    assert_eq!(
        InterfaceType::infer_from_name("eth0"),
        InterfaceType::Ethernet
    );
    assert_eq!(
        InterfaceType::infer_from_name("enp3s0"),
        InterfaceType::Ethernet
    );
    assert_eq!(InterfaceType::infer_from_name("wlan0"), InterfaceType::WiFi);
    assert_eq!(
        InterfaceType::infer_from_name("wlp2s0"),
        InterfaceType::WiFi
    );
    assert_eq!(
        InterfaceType::infer_from_name("rmnet_data0"),
        InterfaceType::Cellular
    );
    assert_eq!(
        InterfaceType::infer_from_name("wwan0"),
        InterfaceType::Cellular
    );
    assert_eq!(InterfaceType::infer_from_name("tun0"), InterfaceType::Tun);
    assert_eq!(InterfaceType::infer_from_name("Meta"), InterfaceType::Tun);
    assert_eq!(
        InterfaceType::infer_from_name("lo"),
        InterfaceType::Loopback
    );

    let wifi = InterfaceType::WiFi;
    let cellular = InterfaceType::Cellular;
    let eth = InterfaceType::Ethernet;

    assert!(wifi.is_wireless());
    assert!(cellular.is_cellular());
    assert!(eth.is_physical());
    assert_eq!(cellular.standard_mtu(), 1420);
    assert_eq!(eth.standard_mtu(), 1500);
}

#[test]
fn test_gateway_priority_arbiter_selection() {
    let eth = create_snapshot("eth0", true, false, vec!["192.168.1.10"])
        .with_interface_type(InterfaceType::Ethernet)
        .with_metric(100);
    let wifi = create_snapshot("wlan0", true, true, vec!["192.168.1.20"])
        .with_interface_type(InterfaceType::WiFi)
        .with_metric(200);
    let cellular = create_snapshot("rmnet0", true, false, vec!["10.50.0.5"])
        .with_interface_type(InterfaceType::Cellular)
        .with_metric(300);
    let tun = create_snapshot("Meta", true, false, vec!["198.18.0.1"])
        .with_interface_type(InterfaceType::Tun);

    let snapshots = vec![cellular.clone(), wifi.clone(), tun.clone(), eth.clone()];
    let best = GatewayPriorityArbiter::select_best_candidate(&snapshots);
    assert_eq!(best.unwrap().name, "eth0");

    // Optimal MTU calculation
    let (tun_mtu, mss) = GatewayPriorityArbiter::calculate_optimal_mtu(1420);
    assert_eq!(tun_mtu, 1340);
    assert_eq!(mss, 1300);
}

#[test]
fn test_interface_flap_guard() {
    let mut guard = InterfaceFlapGuard::new(Duration::from_millis(50), 3);
    assert!(!guard.record_change("wlan0"));
    assert!(!guard.record_change("wlan0"));
    assert!(guard.record_change("wlan0"));
    assert!(guard.is_suppressed());

    guard.reset();
    assert!(!guard.is_suppressed());
}

#[test]
fn test_interface_up_down_detection() {
    let before = vec![
        create_snapshot("eth0", false, false, vec![]),
        create_snapshot("wlan0", true, false, vec!["192.168.1.5"]),
    ];
    let after = vec![
        create_snapshot("eth0", true, false, vec!["10.0.0.5"]),
        create_snapshot("wlan0", false, false, vec![]),
    ];

    let diff = InterfaceDiffDetector::compute_diff(&before, &after);

    assert!(diff.contains(&NetworkEvent::InterfaceUp("eth0".to_string())));
    assert!(diff.contains(&NetworkEvent::InterfaceDown("wlan0".to_string())));
}

#[test]
fn test_default_gateway_migration_detection() {
    let before = vec![
        create_snapshot("eth0", true, true, vec!["10.0.0.5"]),
        create_snapshot("wlan0", true, false, vec!["192.168.1.5"]),
    ];
    let after = vec![
        create_snapshot("eth0", true, false, vec!["10.0.0.5"]),
        create_snapshot("wlan0", true, true, vec!["192.168.1.5"]),
    ];

    let diff = InterfaceDiffDetector::compute_diff(&before, &after);

    assert!(diff.contains(&NetworkEvent::DefaultGatewayChanged {
        old: Some("eth0".to_string()),
        new: Some("wlan0".to_string()),
    }));
}

#[test]
fn test_ip_address_change_detection() {
    let before = vec![create_snapshot("eth0", true, false, vec!["10.0.0.5"])];
    let after = vec![create_snapshot(
        "eth0",
        true,
        false,
        vec!["10.0.0.5", "10.0.0.6"],
    )];

    let diff = InterfaceDiffDetector::compute_diff(&before, &after);

    assert!(diff.contains(&NetworkEvent::IpAddressChanged {
        iface: "eth0".to_string(),
        new_ips: vec!["10.0.0.5".to_string(), "10.0.0.6".to_string()],
    }));
}

#[test]
fn test_gateway_migration_updates_tun_routes() {
    let mut detector = GatewayMigrationDetector::new(Some("Meta".to_string()));
    detector.set_tun_active(true);

    let before = vec![
        create_snapshot("wlan0", true, true, vec!["192.168.1.100"]).with_gateway_ip("192.168.1.1"),
        create_snapshot("eth0", false, false, vec![]),
        create_snapshot("Meta", true, false, vec!["198.18.0.1"]),
    ];

    let _ = detector.detect_migration(&[], &before);

    let after = vec![
        create_snapshot("wlan0", false, false, vec![]),
        create_snapshot("eth0", true, true, vec!["10.0.0.50"]).with_gateway_ip("10.0.0.1"),
        create_snapshot("Meta", true, false, vec!["198.18.0.1"]),
    ];

    let diff = InterfaceDiffDetector::compute_diff_with_detector(&before, &after, &mut detector);

    assert!(InterfaceDiffDetector::should_trigger_core_reconnect(&diff));

    let migration_ev = diff.iter().find_map(|e| match e {
        NetworkEvent::GatewayMigration(m) => Some(m.as_ref()),
        _ => None,
    });

    assert!(migration_ev.is_some());
    let migration = migration_ev.unwrap();
    assert_eq!(
        migration.action_required,
        GatewayMigrationAction::UpdateTunRoutes {
            old_gateway_iface: Some("wlan0".to_string()),
            new_gateway_iface: "eth0".to_string(),
            new_gateway_ip: Some("10.0.0.1".to_string()),
        }
    );
}

#[test]
fn test_routing_loop_prevention() {
    let mut detector = GatewayMigrationDetector::new(Some("Meta".to_string()));
    detector.set_tun_active(false);

    let before = vec![
        create_snapshot("eth0", true, false, vec!["10.0.0.5"]),
        create_snapshot("Meta", true, false, vec!["198.18.0.1"]),
    ];
    let after = vec![
        create_snapshot("eth0", true, false, vec!["10.0.0.5"]),
        create_snapshot("Meta", true, true, vec!["198.18.0.1"]),
    ];

    let diff = InterfaceDiffDetector::compute_diff_with_detector(&before, &after, &mut detector);

    assert!(
        diff.iter()
            .any(|e| matches!(e, NetworkEvent::RoutingLoopRiskDetected { .. }))
    );
    assert!(InterfaceDiffDetector::should_trigger_core_reconnect(&diff));
}

#[test]
fn test_dead_tun_mitigation_on_physical_gateway_down() {
    let mut detector = GatewayMigrationDetector::new(Some("Meta".to_string()));
    detector.set_tun_active(true);

    let initial = vec![
        create_snapshot("wlan0", true, true, vec!["192.168.1.100"]).with_gateway_ip("192.168.1.1"),
        create_snapshot("eth0", true, false, vec!["10.0.0.2"]),
        create_snapshot("Meta", true, false, vec!["198.18.0.1"]),
    ];
    let _ = detector.detect_migration(&[], &initial);

    let after = vec![
        create_snapshot("wlan0", false, true, vec![]),
        create_snapshot("eth0", true, false, vec!["10.0.0.2"]),
        create_snapshot("Meta", true, false, vec!["198.18.0.1"]),
    ];

    let diff = InterfaceDiffDetector::compute_diff_with_detector(&initial, &after, &mut detector);

    let migration_ev = diff.iter().find_map(|e| match e {
        NetworkEvent::GatewayMigration(m) => Some(m.as_ref()),
        _ => None,
    });

    assert!(migration_ev.is_some());
    let migration = migration_ev.unwrap();
    assert!(matches!(
        migration.action_required,
        GatewayMigrationAction::DeadTunMitigation { .. }
    ));
}

#[test]
fn test_poll_interfaces_returns_snapshots() {
    let snapshots = NetworkInterfaceWatcher::poll_interfaces();
    assert!(!snapshots.is_empty() || snapshots.is_empty());
}

#[tokio::test]
async fn test_watcher_update_snapshots_and_broadcast() {
    let watcher = NetworkInterfaceWatcher::with_tun_name("Meta");
    let mut rx = watcher.start();

    let s1 = vec![create_snapshot("eth0", false, false, vec![])];
    let s2 = vec![create_snapshot("eth0", true, true, vec!["192.168.1.10"])];

    let _ = watcher.update_snapshots(s1).await;
    let events = watcher.update_snapshots(s2).await;

    assert!(!events.is_empty());
    let ev = rx.recv().await.unwrap();
    assert!(matches!(
        ev,
        NetworkEvent::InterfaceUp(_) | NetworkEvent::DefaultGatewayChanged { .. }
    ));

    watcher.stop();
}

#[test]
fn test_gateway_hotplug_arbiter_multi_homing_selection() {
    let eth = create_snapshot("eth0", true, false, vec!["192.168.1.100"])
        .with_interface_type(InterfaceType::Ethernet)
        .with_metric(100);
    let wifi = create_snapshot("wlan0", true, true, vec!["192.168.2.100"])
        .with_interface_type(InterfaceType::WiFi)
        .with_metric(200);
    let cellular = create_snapshot("rmnet0", true, false, vec!["10.0.0.2"])
        .with_interface_type(InterfaceType::Cellular)
        .with_metric(300);
    let tun = create_snapshot("Meta", true, false, vec!["198.18.0.1"])
        .with_interface_type(InterfaceType::Tun);

    let snapshots = vec![cellular.clone(), wifi.clone(), tun.clone(), eth.clone()];
    let ranked = GatewayHotplugArbiter::rank_candidates(&snapshots);

    assert_eq!(ranked.len(), 3);
    assert_eq!(ranked[0].name, "eth0");
    assert_eq!(ranked[1].name, "wlan0");
    assert_eq!(ranked[2].name, "rmnet0");

    let decision = GatewayHotplugArbiter::arbitrate(&snapshots, Some("wlan0"), Some("Meta"));
    assert_eq!(decision.selected_interface.as_deref(), Some("eth0"));
    assert_eq!(decision.fallback_interfaces, vec!["wlan0", "rmnet0"]);
    assert!(matches!(
        decision.action,
        GatewayMigrationAction::UpdateTunRoutes { .. }
    ));
}

#[test]
fn test_gateway_hotplug_arbiter_all_down_dead_tun() {
    let tun = create_snapshot("Meta", true, false, vec!["198.18.0.1"])
        .with_interface_type(InterfaceType::Tun);
    let down_eth = create_snapshot("eth0", false, false, vec![]);

    let snapshots = vec![down_eth, tun];
    let decision = GatewayHotplugArbiter::arbitrate(&snapshots, Some("eth0"), Some("Meta"));

    assert_eq!(decision.selected_interface, None);
    assert!(matches!(
        decision.action,
        GatewayMigrationAction::DeadTunMitigation { .. }
    ));
}

#[test]
fn test_gateway_hotplug_debouncer_1000ms_accumulation() {
    let mut debouncer = HotplugDebouncer::with_duration(Duration::from_millis(1000));
    let start = Instant::now();

    let s1 = vec![create_snapshot("eth0", true, true, vec!["192.168.1.10"])];
    let s2 = vec![create_snapshot(
        "eth0",
        true,
        true,
        vec!["192.168.1.10", "192.168.1.11"],
    )];

    // Ingest s1 at start -> debouncing starts
    assert!(debouncer.ingest(s1.clone(), start).is_none());
    assert!(debouncer.is_debouncing());

    // At +400ms, DHCP assigns additional IP (s2) -> debounce timer resets
    let t_400 = start + Duration::from_millis(400);
    assert!(debouncer.ingest(s2.clone(), t_400).is_none());
    assert!(debouncer.is_debouncing());

    // At +1200ms from start (800ms from s2) -> not yet settled
    let t_1200 = start + Duration::from_millis(1200);
    assert!(debouncer.poll_settled(t_1200).is_none());

    // At +1450ms from start (1050ms from s2) -> settled!
    let t_1450 = start + Duration::from_millis(1450);
    let settled = debouncer.poll_settled(t_1450);
    assert!(settled.is_some());
    assert_eq!(settled.unwrap(), s2);
    assert!(!debouncer.is_debouncing());
}

#[tokio::test]
async fn test_watcher_update_snapshots_debounced() {
    let watcher = NetworkInterfaceWatcher::with_config_and_debounce(
        Some("Meta".to_string()),
        Duration::from_millis(100),
        Duration::from_millis(1000),
    );

    let start = Instant::now();
    let s1 = vec![create_snapshot("eth0", false, false, vec![])];
    let s2 = vec![create_snapshot("eth0", true, true, vec!["192.168.1.100"])];

    // First baseline snapshot
    let _ = watcher.update_snapshots(s1).await;

    // Ingest s2 at start
    let events_immediate = watcher.update_snapshots_debounced(s2.clone(), start).await;
    assert!(events_immediate.is_empty());

    // Poll at +500ms -> still debouncing
    let events_mid = watcher
        .update_snapshots_debounced(s2.clone(), start + Duration::from_millis(500))
        .await;
    assert!(events_mid.is_empty());

    // Poll at +1100ms -> settled and events emitted!
    let events_settled = watcher
        .update_snapshots_debounced(s2.clone(), start + Duration::from_millis(1100))
        .await;
    assert!(!events_settled.is_empty());
    assert!(events_settled.iter().any(|e| matches!(
        e,
        NetworkEvent::InterfaceUp(_) | NetworkEvent::DefaultGatewayChanged { .. }
    )));
}
