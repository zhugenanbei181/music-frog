//! High-level runtime facade over the provider registry and switch coordinator.
//!
//! [`LurkerRuntime`] is the small SDK surface an embedding host exposes to an
//! application: register engine providers, select one, start/stop the Core and
//! replace the active engine through [`LurkerRuntime::switch_engine`]. Every
//! adapter fact still comes from the neutral ports; the facade only owns the
//! explicit lifecycle state machine, the control-operation fence and the state
//! fan-out.
//!
//! The facade starts every Core with `CoreProcessStartContext::Standalone`.
//! Android's `Managed` owner context is composed by the Android product root,
//! which keeps the VPNService session proof and the managed child recovery
//! evidence outside this engine-neutral layer.

mod operations;
mod process;
mod state_stream;
mod switch_host;

use crate::error::EngineBackendRegistryError;
use crate::registry::EngineBackendProviderRegistry;
use crate::switch::error::EngineSwitchError;
use crate::switch::memory_journal::MemoryEngineSwitchJournal;
use futures_channel::mpsc;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::engine_switch::{EngineSwitchFailurePhase, EngineSwitchStage};
use lurker_core_contract::identity::{CoreInstanceId, CoreScope, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::engine_backend::{
    ENGINE_BACKEND_CAPABILITY, EngineBackendPorts, EngineBackendProvider,
    EngineBackendProviderAvailability,
};
use lurker_core_ports::engine_switch::{EngineOwnerLease, EngineSwitchJournal};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// Default bound applied while waiting for a freshly started Core to become
/// ready.
pub const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// One exhaustive runtime state. No phase is encoded by a Boolean or an
/// optional field combination: every legal state is its own variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LurkerRuntimeState {
    /// No Core process is owned. `selected` is the engine the runtime was
    /// configured to start.
    Idle {
        /// The engine the runtime was configured to start.
        selected: Option<EngineId>,
    },
    /// A Core process is being prepared, started or probed for readiness.
    Starting {
        /// The engine whose Core is being started.
        engine: EngineId,
    },
    /// The selected engine owns a ready Core process.
    Running {
        /// The engine that owns the running Core.
        engine: EngineId,
        /// The scope every scoped operation is fenced to.
        scope: CoreScope,
        /// The version the running engine reported, when it is known.
        version: Option<CoreVersion>,
    },
    /// The coordinator is replacing the active owner and has published the
    /// exact durable stage it has reached.
    Switching {
        /// The engine whose owner is being closed.
        from: EngineId,
        /// The engine that will own the Core after the switch.
        to: EngineId,
        /// The exact durable stage the switch reached.
        stage: EngineSwitchStage<EngineId>,
    },
    /// The active Core process is being stopped.
    Stopping {
        /// The engine whose Core is being stopped.
        engine: EngineId,
        /// The scope of the owner that is being stopped.
        scope: CoreScope,
    },
    /// Stop reached the exact Stopped/no-PID terminal. `terminal` is the
    /// lifecycle readback derived from that terminal evidence.
    Stopped {
        /// The engine that was stopped, when one was owned.
        engine: Option<EngineId>,
        /// The lifecycle readback of the observed terminal.
        terminal: Option<CoreLifecycleSnapshot>,
    },
    /// A switch failed after the previous owner could no longer be restored.
    /// The journal keeps the exact stage and this state names the failure
    /// phase that must be resumed by the embedding host.
    RecoveryRequired {
        /// The selection the journal holds.
        selection: EngineId,
        /// The switch phase that must be resumed.
        phase: EngineSwitchFailurePhase,
        /// The adapter failure that stopped the switch.
        failure: PortError,
    },
    /// A non-switch lifecycle operation failed.
    Failed {
        /// The engine the failure belongs to, when one was known.
        engine: Option<EngineId>,
        /// The lifecycle phase that failed.
        phase: RuntimeFailurePhase,
        /// The adapter failure that stopped the operation.
        failure: PortError,
    },
}

/// The lifecycle operation phase that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeFailurePhase {
    /// The registry could not produce a backend for the configured engine.
    BackendDispatch,
    /// The backend process adapter has no usable Core identity.
    Identity,
    /// `CoreProcess::prepare_start` rejected or failed the launch.
    PrepareStart,
    /// The durable journal could not bind the first owner.
    JournalBindOwner,
    /// The Core process could not be started.
    StartProcess,
    /// The bounded readiness wait failed.
    Readiness,
    /// The backend could not bind the owner scope for its runtime operations.
    BindOwnerScope,
    /// The running state could not be projected after a successful switch.
    ScopeProjection,
    /// `CoreProcess::stop` itself failed.
    StopProcess,
    /// Stop succeeded but the exact Stopped/no-PID terminal was not observed.
    StopTerminal,
    /// The owner lease could not be released after observing the terminal.
    OwnerRelease,
}

