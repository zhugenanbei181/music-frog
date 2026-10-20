//! Unit coverage for the auto-heal domain model.

use super::*;

#[test]
fn test_circuit_breaker_trips_to_open_on_threshold_failures() {
    let mut breaker = NodeCircuitBreaker::new("HK-01");
    assert!(breaker.is_healthy());

    breaker.record_failure(Duration::from_secs(1));
    assert_eq!(breaker.state, CircuitState::Closed);
    assert_eq!(breaker.failure_count, 1);

    breaker.record_failure(Duration::from_secs(2));
    assert_eq!(breaker.failure_count, 2);

    // Third failure trips to Open
    breaker.record_failure(Duration::from_secs(3));
    assert_eq!(breaker.state, CircuitState::Open);
    assert!(!breaker.is_healthy());

    // Fast fail inside cooldown window
    assert!(!breaker.can_attempt(Duration::from_secs(10)));
}

#[test]
fn test_circuit_breaker_cooldown_to_half_open_and_recovery() {
    let mut breaker = NodeCircuitBreaker::new("US-01");
    for i in 1..=3 {
        breaker.record_failure(Duration::from_secs(i));
    }
    assert_eq!(breaker.state, CircuitState::Open);

    // After cooldown (30s from tripped_at=3 -> 33s)
    assert!(breaker.can_attempt(Duration::from_secs(35)));
    assert_eq!(breaker.state, CircuitState::HalfOpen);

    // First canary success
    breaker.record_success();
    assert_eq!(breaker.state, CircuitState::HalfOpen);

    // Second canary success -> fully recovered
    breaker.record_success();
    assert_eq!(breaker.state, CircuitState::Closed);
    assert!(breaker.is_healthy());
}

#[test]
fn test_circuit_breaker_half_open_failure_reopens() {
    let mut breaker = NodeCircuitBreaker::new("JP-01");
    for i in 1..=3 {
        breaker.record_failure(Duration::from_secs(i));
    }

    // Advance past cooldown to HalfOpen
    assert!(breaker.can_attempt(Duration::from_secs(40)));
    assert_eq!(breaker.state, CircuitState::HalfOpen);

    // Failure during canary immediately re-opens breaker
    breaker.record_failure(Duration::from_secs(41));
    assert_eq!(breaker.state, CircuitState::Open);
    assert_eq!(breaker.tripped_at, Duration::from_secs(41));
}
#[test]
fn test_multi_node_circuit_registry_failover() {
    let mut reg = MultiNodeCircuitRegistry::new();
    let candidates = ["Node-A", "Node-B", "Node-C"];

    // Initially Node-A is selected
    let first = reg.select_first_healthy(&candidates, Duration::ZERO);
    assert_eq!(first, Some("Node-A"));

    // Trip Node-A with 3 failures
    for i in 1..=3 {
        reg.record_node_failure("Node-A", Duration::from_secs(i));
    }
    assert_eq!(reg.tripped_count(), 1);

    // Failover automatically picks Node-B
    let failover = reg.select_first_healthy(&candidates, Duration::from_secs(5));
    assert_eq!(failover, Some("Node-B"));
}
#[test]
fn test_exponential_backoff_delays_and_limit() {
    let policy = ExponentialBackoffPolicy::default();
    assert!(policy.should_retry(0));
    assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(500));
    assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(1000));
    assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(2000));

    // Clamped at max_delay (15s)
    assert_eq!(policy.delay_for_attempt(10), Duration::from_secs(15));

    // Terminate at max_retries (5)
    assert!(!policy.should_retry(5));
}
#[test]
fn test_auto_rollback_transaction_lifecycle() {
    let mut tx = AutoRollbackTransaction::new(
        "routing_intent=follow_rules".to_string(),
        Duration::from_secs(5),
    );
    assert_eq!(tx.current_state, "routing_intent=follow_rules");

    tx.apply_mutation("routing_intent=route_all_through_proxy".to_string());
    assert_eq!(tx.current_state, "routing_intent=route_all_through_proxy");

    // Tick 3s -> not timed out
    assert!(!tx.tick(Duration::from_secs(3)));
    assert_eq!(tx.current_state, "routing_intent=route_all_through_proxy");

    // Tick 3s more (total 6s >= 5s) -> auto rollback triggered
    assert!(tx.tick(Duration::from_secs(3)));
    assert!(tx.is_rolled_back);
    assert_eq!(tx.current_state, "routing_intent=follow_rules");
}
#[test]
fn test_failover_routing_graph_resolution_and_cycle() {
    let mut graph = FailoverRoutingGraph::new();
    graph.link_fallback("HK-01", "HK-02");
    graph.link_fallback("HK-02", "JP-01");

    assert!(!graph.has_cycle_from("HK-01"));

    // If HK-01 is healthy -> HK-01
    assert_eq!(graph.resolve_active_outbound("HK-01", |_| false), "HK-01");

    // If HK-01 is tripped, HK-02 healthy -> HK-02
    assert_eq!(
        graph.resolve_active_outbound("HK-01", |node| node == "HK-01"),
        "HK-02"
    );

    // If HK-01 and HK-02 tripped -> JP-01
    assert_eq!(
        graph.resolve_active_outbound("HK-01", |node| node.starts_with("HK")),
        "JP-01"
    );

    // Circular link detection
    graph.link_fallback("JP-01", "HK-01");
    assert!(graph.has_cycle_from("HK-01"));
    // Safely degrades to DIRECT on circular loop with all tripped
    assert_eq!(graph.resolve_active_outbound("HK-01", |_| true), "DIRECT");
}
#[test]
fn test_root_cause_inference_engine_priority() {
    // HighPacketLoss + TunInterfaceMissing + DnsLeakDetected -> TunInterfaceMissing is primary
    let anomalies = vec![
        DiagnosticAnomaly::HighPacketLoss,
        DiagnosticAnomaly::DnsLeakDetected,
        DiagnosticAnomaly::TunInterfaceMissing,
    ];
    let root = RootCauseInferenceEngine::identify_primary(&anomalies);
    assert_eq!(root, Some(DiagnosticAnomaly::TunInterfaceMissing));

    // If ZombieProcessDetected is also present, it takes top priority
    let with_zombie = vec![
        DiagnosticAnomaly::HighPacketLoss,
        DiagnosticAnomaly::ZombieProcessDetected,
        DiagnosticAnomaly::TunInterfaceMissing,
    ];
    assert_eq!(
        RootCauseInferenceEngine::identify_primary(&with_zombie),
        Some(DiagnosticAnomaly::ZombieProcessDetected)
    );
}
#[test]
fn test_healing_watchdog_timeout_trip() {
    let mut watchdog = HealingWatchdog::new(Duration::from_secs(4));
    assert!(!watchdog.is_repairing());
    assert_eq!(watchdog.stage(), &WatchdogStage::Idle);

    watchdog.arm("rebind_tun").expect("idle watchdog arms");
    assert!(watchdog.is_repairing());
    assert_eq!(watchdog.active_action_name(), Some("rebind_tun"));

    // Advance 2s -> does not trip
    assert!(!watchdog.tick(Duration::from_secs(2)));
    assert!(watchdog.is_repairing());

    // Advance 3s more (5s total >= 4s limit) -> trips watchdog
    assert!(watchdog.tick(Duration::from_secs(3)));
    assert!(watchdog.has_tripped());
    assert!(!watchdog.is_repairing());
    assert_eq!(watchdog.active_action_name(), Some("rebind_tun"));
}
