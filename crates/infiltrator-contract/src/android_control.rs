//! Android product-level control intents shared by native and Bevy surfaces.
//!
//! These are user operations, not mirrored service booleans. The application
//! layer translates each intent into the existing core and VPN command
//! vocabulary.

pub mod core_launch;

use crate::command::CommandIntent;
use crate::core_identity::{CoreCommandOutcome, CoreGeneration, CoreIdentity, CoreScope};
use crate::core_process::CoreProcessStartContext;
use crate::error::{ErrorCode, Failure};
use crate::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use crate::vpn::{VpnGeneration, VpnOwnerStopReceipt};
use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

/// Process role reported by the Android host bridge.
///
/// A UI process is a client of the manifest-declared `:vpn` owner. Only
/// `CoreOwner` may construct the Android `CoreApplication`; an unknown or
/// auxiliary process fails closed as `Unsupported`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidHostProcessRole {
    UiClient,
    CoreOwner,
    Unsupported,
}

impl AndroidHostProcessRole {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::UiClient => 0,
            Self::CoreOwner => 1,
            Self::Unsupported => 2,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::UiClient),
            1 => Some(Self::CoreOwner),
            2 => Some(Self::Unsupported),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidControlIntent {
    /// Start Mihomo and then enable the Android VPN data path.
    StartProxy,
    /// Disable the VPN data path while leaving the Mihomo process available.
    StopProxy,
    /// Start Mihomo without enabling the Android VPN data path.
    ResumeKernel,
    /// Stop the VPN data path before pausing Mihomo.
    PauseKernel,
    /// Stop the VPN data path and restart Mihomo.
    RestartKernel,
}

impl AndroidControlIntent {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::StartProxy => 0,
            Self::StopProxy => 1,
            Self::ResumeKernel => 2,
            Self::PauseKernel => 3,
            Self::RestartKernel => 4,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::StartProxy),
            1 => Some(Self::StopProxy),
            2 => Some(Self::ResumeKernel),
            3 => Some(Self::PauseKernel),
            4 => Some(Self::RestartKernel),
            _ => None,
        }
    }
}

/// Core-scoped Android product controls. VPN shutdown is intentionally absent:
/// Android UI clients must stop through the VPN owner lifecycle coordinator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidProductControlIntent {
    StartProxy,
    ResumeKernel,
    PauseKernel,
    RestartKernel,
}

impl From<AndroidProductControlIntent> for AndroidControlIntent {
    fn from(intent: AndroidProductControlIntent) -> Self {
        match intent {
            AndroidProductControlIntent::StartProxy => Self::StartProxy,
            AndroidProductControlIntent::ResumeKernel => Self::ResumeKernel,
            AndroidProductControlIntent::PauseKernel => Self::PauseKernel,
            AndroidProductControlIntent::RestartKernel => Self::RestartKernel,
        }
    }
}

/// Versioned Android product command payload carried by typed owner IPC.
/// UI surfaces serialize this shared Rust enum; only the `:vpn` owner
/// deserializes and executes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidProductCommand {
    Control(AndroidProductControlIntent),
    Application(Box<CommandIntent>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidProductCommandRequest {
    pub scope: CoreScope,
    pub command: AndroidProductCommand,
}

/// Terminal command result crossing the Android UI/owner process boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidProductCommandOutcome {
    Terminal(Box<CoreCommandOutcome>),
    Unavailable {
        scope: Option<CoreScope>,
        failure: Failure,
    },
}

/// Owner-authored core lifecycle readback transported over Android IPC.
///
/// `generation`, `session_token`, and `revision` let each client reject a
/// terminal result or snapshot from an older Core session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidCoreLifecycleSnapshot {
    pub identity: Option<CoreIdentity>,
    pub lifecycle: CoreLifecycle,
    pub generation: u64,
    /// Android transports the 128-bit token as decimal text because the
    /// Kotlin/Bundle boundary only has signed 64-bit integer primitives.
    #[serde(with = "session_token_wire")]
    pub session_token: Option<SessionToken>,
    pub revision: u64,
    pub failure: Option<Failure>,
}

impl From<CoreLifecycleSnapshot> for AndroidCoreLifecycleSnapshot {
    fn from(snapshot: CoreLifecycleSnapshot) -> Self {
        Self {
            identity: snapshot.identity,
            lifecycle: snapshot.lifecycle,
            generation: snapshot.generation,
            session_token: snapshot.session_token,
            revision: snapshot.revision,
            failure: snapshot.failure,
        }
    }
}

mod session_token_wire {
    use lurker_core_contract::session::SessionToken;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(token: &Option<SessionToken>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        token
            .map(|value| value.value().to_string())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SessionToken>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                value
                    .parse::<u128>()
                    .map(SessionToken::new)
                    .map_err(D::Error::custom)
            })
            .transpose()
    }
}