/// A typed control or lifecycle failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// Another start, stop or switch is already in flight.
    ControlBusy,
    /// The requested engine has no registered provider.
    UnregisteredEngine {
        /// The engine that has no registered provider.
        engine_id: EngineId,
        /// Human-readable reason the engine is unavailable.
        reason: String,
    },
    /// The operation is not legal in the current state.
    InvalidState {
        /// The state that refused the operation.
        state: Box<LurkerRuntimeState>,
    },
    /// The runtime is not running yet and cannot become ready by itself.
    NotReady {
        /// The state that proved the runtime is not ready.
        state: Box<LurkerRuntimeState>,
    },
    /// The runtime did not reach `Running` before the supplied bound.
    ReadinessTimeout {
        /// The engine that did not become ready, when one was known.
        engine_id: Option<EngineId>,
        /// The bound that elapsed.
        timeout: Duration,
    },
    /// The state stream was closed before readiness was observed.
    StateStreamClosed,
    /// A port adapter failed outside an owner switch.
    Port(PortError),
    /// The coordinator rejected or failed an owner switch.
    Switch(EngineSwitchError<EngineId>),
}

impl Display for RuntimeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlBusy => {
                formatter.write_str("another runtime control operation is in flight")
            }
            Self::UnregisteredEngine { engine_id, reason } => write!(
                formatter,
                "no engine backend provider is registered for {}: {reason}",
                engine_id.as_str()
            ),
            Self::InvalidState { state } => {
                write!(
                    formatter,
                    "runtime operation is not legal in state {state:?}"
                )
            }
            Self::NotReady { state } => {
                write!(formatter, "runtime is not ready in state {state:?}")
            }
            Self::ReadinessTimeout { engine_id, timeout } => match engine_id {
                Some(engine_id) => write!(
                    formatter,
                    "{} runtime did not become ready within {timeout:?}",
                    engine_id.as_str()
                ),
                None => write!(formatter, "runtime did not become ready within {timeout:?}"),
            },
            Self::StateStreamClosed => formatter.write_str("runtime state stream closed"),
            Self::Port(error) => Display::fmt(error, formatter),
            Self::Switch(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Port(error) => Some(error),
            Self::Switch(error) => Some(error),
            Self::ControlBusy
            | Self::UnregisteredEngine { .. }
            | Self::InvalidState { .. }
            | Self::NotReady { .. }
            | Self::ReadinessTimeout { .. }
            | Self::StateStreamClosed => None,
        }
    }
}

/// A builder that fails closed instead of producing a partially composed
/// runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeBuildError {
    /// No provider was registered.
    EmptyRegistry,
    /// No engine was selected.
    MissingSelection,
    /// The selected engine has no registered provider.
    EngineNotRegistered {
        /// The engine that has no registered provider.
        engine_id: EngineId,
    },
    /// A provider is already registered for the same engine.
    DuplicateProvider {
        /// The engine the duplicate registration targets.
        engine_id: EngineId,
    },
}

impl Display for RuntimeBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRegistry => formatter.write_str("no engine backend provider is registered"),
            Self::MissingSelection => formatter.write_str("no engine is selected"),
            Self::EngineNotRegistered { engine_id } => write!(
                formatter,
                "no engine backend provider is registered for {}",
                engine_id.as_str()
            ),
            Self::DuplicateProvider { engine_id } => write!(
                formatter,
                "an engine backend provider is already registered for {}",
                engine_id.as_str()
            ),
        }
    }
}

impl Error for RuntimeBuildError {}

/// Builds a [`LurkerRuntime`] from registered engine providers.
///
/// The journal defaults to [`MemoryEngineSwitchJournal`], which is suitable for
/// one process and deterministic tests. Hosts that persist the owner selection
/// supply their own `EngineSwitchJournal<Selection = EngineId>`.
pub struct LurkerRuntimeBuilder {
    registry: EngineBackendProviderRegistry<EngineBackendPorts>,
    provider_count: usize,
    selected: Option<EngineId>,
    expected_instance: Option<CoreInstanceId>,
    journal: Option<Arc<dyn EngineSwitchJournal<Selection = EngineId>>>,
    readiness_timeout: Duration,
}

impl LurkerRuntimeBuilder {
    /// Creates an empty builder with no provider and no selection.
    pub fn new() -> Self {
        Self {
            registry: EngineBackendProviderRegistry::empty(),
            provider_count: 0,
            selected: None,
            expected_instance: None,
            journal: None,
            readiness_timeout: DEFAULT_READINESS_TIMEOUT,
        }
    }

