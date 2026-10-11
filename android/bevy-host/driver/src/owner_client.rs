//! Bounded client for the Kotlin Activity's typed `:vpn` owner entry points.

use infiltrator_bevy_ui::command::{UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::surface::{SurfaceSource, overview_projection};
use infiltrator_contract::android_control::{
    AndroidControlIntent, AndroidProductCommand, AndroidProductCommandOutcome,
    AndroidProductCommandRequest, AndroidProductControlIntent, VpnOwnerCoreReadback,
    VpnOwnerSnapshot, validate_android_control_owner_observation,
    validate_android_control_terminal_readback, validate_android_vpn_stop_terminal_readback,
};
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_contract::vpn::VpnOwnerStopReceipt;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const OWNER_QUEUE_CAPACITY: usize = 16;
const OWNER_COMMAND_CAPACITY: usize = 32;
const MAX_OWNER_JSON_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy)]
struct OwnerClientPolicy {
    surface_interval: Duration,
    command_timeout: Duration,
}

impl Default for OwnerClientPolicy {
    fn default() -> Self {
        Self {
            surface_interval: Duration::from_millis(700),
            command_timeout: Duration::from_secs(10),
        }
    }
}

pub(crate) trait OwnerTransport: Send + Sync + 'static {
    fn read_surface_snapshot_json(&self) -> Result<Option<String>, Failure>;
    fn read_vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot, Failure>;
    fn execute_product_command_json(&self, request_json: &str) -> Result<String, Failure>;
    fn stop_vpn_current(&self) -> Result<VpnOwnerStopReceipt, Failure>;
}

#[derive(Clone)]
pub(crate) struct AndroidOwnerClient {
    shared: Arc<OwnerClientState>,
    requests: SyncSender<OwnerRequest>,
    policy: OwnerClientPolicy,
}

struct OwnerClientState {
    latest: RwLock<SurfaceSnapshot>,
    request_sequence: AtomicU64,
    commands: Mutex<CommandState>,
}

#[derive(Default)]
struct CommandState {
    pending: HashMap<RequestId, PendingCommand>,
    ready: VecDeque<CommandExecutedEvent>,
}

struct PendingCommand {
    command: UiCommand,
    deadline: Instant,
}

struct OwnerRequest {
    request_id: RequestId,
    operation: OwnerOperation,
}

enum OwnerOperation {
    ProductCommand {
        scope: CoreScope,
        command: AndroidProductCommand,
    },
    StopVpnCurrent,
}

enum ProductReadbackPlan {
    Application,
    Control {
        intent: AndroidProductControlIntent,
        before: VpnOwnerSnapshot,
    },
}

struct ProductCommandTerminal {
    scope: CoreScope,
    output: CommandOutput,
}

impl AndroidOwnerClient {
    #[cfg(target_os = "android")]
    pub(crate) fn start(transport: Arc<dyn OwnerTransport>) -> Result<Self, Failure> {
        Self::start_with_policy(transport, OwnerClientPolicy::default())
    }

