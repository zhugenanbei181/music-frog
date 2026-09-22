//! DUAL-06-14: application seam for the Group 06 speedtest regression matrix.

use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_contract::speedtest_matrix::SpeedtestRegressionMatrixReport;

/// Application facade providing on-demand verification of the speedtest suite.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpeedtestMatrixApplication;

impl SpeedtestMatrixApplication {
    /// Run the deterministic shared matrix over all Group 06 capabilities.
    pub fn execute(&self) -> SpeedtestRegressionMatrixReport {
        SpeedtestRegressionMatrixReport::run_deterministic_matrix()
    }

    /// Verify that an arbitrary live snapshot is a legitimate single fact
    /// source: both UI surfaces project exactly this read model, so a snapshot
    /// is "dual-surface ready" when every measured node carries its metrics and
    /// the lifecycle phase is unambiguous.
    pub fn verify_dual_surface_snapshot(&self, snapshot: &SpeedtestSnapshot) -> bool {
        let running = snapshot.is_running();
        let known_phase = match snapshot.phase {
            infiltrator_contract::speedtest::SpeedtestPhase::ProbingLatency
            | infiltrator_contract::speedtest::SpeedtestPhase::MeasuringBandwidth => running,
            _ => !running,
        };
        let metrics_consistent = snapshot.node_results.values().all(|node| {
            node.is_alive == (node.delay_ms.is_some() || node.bandwidth_mbps.is_some())
                || !node.is_alive
        });
        known_phase && metrics_consistent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speedtest_matrix_application_execution() {
        let report = SpeedtestMatrixApplication.execute();
        assert!(report.is_all_passed());
        assert_eq!(report.total_scenarios, 15);
        assert_eq!(report.passed_scenarios, 15);
    }

    #[test]
    fn test_verify_dual_surface_snapshot_accepts_demo_and_rejects_inconsistent() {
        let app = SpeedtestMatrixApplication;
        let demo = SpeedtestSnapshot::demo_fixture();
        assert!(app.verify_dual_surface_snapshot(&demo));

        let mut inconsistent = SpeedtestSnapshot {
            phase: infiltrator_contract::speedtest::SpeedtestPhase::ProbingLatency,
            ..Default::default()
        };
        inconsistent.node_results.insert(
            "Dead-But-Alive".to_owned(),
            infiltrator_contract::speedtest::NodeSpeedtestResult {
                node_name: "Dead-But-Alive".to_owned(),
                group_name: None,
                proxy_type: "Shadowsocks".to_owned(),
                delay_ms: None,
                jitter: None,
                bandwidth_mbps: None,
                packet_loss: infiltrator_contract::speedtest::PacketLossRating::Dead,
                star_rating: 1,
                label_country: None,
                outbound_ip: None,
                outbound_country: None,
                is_alive: true,
                tested_at_epoch_ms: 0,
            },
        );
        assert!(!app.verify_dual_surface_snapshot(&inconsistent));
    }
}