impl AndroidCoreLifecycleSnapshot {
    pub fn scope(&self) -> Option<CoreScope> {
        self.identity.clone().map(|identity| CoreScope {
            identity,
            generation: CoreGeneration::from(self.generation),
            session_token: self.session_token,
        })
    }
}

/// Availability of the Core facts included in an owner-authored VPN snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnOwnerCoreReadback {
    Available(CoreLifecycleSnapshot),
    Unavailable { failure_message: String },
}

/// Owner lifecycle state for the Android VPN service and its data plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnOwnerLifecycle {
    PermissionRequired,
    StartRequested,
    Starting,
    Foreground,
    TunnelEstablished,
    Running,
    Stopping,
    Stopped,
    Revoked,
    Failed { failure_message: String },
    Unavailable { failure_message: String },
}

/// Readiness boundary reached by the owner for the current VPN generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnOwnerReadiness {
    Unavailable,
    PermissionRequired,
    AwaitingForeground,
    AwaitingTunnel,
    AwaitingDataPlane,
    Ready,
    Stopping,
    CleanupPending,
    Stopped,
    Revoked,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnOwnerResourceKind {
    PreparedConfiguration,
    Worker,
    TunnelDescriptor,
    RoutingLease,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnOwnerResourceState {
    Absent,
    Held {
        generation: u64,
    },
    CleanupPending {
        generation: u64,
        failure_message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnOwnerResourceFact {
    pub kind: VpnOwnerResourceKind,
    pub state: VpnOwnerResourceState,
}

/// One immutable readback from the `:vpn` owner containing the VPN, Core,
/// readiness and resource facts used to authorize client decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VpnOwnerSnapshot {
    pub owner_session_id: String,
    pub revision: u64,
    pub vpn_generation: VpnGeneration,
    pub vpn_state: VpnOwnerLifecycle,
    pub readiness: VpnOwnerReadiness,
    pub resources: Vec<VpnOwnerResourceFact>,
    pub foreground: bool,
    pub mtu: Option<u32>,
    pub route_count: u32,
    pub dns_servers: Vec<String>,
    pub ipv6: bool,
    pub core: VpnOwnerCoreReadback,
}

impl VpnOwnerSnapshot {
    pub fn is_running(&self) -> bool {
        self.vpn_state == VpnOwnerLifecycle::Running
            && self.readiness == VpnOwnerReadiness::Ready
            && self.foreground
            && resource_held_for_generation(self, VpnOwnerResourceKind::Worker)
            && resource_held_for_generation(self, VpnOwnerResourceKind::TunnelDescriptor)
    }

    pub fn is_stopped(&self) -> bool {
        let terminal_readiness = matches!(
            (&self.vpn_state, self.readiness),
            (VpnOwnerLifecycle::Stopped, VpnOwnerReadiness::Stopped)
                | (VpnOwnerLifecycle::Revoked, VpnOwnerReadiness::Revoked)
        );
        let expected_resources = [
            VpnOwnerResourceKind::PreparedConfiguration,
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
            VpnOwnerResourceKind::RoutingLease,
        ];
        let resources_released = expected_resources.iter().all(|kind| {
            self.resources
                .iter()
                .any(|fact| fact.kind == *kind && fact.state == VpnOwnerResourceState::Absent)
        }) && self.resources.len() == expected_resources.len();
        terminal_readiness && resources_released
    }
}

pub fn validate_android_control_owner_observation(
    snapshot: &VpnOwnerSnapshot,
) -> Result<(), Failure> {
    if snapshot.owner_session_id.trim().is_empty() || snapshot.revision == 0 {
        return Err(control_readback_failure(
            "Android VPN owner readback has no session or revision identity",
        ));
    }
    if !has_complete_vpn_resource_facts(snapshot) {
        return Err(control_readback_failure(
            "Android VPN owner readback has an incomplete or duplicated resource manifest",
        ));
    }
    match (&snapshot.vpn_state, snapshot.readiness) {
        (VpnOwnerLifecycle::Unavailable { failure_message }, _) => Err(Failure::new(
            ErrorCode::NotReady,
            failure_message.clone(),
            true,
        )),
        (_, VpnOwnerReadiness::Unavailable) => Err(Failure::new(
            ErrorCode::NotReady,
            "Android VPN owner readback is unavailable",
            true,
        )),
        _ => match &snapshot.core {
            VpnOwnerCoreReadback::Available(_) => Ok(()),
            VpnOwnerCoreReadback::Unavailable { failure_message } => Err(Failure::new(
                ErrorCode::NotReady,
                failure_message.clone(),
                true,
            )),
        },
    }
}

pub fn validate_android_control_terminal_readback(
    intent: AndroidProductControlIntent,
    before: &VpnOwnerSnapshot,
    after: &VpnOwnerSnapshot,
    command_scope: Option<&CoreScope>,
) -> Result<(), Failure> {
    validate_android_control_owner_observation(before)?;
    validate_android_control_owner_observation(after)?;
    if before.owner_session_id != after.owner_session_id {
        return Err(control_readback_failure(
            "Android VPN owner session changed while the control command was executing",
        ));
    }
    if after.revision < before.revision {
        return Err(control_readback_failure(
            "Android VPN owner returned a stale lifecycle revision after the control command",
        ));
    }
    if after.revision == before.revision && !same_owner_snapshot_facts(before, after) {
        return Err(control_readback_failure(
            "Android VPN owner changed snapshot facts without advancing its revision",
        ));
    }

    let (before_core, after_core) = match (&before.core, &after.core) {
        (
            VpnOwnerCoreReadback::Available(before_core),
            VpnOwnerCoreReadback::Available(after_core),
        ) => (before_core, after_core),
        (VpnOwnerCoreReadback::Unavailable { failure_message }, _) => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                failure_message.clone(),
                true,
            ));
        }
        (_, VpnOwnerCoreReadback::Unavailable { failure_message }) => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                failure_message.clone(),
                true,
            ));
        }
    };
    if after_core.revision < before_core.revision
        || (after_core.revision == before_core.revision && after_core != before_core)
        || after_core.generation < before_core.generation
        || after.vpn_generation < before.vpn_generation
    {
        return Err(control_readback_failure(
            "Android control readback regressed Core or VPN generation/revision facts",
        ));
    }
    if command_scope != after_core.scope().as_ref() {
        return Err(control_readback_failure(
            "Android control result and owner Core snapshot refer to different Core generations",
        ));
    }

    match intent {
        AndroidProductControlIntent::StartProxy => {
            require_usable_core(after_core, "StartProxy")?;
            if after.is_running() {
                Ok(())
            } else {
                Err(control_readback_failure(
                    "StartProxy completed without owner-confirmed VPN data-plane readiness",
                ))
            }
        }
        AndroidProductControlIntent::ResumeKernel => {
            require_usable_core(after_core, "ResumeKernel")?;
            if same_vpn_activation(before, after) {
                Ok(())
            } else {
                Err(control_readback_failure(
                    "ResumeKernel changed VPN activation while only resuming Core",
                ))
            }
        }
        AndroidProductControlIntent::PauseKernel => {
            let stopped_core = after_core.lifecycle == CoreLifecycle::Stopped
                && after_core.session_token.is_none()
                && same_core_identity_and_generation(before_core, after_core);
            if stopped_core && after.is_stopped() {
                Ok(())
            } else {
                Err(control_readback_failure(
                    "PauseKernel completed without owner-confirmed Core and VPN stopped states",
                ))
            }
        }
        AndroidProductControlIntent::RestartKernel => {
            let restarted_core = matches!(
                after_core.lifecycle,
                CoreLifecycle::Ready | CoreLifecycle::Running
            ) && after_core.session_token.is_some()
                && after_core.generation > before_core.generation;
            if restarted_core && after.is_stopped() {
                Ok(())
            } else {
                Err(control_readback_failure(
                    "RestartKernel completed without a new Core session and stopped VPN data plane",
                ))
            }
        }
    }
}