    fn start_with_policy(
        transport: Arc<dyn OwnerTransport>,
        policy: OwnerClientPolicy,
    ) -> Result<Self, Failure> {
        let initial = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyAndroid,
            HostKind::Android,
            Failure::new(
                ErrorCode::NotReady,
                "waiting for the :vpn owner surface snapshot",
                true,
            ),
        );
        let shared = Arc::new(OwnerClientState {
            latest: RwLock::new(initial),
            request_sequence: AtomicU64::new(0),
            commands: Mutex::new(CommandState::default()),
        });
        let (requests, receiver) = sync_channel(OWNER_QUEUE_CAPACITY);
        let worker_shared = shared.clone();
        thread::Builder::new()
            .name("bevy-android-owner-ipc".to_owned())
            .spawn(move || owner_worker(receiver, worker_shared, transport, policy))
            .map_err(|error| {
                Failure::new(
                    ErrorCode::NotReady,
                    format!("could not start the Android owner IPC worker: {error}"),
                    true,
                )
            })?;
        Ok(Self {
            shared,
            requests,
            policy,
        })
    }

    fn latest_snapshot(&self) -> SurfaceSnapshot {
        self.shared
            .latest
            .read()
            .expect("Android owner snapshot lock")
            .clone()
    }

    fn submit_command(&self, command: UiCommand) -> Option<RequestId> {
        let request_id = RequestId::new(
            self.shared
                .request_sequence
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1),
        );
        let operation = match &command {
            UiCommand::StopVpn => OwnerOperation::StopVpnCurrent,
            _ => {
                let product_command = product_command(&command)?;
                let scope = self.latest_snapshot().core.scope()?;
                OwnerOperation::ProductCommand {
                    scope,
                    command: product_command,
                }
            }
        };
        let request = OwnerRequest {
            request_id,
            operation,
        };
        let mut commands = self
            .shared
            .commands
            .lock()
            .expect("Android owner command lock");
        if commands.pending.len() + commands.ready.len() >= OWNER_COMMAND_CAPACITY {
            return None;
        }
        commands.pending.insert(
            request_id,
            PendingCommand {
                command,
                deadline: Instant::now() + self.policy.command_timeout,
            },
        );
        match self.requests.try_send(request) {
            Ok(()) => Some(request_id),
            Err(TrySendError::Full(_)) => {
                commands.pending.remove(&request_id);
                None
            }
            Err(TrySendError::Disconnected(_)) => {
                if let Some(pending) = commands.pending.remove(&request_id) {
                    finish_rejected(
                        &mut commands,
                        request_id,
                        pending,
                        Failure::new(
                            ErrorCode::NotReady,
                            "the Android owner IPC worker has stopped",
                            true,
                        ),
                    );
                }
                Some(request_id)
            }
        }
    }
}

impl UiCommandSink for AndroidOwnerClient {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }

    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        self.submit_command(command)
    }

    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        expire_commands(&self.shared);
        let mut commands = self
            .shared
            .commands
            .lock()
            .expect("Android owner command lock");
        commands.ready.drain(..).collect()
    }
}

impl SurfaceSource for AndroidOwnerClient {
    fn surface_snapshot(&self) -> SurfaceSnapshot {
        self.latest_snapshot()
    }
}

impl OverviewSource for AndroidOwnerClient {
    fn current(&self) -> OverviewProjection {
        overview_projection(&self.latest_snapshot())
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

fn product_command(command: &UiCommand) -> Option<AndroidProductCommand> {
    match command {
        UiCommand::StopVpn => return None,
        _ => {}
    }
    if let Some(intent) = command.android_control_intent() {
        return Some(AndroidProductCommand::Control(match intent {
            AndroidControlIntent::StartProxy => AndroidProductControlIntent::StartProxy,
            AndroidControlIntent::ResumeKernel => AndroidProductControlIntent::ResumeKernel,
            AndroidControlIntent::PauseKernel => AndroidProductControlIntent::PauseKernel,
            AndroidControlIntent::RestartKernel => AndroidProductControlIntent::RestartKernel,
            AndroidControlIntent::StopProxy => return None,
        }));
    }
    command
        .to_intent()
        .map(|intent| AndroidProductCommand::Application(Box::new(intent)))
}

fn owner_worker(
    requests: mpsc::Receiver<OwnerRequest>,
    shared: Arc<OwnerClientState>,
    transport: Arc<dyn OwnerTransport>,
    policy: OwnerClientPolicy,
) {
    let mut next_surface_read = Instant::now();
    loop {
        if Instant::now() >= next_surface_read {
            read_owner_surface(&shared, transport.as_ref());
            next_surface_read = Instant::now() + policy.surface_interval;
        }
        let timeout = next_surface_read.saturating_duration_since(Instant::now());
        match requests.recv_timeout(timeout) {
            Ok(request) => execute_owner_command(&shared, transport.as_ref(), request),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn read_owner_surface(shared: &OwnerClientState, transport: &dyn OwnerTransport) {
    let raw = match transport.read_surface_snapshot_json() {
        Ok(Some(raw)) => raw,
        Ok(None) | Err(_) => return,
    };
    let snapshot = match parse_owner_snapshot(&raw) {
        Ok(snapshot) => snapshot,
        Err(_) => return,
    };
    let mut latest = shared.latest.write().expect("Android owner snapshot lock");
    if snapshot.is_newer_than(&latest) {
        *latest = snapshot;
    }
}

fn parse_owner_snapshot(raw: &str) -> Result<SurfaceSnapshot, Failure> {
    if raw.len() > MAX_OWNER_JSON_BYTES {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "Android owner surface snapshot exceeds the bounded payload size",
            false,
        ));
    }
    let snapshot = serde_json::from_str::<SurfaceSnapshot>(raw).map_err(|error| {
        Failure::new(
            ErrorCode::InvalidInput,
            format!("Android owner returned an invalid SurfaceSnapshot: {error}"),
            false,
        )
    })?;
    if snapshot.surface != SurfaceKind::BevyAndroid || snapshot.origin != SurfaceOrigin::Live {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "Android owner returned a non-live or wrong-surface snapshot",
            false,
        ));
    }
    if snapshot.core.scope().is_none() {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Android owner SurfaceSnapshot has no CoreScope",
            true,
        ));
    }
    Ok(snapshot)
}

