//! Headless coverage for the self-heal signal projection, the sandbox
//! permission boundary and the typed ABI capability negotiation.

use super::*;

#[test]
fn test_context_aware_heal_best_node_and_unknown_handling() {
    use infiltrator_bevy_widgets::auto_heal::{
        ContextAwareHealDetector, HeuristicNodeEvaluator, NodeCandidate, NodeEvaluation,
        NodeObservation, NodeProbeHistory, UnscorableReason,
    };

    let mut fast = NodeCandidate::new("HK-Fast");
    fast.observation = NodeObservation {
        latency_ms: Some(40.0),
        loss_ratio: Some(0.0),
    };
    let mut slow = NodeCandidate::new("US-Slow");
    slow.observation = NodeObservation {
        latency_ms: Some(220.0),
        loss_ratio: Some(0.01),
    };
    let unknown = NodeCandidate::new("JP-Unknown");

    let evaluator = HeuristicNodeEvaluator::new();
    let candidates = [slow.clone(), fast.clone(), unknown.clone()];
    let best = evaluator
        .best(&candidates)
        .expect("a scored candidate exists");
    assert_eq!(best.name, "HK-Fast");

    // Unknown facts are never fabricated into a score.
    assert_eq!(
        evaluator.evaluate(&unknown),
        NodeEvaluation::Unscorable {
            reason: UnscorableReason::LatencyAndLossUnobserved
        }
    );
    assert_eq!(
        evaluator.evaluate(&NodeCandidate {
            name: "Loss-Only".to_string(),
            observation: NodeObservation {
                latency_ms: None,
                loss_ratio: Some(0.1),
            },
            history: NodeProbeHistory::default(),
        }),
        NodeEvaluation::Unscorable {
            reason: UnscorableReason::LatencyUnobserved
        }
    );

    // Missing history is neutral, not a fabricated perfect ratio.
    match evaluator.evaluate(&fast) {
        NodeEvaluation::Scored { history_ratio, .. } => assert_eq!(history_ratio, None),
        NodeEvaluation::Unscorable { .. } => panic!("fast node must be scorable"),
    }

    // A failing history raises the score and drops the node below a clean peer.
    let mut flaky = NodeCandidate::new("SG-Flaky");
    flaky.observation = NodeObservation {
        latency_ms: Some(40.0),
        loss_ratio: Some(0.0),
    };
    flaky.history = NodeProbeHistory {
        successes: 1,
        failures: 9,
    };
    let clean_score = match evaluator.evaluate(&fast) {
        NodeEvaluation::Scored { score, .. } => score,
        NodeEvaluation::Unscorable { .. } => panic!("fast node must be scorable"),
    };
    let flaky_score = match evaluator.evaluate(&flaky) {
        NodeEvaluation::Scored { score, .. } => score,
        NodeEvaluation::Unscorable { .. } => panic!("flaky node must be scorable"),
    };
    assert!(clean_score < flaky_score);
    assert_eq!(
        evaluator
            .best(&[flaky, fast])
            .map(|node| node.name.as_str()),
        Some("HK-Fast")
    );

    // An all-unobserved candidate set yields no fabricated winner.
    let detector = ContextAwareHealDetector::new();
    assert!(detector.best(&[unknown]).is_none());
}

#[test]
fn test_self_heal_anomaly_signals_for_ui() {
    use infiltrator_bevy_widgets::auto_heal::{
        AnomalySeverity, ContextAwareHealDetector, NodeCandidate, NodeObservation,
        NodeProbeHistory, SelfHealAnomaly, SelfHealSignalBoard, UnscorableReason,
    };

    let mut healthy = NodeCandidate::new("HK-01");
    healthy.observation = NodeObservation {
        latency_ms: Some(35.0),
        loss_ratio: Some(0.0),
    };
    healthy.history = NodeProbeHistory {
        successes: 10,
        failures: 0,
    };

    let mut lossy = NodeCandidate::new("US-01");
    lossy.observation = NodeObservation {
        latency_ms: Some(120.0),
        loss_ratio: Some(0.4),
    };

    let mut dead = NodeCandidate::new("JP-01");
    dead.history = NodeProbeHistory {
        successes: 0,
        failures: 5,
    };

    let detector = ContextAwareHealDetector::new();
    let anomalies = detector.detect(&[healthy.clone(), lossy.clone(), dead.clone()]);
    assert!(anomalies.iter().any(|anomaly| matches!(
        anomaly,
        SelfHealAnomaly::ExcessiveLoss { node, .. } if node == "US-01"
    )));
    assert!(anomalies.iter().any(|anomaly| matches!(
        anomaly,
        SelfHealAnomaly::NodeUnobserved {
            node,
            reason: UnscorableReason::LatencyAndLossUnobserved
        } if node == "JP-01"
    )));
    assert!(
        !anomalies
            .iter()
            .any(|anomaly| anomaly.node_name() == "HK-01")
    );

    let mut board = SelfHealSignalBoard::new();
    board.refresh(&detector, &[healthy, lossy, dead]);
    assert_eq!(board.best_node.as_deref(), Some("HK-01"));
    assert_eq!(board.highest_severity(), Some(AnomalySeverity::Critical));
    assert!(board.count_at_least(AnomalySeverity::Warning) >= 2);
}