pub fn validate_android_vpn_stop_terminal_readback(
    receipt: &VpnOwnerStopReceipt,
) -> Result<(), Failure> {
    let after = match receipt {
        VpnOwnerStopReceipt::Stopped { snapshot } => snapshot,
        VpnOwnerStopReceipt::OwnerUnavailable { failure }
        | VpnOwnerStopReceipt::OwnerUnavailableWithReadback { failure, .. }
        | VpnOwnerStopReceipt::Failed { failure }
        | VpnOwnerStopReceipt::FailedWithReadback { failure, .. }
        | VpnOwnerStopReceipt::TimedOut { failure }
        | VpnOwnerStopReceipt::TimedOutWithReadback { failure, .. }
        | VpnOwnerStopReceipt::Superseded { failure }
        | VpnOwnerStopReceipt::SupersededWithReadback { failure, .. }
        | VpnOwnerStopReceipt::Canceled { failure }
        | VpnOwnerStopReceipt::CanceledWithReadback { failure, .. } => {
            return Err(failure.clone());
        }
    };
    if after.owner_session_id.trim().is_empty() || after.revision == 0 {
        return Err(control_readback_failure(
            "StopProxy returned an owner snapshot without session or revision identity",
        ));
    }
    if !after.is_stopped() {
        return Err(control_readback_failure(
            "StopProxy completed without all VPN owner resources being released",
        ));
    }
    Ok(())
}

