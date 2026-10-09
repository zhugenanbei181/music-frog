//! Digital twin chaos network injection sandbox and automated headless monkey test explorer.

use bevy::ecs::resource::Resource;
use std::collections::HashSet;

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

    pub fn latency_spike(jitter_ms: f32) -> Self {
        Self {
            name: "Latency Spike Simulation".to_string(),
            fault_type: ChaosFaultType::LatencySpike(jitter_ms),
            duration_secs: 12.0,
            expected_anomaly: DiagnosticAnomaly::HighPacketLoss,
        }
    }

    pub fn packet_loss(rate: f32) -> Self {
        Self {
            name: "High Packet Loss Simulation".to_string(),
            fault_type: ChaosFaultType::HighPacketLoss(rate),
            duration_secs: 15.0,
            expected_anomaly: DiagnosticAnomaly::HighPacketLoss,
        }
    }

    pub fn zombie_core() -> Self {
        Self {
            name: "Zombie Core Process Simulation".to_string(),
            fault_type: ChaosFaultType::ZombieCore,
            duration_secs: 8.0,
            expected_anomaly: DiagnosticAnomaly::ZombieProcessDetected,
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

/// Deterministic SplitMix64 generator. The chaos sandbox must be reproducible:
/// the same seed yields the same fault schedule, so a state found by the monkey
/// explorer can be replayed exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub const fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform `f32` in `[0, 1)` derived from the top 24 bits.
    pub fn next_unit(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / (1u64 << 24) as f32
    }

    /// Deterministic index in `[0, upper)`; `0` when `upper` is zero.
    pub fn next_below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() % upper as u64) as usize
        }
    }
}

/// Fault modes the injection engine can schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FaultMode {
    LatencySpike,
    PacketDrop,
    ControllerPanic,
    InvalidYaml,
    TunDrop,
    ZombieCore,
}

/// A single scheduled fault injection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InjectedFault {
    pub step: u64,
    pub mode: FaultMode,
    pub magnitude: f32,
}

/// Deterministic fault-injection engine. Identical `(seed, config, steps)`
/// always produce the identical fault schedule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChaosFaultEngine {
    rng: DeterministicRng,
    config: ChaosFaultConfig,
    step: u64,
}

impl ChaosFaultEngine {
    pub fn new(seed: u64, config: ChaosFaultConfig) -> Self {
        Self {
            rng: DeterministicRng::seeded(seed),
            config,
            step: 0,
        }
    }

    pub fn config(&self) -> ChaosFaultConfig {
        self.config
    }

    pub fn step_index(&self) -> u64 {
        self.step
    }

    /// Deterministic route-selection draw for the monkey explorer.
    pub fn next_route_index(&mut self, route_count: usize) -> usize {
        self.rng.next_below(route_count)
    }

    /// Advance one step and return the scheduled fault, if any. Two draws are
    /// always consumed so the schedule does not shift with config changes.
    pub fn next_fault(&mut self) -> Option<InjectedFault> {
        self.step += 1;
        let roll = self.rng.next_unit();
        let magnitude = self.rng.next_unit();
        if !self.config.is_enabled {
            return None;
        }
        let mode = if self.config.inject_controller_panic && roll < 0.10 {
            FaultMode::ControllerPanic
        } else if self.config.inject_invalid_yaml && roll < 0.20 {
            FaultMode::InvalidYaml
        } else if roll < self.config.packet_drop_rate {
            FaultMode::PacketDrop
        } else if self.config.latency_jitter_ms > 0.0 && roll < 0.30 + self.config.packet_drop_rate
        {
            FaultMode::LatencySpike
        } else {
            return None;
        };
        Some(InjectedFault {
            step: self.step,
            mode,
            magnitude,
        })
    }
}

/// A distinct state reached by the monkey explorer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredState {
    pub step: u64,
    pub route: String,
    pub fault_mode: Option<FaultMode>,
}

/// Bounded headless monkey explorer. It walks at most `max_steps` deterministic
/// interactions and records every distinct `(route, fault)` state it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundedMonkeyExplorer {
    pub max_steps: u64,
    pub steps_executed: u64,
    pub states: Vec<DiscoveredState>,
    seen_states: HashSet<(String, Option<FaultMode>)>,
}

impl BoundedMonkeyExplorer {
    pub fn new(max_steps: u64) -> Self {
        Self {
            max_steps,
            steps_executed: 0,
            states: Vec::new(),
            seen_states: HashSet::new(),
        }
    }

