//! Digital twin chaos network injection sandbox and automated headless monkey test explorer.

use bevy::ecs::resource::Resource;

/// Configurable network chaos fault parameters injected into UI data streams.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ChaosFaultConfig {
    pub is_enabled: bool,
    pub latency_jitter_ms: f32,
    pub packet_drop_rate: f32,
    pub inject_controller_panic: bool,
    pub inject_invalid_yaml: bool,
}

impl Default for ChaosFaultConfig {
    fn default() -> Self {
        Self {
            is_enabled: false,
            latency_jitter_ms: 0.0,
            packet_drop_rate: 0.0,
            inject_controller_panic: false,
            inject_invalid_yaml: false,
        }
    }
}

impl ChaosFaultConfig {
    pub fn should_drop_packet(&self, random_seed: f32) -> bool {
        self.is_enabled && random_seed < self.packet_drop_rate
    }
}

/// Autonomous headless explorer monkey bot executing pseudo-random UI interactions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonkeyExplorerBot {
    pub actions_executed: u64,
    pub routes_visited: Vec<String>,
    pub exceptions_caught: u64,
}

impl MonkeyExplorerBot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_action(&mut self, route: impl Into<String>) {
        self.actions_executed += 1;
        self.routes_visited.push(route.into());
    }

    pub fn record_exception(&mut self) {
        self.exceptions_caught += 1;
    }
}

use crate::auto_heal::DiagnosticAnomaly;

/// Concrete fault types that can be injected into UI pipelines.
#[derive(Clone, Debug, PartialEq)]
pub enum ChaosFaultType {
    LatencySpike(f32),
    DnsPollution { fake_ip: String },
    TunInterfaceDrop,
    ControllerDisconnect(u16),
    HighPacketLoss(f32),
    ZombieCore,
}

/// A structured network chaos fault scenario for resilience exercises.
#[derive(Clone, Debug, PartialEq)]
pub struct ChaosFaultScenario {
    pub name: String,
    pub fault_type: ChaosFaultType,
    pub duration_secs: f32,
    pub expected_anomaly: DiagnosticAnomaly,
}

impl ChaosFaultScenario {
    pub fn port_conflict(port: u16) -> Self {
        Self {
            name: "Controller Port Conflict Simulation".to_string(),
            fault_type: ChaosFaultType::ControllerDisconnect(port),
            duration_secs: 15.0,
            expected_anomaly: DiagnosticAnomaly::ControllerPortConflict(port),
        }
    }

    pub fn tun_failure() -> Self {
        Self {
            name: "TUN Device Unbind Simulation".to_string(),
            fault_type: ChaosFaultType::TunInterfaceDrop,
            duration_secs: 20.0,
            expected_anomaly: DiagnosticAnomaly::TunInterfaceMissing,
        }
    }

    pub fn dns_pollution() -> Self {
        Self {
            name: "DNS Leak and Pollution Simulation".to_string(),
            fault_type: ChaosFaultType::DnsPollution {
                fake_ip: "198.18.0.1".to_string(),
            },
            duration_secs: 10.0,
            expected_anomaly: DiagnosticAnomaly::DnsLeakDetected,
        }
    }
}

/// Interactive chaos simulation runner managing scheduled fault injections.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ChaosSimulationRunner {
    pub active_scenario: Option<ChaosFaultScenario>,
    pub elapsed_secs: f32,
    pub is_running: bool,
}

impl ChaosSimulationRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inject(&mut self, scenario: ChaosFaultScenario) {
        self.active_scenario = Some(scenario);
        self.elapsed_secs = 0.0;
        self.is_running = true;
    }

    pub fn stop(&mut self) {
        self.active_scenario = None;
        self.elapsed_secs = 0.0;
        self.is_running = false;
    }

    pub fn tick(&mut self, dt_secs: f32) -> Option<DiagnosticAnomaly> {
        if !self.is_running {
            return None;
        }
        self.elapsed_secs += dt_secs;
        if let Some(ref scenario) = self.active_scenario {
            if self.elapsed_secs <= scenario.duration_secs {
                Some(scenario.expected_anomaly)
            } else {
                self.stop();
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_scenario_injection_and_auto_heal_mapping() {
        let mut runner = ChaosSimulationRunner::new();
        assert!(!runner.is_running);

        let scenario = ChaosFaultScenario::port_conflict(9099);
        runner.inject(scenario);
        assert!(runner.is_running);

        // First tick returns expected anomaly
        let anomaly = runner.tick(1.0).expect("anomaly emitted");
        assert_eq!(anomaly, DiagnosticAnomaly::ControllerPortConflict(9099));

        // Advance past duration (15s)
        let end = runner.tick(16.0);
        assert!(end.is_none());
        assert!(!runner.is_running);
    }
}