fn resource_held_for_generation(snapshot: &VpnOwnerSnapshot, kind: VpnOwnerResourceKind) -> bool {
    let mut facts = snapshot.resources.iter().filter(|fact| fact.kind == kind);
    match (facts.next(), facts.next()) {
        (Some(fact), None) => matches!(
            &fact.state,
            VpnOwnerResourceState::Held { generation }
                if *generation == snapshot.vpn_generation.value()
        ),
        _ => false,
    }
}

fn has_complete_vpn_resource_facts(snapshot: &VpnOwnerSnapshot) -> bool {
    let expected_resources = [
        VpnOwnerResourceKind::PreparedConfiguration,
        VpnOwnerResourceKind::Worker,
        VpnOwnerResourceKind::TunnelDescriptor,
        VpnOwnerResourceKind::RoutingLease,
    ];
    snapshot.resources.len() == expected_resources.len()
        && expected_resources.iter().all(|kind| {
            snapshot
                .resources
                .iter()
                .filter(|fact| fact.kind == *kind)
                .count()
                == 1
        })
}

fn same_vpn_activation(before: &VpnOwnerSnapshot, after: &VpnOwnerSnapshot) -> bool {
    before.vpn_generation == after.vpn_generation
        && before.vpn_state == after.vpn_state
        && before.readiness == after.readiness
        && before.resources == after.resources
        && before.foreground == after.foreground
        && before.mtu == after.mtu
        && before.route_count == after.route_count
        && before.dns_servers == after.dns_servers
        && before.ipv6 == after.ipv6
}

fn same_owner_snapshot_facts(before: &VpnOwnerSnapshot, after: &VpnOwnerSnapshot) -> bool {
    same_vpn_activation(before, after) && before.core == after.core
}

fn same_core_identity_and_generation(
    before: &CoreLifecycleSnapshot,
    after: &CoreLifecycleSnapshot,
) -> bool {
    before.identity == after.identity && before.generation == after.generation
}

fn require_usable_core(snapshot: &CoreLifecycleSnapshot, intent_name: &str) -> Result<(), Failure> {
    if matches!(
        snapshot.lifecycle,
        CoreLifecycle::Ready | CoreLifecycle::Running
    ) {
        Ok(())
    } else {
        Err(control_readback_failure(&format!(
            "{intent_name} completed without owner-confirmed Core readiness",
        )))
    }
}

fn control_readback_failure(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidState, message, true)
}

/// Result of Android's system-owned `VpnService.prepare` consent flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnAuthorizationResult {
    Granted,
    Denied,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnAuthorizationState {
    Required,
    Granted,
}

/// Exact Android child identity used to distinguish a live process from PID reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidChildProcessIdentity {
    pub pid: u32,
    pub start_time_ticks: u64,
}

/// Persisted relation between one managed Core session and its Android child.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidOwnedCoreProcess {
    pub context: CoreProcessStartContext,
    pub child: AndroidChildProcessIdentity,
}

/// Kernel evidence that a previously recorded Android child has exited.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidCoreProcessExitEvidence {
    ProcessEntryMissing,
    ProcessStartIdentityChanged,
    ProcessHandleExited,
}

/// Why the Android owner could not make an exact child-process observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidCoreProcessUnavailableReason {
    LaunchReservationIncomplete,
    OwnershipRecordCorrupt,
    ProcessInspectionUnavailable,
    OwnershipRecordUnavailable,
}

/// Typed readback of Android's persisted Core child ownership record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidCoreProcessObservation {
    NeverStarted,
    Running {
        owned: AndroidOwnedCoreProcess,
    },
    Exited {
        owned: AndroidOwnedCoreProcess,
        evidence: AndroidCoreProcessExitEvidence,
    },
    Unavailable {
        reason: AndroidCoreProcessUnavailableReason,
    },
}

/// Receipt from the `:vpn` host after Rust owner recovery has committed. The
/// child record may be consumed only when it is absent or still matches the
/// exact managed context and the host confirms it has exited.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidCoreProcessRecoveryReceipt {
    Consumed,
    AlreadyAbsent,
    ContextMismatch,
    ChildStillRunning,
    ObservationUnavailable,
    NotVpnOwner,
}

impl AndroidCoreProcessRecoveryReceipt {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::Consumed => 0,
            Self::AlreadyAbsent => 1,
            Self::ContextMismatch => 2,
            Self::ChildStillRunning => 3,
            Self::ObservationUnavailable => 4,
            Self::NotVpnOwner => 5,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Consumed),
            1 => Some(Self::AlreadyAbsent),
            2 => Some(Self::ContextMismatch),
            3 => Some(Self::ChildStillRunning),
            4 => Some(Self::ObservationUnavailable),
            5 => Some(Self::NotVpnOwner),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "android_control_tests.rs"]
mod tests;