    /// Register one provider for the engine it declares. Duplicate engines are
    /// rejected before any provider side effect.
    pub fn register_engine(
        mut self,
        provider: Arc<dyn EngineBackendProvider<EngineBackendPorts>>,
    ) -> Result<Self, RuntimeBuildError> {
        self.registry
            .register(provider)
            .map_err(|error| match error {
                EngineBackendRegistryError::DuplicateProvider { engine_id } => {
                    RuntimeBuildError::DuplicateProvider { engine_id }
                }
                EngineBackendRegistryError::UnknownProvider { engine_id } => {
                    RuntimeBuildError::EngineNotRegistered { engine_id }
                }
            })?;
        self.provider_count += 1;
        Ok(self)
    }

    /// Select the engine used by the first [`LurkerRuntime::start`].
    pub fn select_engine(mut self, engine_id: EngineId) -> Self {
        self.selected = Some(engine_id);
        self
    }

    /// Require every engine process adapter to report this configured instance
    /// identity. Omit it to accept the adapter-reported instance.
    pub fn with_instance_id(mut self, instance_id: CoreInstanceId) -> Self {
        self.expected_instance = Some(instance_id);
        self
    }

    /// Use a durable switch journal instead of the in-memory default.
    pub fn with_switch_journal<J>(mut self, journal: Arc<J>) -> Self
    where
        J: EngineSwitchJournal<Selection = EngineId> + 'static,
    {
        let journal: Arc<dyn EngineSwitchJournal<Selection = EngineId>> = journal;
        self.journal = Some(journal);
        self
    }

    /// Bound the readiness wait performed by `start` and by the switch host.
    pub fn with_readiness_timeout(mut self, timeout: Duration) -> Self {
        self.readiness_timeout = timeout;
        self
    }

    /// Build the runtime, rejecting an empty registry, a missing selection or
    /// a selection with no registered provider.
    pub fn build(self) -> Result<LurkerRuntime, RuntimeBuildError> {
        if self.provider_count == 0 {
            return Err(RuntimeBuildError::EmptyRegistry);
        }
        let selected = self.selected.ok_or(RuntimeBuildError::MissingSelection)?;
        match self.registry.availability(&selected) {
            EngineBackendProviderAvailability::Registered => {}
            EngineBackendProviderAvailability::Unsupported { .. } => {
                return Err(RuntimeBuildError::EngineNotRegistered {
                    engine_id: selected,
                });
            }
        }
        let journal = match self.journal {
            Some(journal) => journal,
            None => Arc::new(MemoryEngineSwitchJournal::new(selected.clone())),
        };
        Ok(LurkerRuntime {
            inner: Arc::new(RuntimeInner {
                registry: Arc::new(self.registry),
                journal,
                expected_instance: self.expected_instance,
                readiness_timeout: self.readiness_timeout,
                control: Mutex::new(ControlOperation { in_flight: false }),
                state: Mutex::new(StateCell {
                    current: LurkerRuntimeState::Idle {
                        selected: Some(selected),
                    },
                    subscribers: Vec::new(),
                }),
                active: Mutex::new(ActiveOwner::default()),
                terminal_revision: AtomicU64::new(0),
            }),
        })
    }
}

impl Default for LurkerRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// The high-level runtime handle. Cloning it shares the same control fence and
/// state stream.
#[derive(Clone)]
pub struct LurkerRuntime {
    inner: Arc<RuntimeInner>,
}

impl LurkerRuntime {
    /// The current exhaustive lifecycle state.
    pub fn state(&self) -> LurkerRuntimeState {
        self.inner.state()
    }

    /// Subscribe to lifecycle states. The current state is replayed first, then
    /// every later transition is delivered in order.
    pub fn state_stream(&self) -> LurkerRuntimeStateStream {
        let mut state = lock_state(&self.inner.state);
        let (sender, receiver) = mpsc::unbounded();
        let _ = sender.unbounded_send(state.current.clone());
        state.subscribers.push(sender);
        LurkerRuntimeStateStream { receiver }
    }

    /// The scope of the active owner, or `None` while no owner is bound.
    pub fn scope(&self) -> Option<CoreScope> {
        match self.inner.active.lock() {
            Ok(active) => active.scope.clone(),
            Err(_) => None,
        }
    }

    /// The active engine's outbound port.
    pub fn outbound(&self) -> Result<Arc<dyn OutboundPort>, PortError> {
        let active = lock_active(&self.inner.active)?;
        match &active.backend {
            Some(backend) => backend.operations().outbound(),
            None => Err(PortError::unsupported(
                ENGINE_BACKEND_CAPABILITY,
                "no active engine backend is bound",
            )),
        }
    }

    /// The active engine's routing-intent port.
    pub fn routing_intent(&self) -> Result<Arc<dyn RoutingIntentPort>, PortError> {
        let active = lock_active(&self.inner.active)?;
        match &active.backend {
            Some(backend) => backend.operations().routing_intent(),
            None => Err(PortError::unsupported(
                Capability::RoutingIntent,
                "no active engine backend is bound",
            )),
        }
    }
}