#[test]
fn test_sandbox_quota_and_permission_boundary() {
    use infiltrator_bevy_widgets::sandbox::{
        HostCapability, SandboxFault, WidgetManifest, WidgetPermission,
    };

    let mut widget = SandboxedWidgetInstance::new("w_dashboard", "community_dashboard");
    let manifest = WidgetManifest::new("w_dashboard", "Community Dashboard")
        .with_permission(WidgetPermission::ReadTrafficStats);
    assert!(widget.bind_manifest(&manifest).is_ok());

    // Typed permission check yields a frame-scoped grant.
    let grant = widget.authorize(HostCapability::ReadTrafficStats).unwrap();
    assert_eq!(grant.capability(), HostCapability::ReadTrafficStats);
    assert_eq!(grant.frame(), 0);

    // Denied permission blocks the capability with a typed fault.
    assert_eq!(
        widget.authorize(HostCapability::ManageProfiles),
        Err(SandboxFault::CapabilityDenied(
            HostCapability::ManageProfiles
        ))
    );

    // Network read is denied by default.
    assert_eq!(
        widget.authorize(HostCapability::NetworkRead),
        Err(SandboxFault::CapabilityDenied(HostCapability::NetworkRead))
    );

    // Quota exceeded fails closed: quarantine refuses even granted capabilities.
    assert!(widget.reserve_memory(widget.quota.max_memory_bytes).is_ok());
    assert!(widget.reserve_memory(1).is_err());
    assert!(widget.is_quarantined());
    assert_eq!(
        widget.authorize(HostCapability::ReadTrafficStats),
        Err(SandboxFault::Quarantined)
    );
}

#[test]
fn test_sandbox_manifest_validation_and_render_slot() {
    use infiltrator_bevy_widgets::sandbox::WidgetManifest;

    let mut widget = SandboxedWidgetInstance::new("w_map", "community_map");
    let mut manifest = WidgetManifest::new("w_map", "Community Map");
    manifest.min_bevy_version = "0.19.0".to_string();
    assert!(widget.bind_manifest(&manifest).is_err());
    assert!(widget.granted_permissions().is_empty());

    // The render slot is an opaque descriptor: no ECS/World handle is exposed.
    let slot = widget.render_slot();
    assert!(slot.is_valid());
    let resized = slot.resized(640, 360);
    assert_eq!(resized.slot_id(), slot.slot_id());
    assert_eq!(resized.revision(), slot.revision() + 1);
}

#[test]
fn test_abi_capability_negotiation_typed() {
    use infiltrator_bevy_widgets::abi::{
        AbiNegotiation, HostCapabilities, WidgetAbiRequirement, WidgetCapability, negotiate,
    };

    // Compatible: host advertises, widget requires.
    let requirement =
        WidgetAbiRequirement::for_current_abi().with_capability(WidgetCapability::GpuShaders);
    let outcome = negotiate(
        WIDGET_ABI_VERSION,
        HostCapabilities::all_desktop(),
        requirement,
    );
    assert!(outcome.is_compatible());

    // Missing capability is typed, not a panic.
    let requirement =
        WidgetAbiRequirement::for_current_abi().with_capability(WidgetCapability::MultiWindow);
    let outcome = negotiate(
        WIDGET_ABI_VERSION,
        HostCapabilities::mobile_default(),
        requirement,
    );
    assert_eq!(outcome.granted(), None);
    assert!(
        outcome
            .missing()
            .unwrap()
            .has(WidgetCapability::MultiWindow)
    );

    // Version mismatch is rejected, never silently downgraded.
    let outcome = negotiate(
        WIDGET_ABI_VERSION,
        HostCapabilities::all_desktop(),
        WidgetAbiRequirement::new((2, 0, 0)),
    );
    assert!(matches!(outcome, AbiNegotiation::VersionMismatch { .. }));
}