    /// Run the deterministic exploration. The same `(seed, config, routes)`
    /// yields the identical `states` sequence.
    pub fn explore(
        &mut self,
        routes: &[&str],
        seed: u64,
        config: ChaosFaultConfig,
    ) -> &[DiscoveredState] {
        self.steps_executed = 0;
        self.states.clear();
        self.seen_states.clear();
        if routes.is_empty() {
            return &self.states;
        }
        let mut engine = ChaosFaultEngine::new(seed, config);
        while self.steps_executed < self.max_steps {
            let index = engine.next_route_index(routes.len());
            let route = routes[index].to_string();
            let fault_mode = engine.next_fault().map(|fault| fault.mode);
            self.steps_executed += 1;
            let key = (route.clone(), fault_mode);
            if self.seen_states.insert(key) {
                self.states.push(DiscoveredState {
                    step: self.steps_executed,
                    route,
                    fault_mode,
                });
            }
        }
        &self.states
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

    #[test]
    fn test_deterministic_rng_repeats_for_same_seed() {
        let mut first = DeterministicRng::seeded(42);
        let mut second = DeterministicRng::seeded(42);
        for _ in 0..64 {
            assert_eq!(first.next_u64(), second.next_u64());
        }
        let mut other = DeterministicRng::seeded(43);
        assert_ne!(first.next_u64(), other.next_u64());
    }

    #[test]
    fn test_chaos_engine_schedule_is_seed_deterministic() {
        let config = ChaosFaultConfig {
            is_enabled: true,
            latency_jitter_ms: 30.0,
            packet_drop_rate: 0.2,
            inject_controller_panic: true,
            inject_invalid_yaml: true,
        };
        let mut first = ChaosFaultEngine::new(7, config);
        let mut second = ChaosFaultEngine::new(7, config);
        let schedule_a: Vec<InjectedFault> = (0..32).filter_map(|_| first.next_fault()).collect();
        let schedule_b: Vec<InjectedFault> = (0..32).filter_map(|_| second.next_fault()).collect();
        assert_eq!(schedule_a, schedule_b);
        assert!(!schedule_a.is_empty());

        let mut other = ChaosFaultEngine::new(8, config);
        let schedule_c: Vec<InjectedFault> = (0..32).filter_map(|_| other.next_fault()).collect();
        assert_ne!(schedule_a, schedule_c);
    }

    #[test]
    fn test_chaos_engine_disabled_and_packet_drop_modes() {
        let mut disabled = ChaosFaultEngine::new(1, ChaosFaultConfig::default());
        for _ in 0..16 {
            assert!(disabled.next_fault().is_none());
        }

        let drop_all = ChaosFaultConfig {
            is_enabled: true,
            packet_drop_rate: 1.0,
            ..Default::default()
        };
        let mut engine = ChaosFaultEngine::new(3, drop_all);
        for _ in 0..16 {
            let fault = engine.next_fault().expect("packet drop scheduled");
            assert_eq!(fault.mode, FaultMode::PacketDrop);
        }
    }

    #[test]
    fn test_chaos_engine_single_mode_configs() {
        let latency = ChaosFaultConfig {
            is_enabled: true,
            latency_jitter_ms: 25.0,
            ..Default::default()
        };
        let mut engine = ChaosFaultEngine::new(11, latency);
        let modes: Vec<FaultMode> = (0..64)
            .filter_map(|_| engine.next_fault())
            .map(|fault| fault.mode)
            .collect();
        assert!(modes.contains(&FaultMode::LatencySpike));
        assert!(modes.iter().all(|mode| *mode == FaultMode::LatencySpike));

        let panic = ChaosFaultConfig {
            is_enabled: true,
            inject_controller_panic: true,
            ..Default::default()
        };
        let mut engine = ChaosFaultEngine::new(5, panic);
        let modes: Vec<FaultMode> = (0..64)
            .filter_map(|_| engine.next_fault())
            .map(|fault| fault.mode)
            .collect();
        assert!(modes.contains(&FaultMode::ControllerPanic));
        assert!(modes.iter().all(|mode| *mode == FaultMode::ControllerPanic));
    }

    #[test]
    fn test_bounded_monkey_explorer_determinism_and_bound() {
        let routes = ["Overview", "Proxies", "Rules", "Logs"];
        let config = ChaosFaultConfig {
            is_enabled: true,
            latency_jitter_ms: 20.0,
            packet_drop_rate: 0.25,
            ..Default::default()
        };
        let mut first = BoundedMonkeyExplorer::new(40);
        let states_a = first.explore(&routes, 99, config).to_vec();
        let mut second = BoundedMonkeyExplorer::new(40);
        let states_b = second.explore(&routes, 99, config).to_vec();
        assert_eq!(states_a, states_b);
        assert_eq!(first.steps_executed, 40);

        let mut bounded = BoundedMonkeyExplorer::new(3);
        bounded.explore(&routes, 99, config);
        assert_eq!(bounded.steps_executed, 3);
        assert!(bounded.states.len() <= 3);

        let mut empty = BoundedMonkeyExplorer::new(10);
        empty.explore(&[], 1, config);
        assert_eq!(empty.steps_executed, 0);
        assert!(empty.states.is_empty());
    }

    #[test]
    fn test_monkey_explorer_records_distinct_fault_states() {
        let routes = ["Overview", "Proxies"];
        let drop_all = ChaosFaultConfig {
            is_enabled: true,
            packet_drop_rate: 1.0,
            ..Default::default()
        };
        let mut explorer = BoundedMonkeyExplorer::new(8);
        let states = explorer.explore(&routes, 4, drop_all).to_vec();
        assert!(!states.is_empty());
        assert!(
            states
                .iter()
                .all(|state| state.fault_mode == Some(FaultMode::PacketDrop))
        );

        let mut keys: Vec<(String, Option<FaultMode>)> = states
            .iter()
            .map(|state| (state.route.clone(), state.fault_mode))
            .collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), states.len());
    }
}