/// A replaying stream of [`LurkerRuntimeState`] values.
pub struct LurkerRuntimeStateStream {
    receiver: mpsc::UnboundedReceiver<LurkerRuntimeState>,
}

struct RuntimeInner {
    registry: Arc<EngineBackendProviderRegistry<EngineBackendPorts>>,
    journal: Arc<dyn EngineSwitchJournal<Selection = EngineId>>,
    expected_instance: Option<CoreInstanceId>,
    readiness_timeout: Duration,
    control: Mutex<ControlOperation>,
    state: Mutex<StateCell>,
    active: Mutex<ActiveOwner>,
    terminal_revision: AtomicU64,
}

impl RuntimeInner {
    fn state(&self) -> LurkerRuntimeState {
        lock_state(&self.state).current.clone()
    }

    fn publish(&self, state: LurkerRuntimeState) {
        let mut cell = lock_state(&self.state);
        cell.current = state.clone();
        cell.subscribers
            .retain(|subscriber| subscriber.unbounded_send(state.clone()).is_ok());
    }

    fn begin_control(&self) -> Result<ControlGuard<'_>, RuntimeError> {
        let mut operation = self.control.lock().map_err(|_| {
            RuntimeError::Port(PortError::Failed(
                "runtime control lock is poisoned".to_owned(),
            ))
        })?;
        if operation.in_flight {
            return Err(RuntimeError::ControlBusy);
        }
        operation.in_flight = true;
        drop(operation);
        Ok(ControlGuard { inner: self })
    }

    fn running_projection(&self) -> Result<(CoreScope, Option<CoreVersion>), PortError> {
        let active = lock_active(&self.active)?;
        match (&active.backend, &active.scope) {
            (Some(backend), Some(scope)) if backend.engine_id() == &scope.identity.engine => {
                Ok((scope.clone(), active.version.clone()))
            }
            (Some(_), Some(_)) => Err(PortError::Failed(
                "active engine backend and owner scope disagree on the engine".to_owned(),
            )),
            (None, _) | (_, None) => Err(PortError::Failed(
                "the running engine has no active owner scope".to_owned(),
            )),
        }
    }

    fn terminal_snapshot(&self, scope: &CoreScope) -> CoreLifecycleSnapshot {
        let revision = self.terminal_revision.fetch_add(1, Ordering::SeqCst) + 1;
        CoreLifecycleSnapshot {
            identity: Some(scope.identity.clone()),
            lifecycle: CoreLifecycle::Stopped,
            generation: scope.generation.value(),
            session_token: None,
            revision,
            failure: None,
        }
    }

    fn observed_engine(&self) -> Option<EngineId> {
        match self.state() {
            LurkerRuntimeState::Idle { selected } => selected,
            LurkerRuntimeState::Starting { engine }
            | LurkerRuntimeState::Stopping { engine, .. } => Some(engine),
            LurkerRuntimeState::Running { engine, .. } => Some(engine),
            LurkerRuntimeState::Switching { to, .. } => Some(to),
            LurkerRuntimeState::Stopped { engine, .. }
            | LurkerRuntimeState::Failed { engine, .. } => engine,
            LurkerRuntimeState::RecoveryRequired { selection, .. } => Some(selection),
        }
    }
}

struct ControlOperation {
    in_flight: bool,
}

struct ControlGuard<'a> {
    inner: &'a RuntimeInner,
}

impl Drop for ControlGuard<'_> {
    fn drop(&mut self) {
        match self.inner.control.lock() {
            Ok(mut operation) => operation.in_flight = false,
            Err(poisoned) => poisoned.into_inner().in_flight = false,
        }
    }
}

struct StateCell {
    current: LurkerRuntimeState,
    subscribers: Vec<mpsc::UnboundedSender<LurkerRuntimeState>>,
}

#[derive(Default)]
struct ActiveOwner {
    backend: Option<EngineBackendPorts>,
    scope: Option<CoreScope>,
    version: Option<CoreVersion>,
    lease: Option<Box<dyn EngineOwnerLease>>,
}

struct RuntimeScopeSource {
    scope: CoreScope,
}

impl CoreScopeSource for RuntimeScopeSource {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.scope.clone())
    }
}

fn lock_state(state: &Mutex<StateCell>) -> MutexGuard<'_, StateCell> {
    match state.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn lock_active(active: &Mutex<ActiveOwner>) -> Result<MutexGuard<'_, ActiveOwner>, PortError> {
    active
        .lock()
        .map_err(|_| PortError::Failed("runtime active owner lock is poisoned".to_owned()))
}

#[cfg(test)]
#[path = "facade_tests.rs"]
mod tests;