fn execute_owner_command(
    shared: &OwnerClientState,
    transport: &dyn OwnerTransport,
    request: OwnerRequest,
) {
    let result = match request.operation {
        OwnerOperation::ProductCommand { scope, command } => {
            execute_product_command(transport, &scope, &command)
        }
        OwnerOperation::StopVpnCurrent => execute_vpn_stop(transport),
    };
    complete_command(shared, request.request_id, result);
}

fn execute_vpn_stop(transport: &dyn OwnerTransport) -> Result<CommandOutput, Failure> {
    let receipt = transport.stop_vpn_current()?;
    validate_android_vpn_stop_terminal_readback(&receipt)?;
    Ok(CommandOutput::Unit)
}

fn execute_product_command(
    transport: &dyn OwnerTransport,
    scope: &CoreScope,
    command: &AndroidProductCommand,
) -> Result<CommandOutput, Failure> {
    let readback_plan = match command {
        AndroidProductCommand::Control(intent) => {
            let before = transport.read_vpn_owner_snapshot()?;
            validate_android_control_owner_observation(&before)?;
            let owner_scope = match &before.core {
                VpnOwnerCoreReadback::Available(core) => core.scope(),
                VpnOwnerCoreReadback::Unavailable { failure_message } => {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        failure_message.clone(),
                        true,
                    ));
                }
            };
            if owner_scope.as_ref() != Some(scope) {
                return Err(stale_scope_failure(
                    "Android VPN owner readback belongs to a different Core scope",
                ));
            }
            ProductReadbackPlan::Control {
                intent: *intent,
                before,
            }
        }
        AndroidProductCommand::Application(_) => ProductReadbackPlan::Application,
    };
    let request_value = AndroidProductCommandRequest {
        scope: scope.clone(),
        command: command.clone(),
    };
    let terminal = serde_json::to_string(&request_value)
        .map_err(|error| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("Android product command serialization failed: {error}"),
                false,
            )
        })
        .and_then(|request_json| {
            if request_json.len() > MAX_OWNER_JSON_BYTES {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Android product command exceeds the bounded payload size",
                    false,
                ));
            }
            transport.execute_product_command_json(&request_json)
        })
        .and_then(|outcome_json| decode_product_outcome(&outcome_json, scope, command))?;
    match readback_plan {
        ProductReadbackPlan::Application => Ok(terminal.output),
        ProductReadbackPlan::Control { intent, before } => {
            if !matches!(terminal.output, CommandOutput::Unit) {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Android lifecycle control returned an unexpected produced result",
                    true,
                ));
            }
            let after = transport.read_vpn_owner_snapshot()?;
            validate_android_control_terminal_readback(
                intent,
                &before,
                &after,
                Some(&terminal.scope),
            )?;
            Ok(CommandOutput::Unit)
        }
    }
}

