use super::*;
#[test]
fn flow_strip_keeps_phase_bounded_and_uses_shared_snapshot() {
    let snapshot = TrafficTopologySnapshot::demo_fixture();
    let strip = TopologyFlowCanvas {
        snapshot: snapshot.clone(),
        phase: 1.75,
    };
    assert_eq!(strip.snapshot, snapshot);
    assert!((strip.phase - 1.75).abs() < f32::EPSILON);
    assert!(strip.snapshot.is_flowing());
}