fn decode_product_outcome(
    raw: &str,
    request_scope: &CoreScope,
    request_command: &AndroidProductCommand,
) -> Result<ProductCommandTerminal, Failure> {
    if raw.len() > MAX_OWNER_JSON_BYTES {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "Android owner command result exceeds the bounded payload size",
            false,
        ));
    }
    let outcome = serde_json::from_str::<AndroidProductCommandOutcome>(raw).map_err(|error| {
        Failure::new(
            ErrorCode::NotReady,
            format!("Android owner returned an invalid product command result: {error}"),
            true,
        )
    })?;
    match outcome {
        AndroidProductCommandOutcome::Terminal(outcome) => {
            let outcome = *outcome;
            let Some(scope) = outcome.scope else {
                return Err(stale_scope_failure(
                    "Android owner command result has no CoreScope",
                ));
            };
            if !terminal_scope_is_current(request_scope, &scope, request_command) {
                return Err(stale_scope_failure(
                    "Android owner command result belongs to an expired CoreScope",
                ));
            }
            let output = match outcome.result {
                CommandResult::Completed { .. } => Ok(CommandOutput::Unit),
                CommandResult::Produced { output, .. } => Ok(*output),
                CommandResult::Rejected { failure, .. } => Err(failure),
                CommandResult::Accepted { .. } => Err(stale_scope_failure(
                    "Android owner returned a non-terminal command result",
                )),
            }?;
            Ok(ProductCommandTerminal { scope, output })
        }
        AndroidProductCommandOutcome::Unavailable { scope, failure } => {
            if scope.as_ref().is_some_and(|scope| {
                !terminal_scope_is_current(request_scope, scope, request_command)
            }) {
                return Err(stale_scope_failure(
                    "Android owner unavailable result belongs to an expired CoreScope",
                ));
            }
            Err(failure)
        }
    }
}

fn terminal_scope_is_current(
    request: &CoreScope,
    terminal: &CoreScope,
    command: &AndroidProductCommand,
) -> bool {
    if request.identity != terminal.identity || terminal.generation < request.generation {
        return false;
    }
    if terminal.generation > request.generation {
        return lifecycle_command(command);
    }
    if terminal.session_token == request.session_token {
        return true;
    }
    lifecycle_command(command)
        && matches!(
            (request.session_token, terminal.session_token),
            (Some(_), None) | (None, Some(_))
        )
}

fn lifecycle_command(command: &AndroidProductCommand) -> bool {
    match command {
        AndroidProductCommand::Control(_) => true,
        AndroidProductCommand::Application(intent) => matches!(
            intent.as_ref(),
            CommandIntent::StartCore | CommandIntent::StopCore | CommandIntent::RestartCore
        ),
    }
}

fn complete_command(
    shared: &OwnerClientState,
    request_id: RequestId,
    result: Result<CommandOutput, Failure>,
) {
    let mut commands = shared.commands.lock().expect("Android owner command lock");
    let Some(pending) = commands.pending.remove(&request_id) else {
        return;
    };
    commands.ready.push_back(CommandExecutedEvent {
        command: pending.command,
        request_id,
        result,
    });
}

fn expire_commands(shared: &OwnerClientState) {
    let mut commands = shared.commands.lock().expect("Android owner command lock");
    let now = Instant::now();
    let expired = commands
        .pending
        .iter()
        .filter(|(_, pending)| now >= pending.deadline)
        .map(|(request_id, _)| *request_id)
        .collect::<Vec<_>>();
    for request_id in expired {
        if let Some(pending) = commands.pending.remove(&request_id) {
            finish_rejected(
                &mut commands,
                request_id,
                pending,
                Failure::new(
                    ErrorCode::NotReady,
                    "Android owner command timed out before its terminal result",
                    true,
                ),
            );
        }
    }
}

fn finish_rejected(
    commands: &mut CommandState,
    request_id: RequestId,
    pending: PendingCommand,
    failure: Failure,
) {
    commands.ready.push_back(CommandExecutedEvent {
        command: pending.command,
        request_id,
        result: Err(failure),
    });
}

fn stale_scope_failure(message: &str) -> Failure {
    Failure::new(ErrorCode::NotReady, message, true)
}

#[cfg(test)]
#[path = "owner_client_tests.rs"]
mod tests;
