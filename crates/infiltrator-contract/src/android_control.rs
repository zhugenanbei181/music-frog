//! Android product-level control intents shared by native and Bevy surfaces.
//!
//! These are user operations, not mirrored service booleans. The application
//! layer translates each intent into the existing core and VPN command
//! vocabulary.

use crate::command::CommandIntent;
use crate::core_identity::{CoreCommandOutcome, CoreGeneration, CoreIdentity, CoreScope, EngineId};
use crate::core_process::CoreProcessStartContext;
use crate::error::{ErrorCode, Failure};
use crate::profile_document_identity::{ProfileDocumentRevision, ProfileId};
use crate::session::SessionToken;
use crate::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use crate::vpn::{VpnGeneration, VpnOwnerStopReceipt};
use serde::de;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{error, fmt};

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

/// Exact profile selected by the Rust owner for one Core process launch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreLaunchConfigurationIdentity {
    pub engine_id: EngineId,
    pub profile_id: ProfileId,
    pub document_revision: ProfileDocumentRevision,
}

/// Rust-authored process launch inputs consumed by the narrow Android spawn
/// adapter. Configuration selection, path resolution and default creation
/// remain with the Rust owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreLaunchSpec {
    pub configuration_identity: CoreLaunchConfigurationIdentity,
    pub working_directory: CoreLaunchWorkingDirectory,
    #[serde(rename = "launch_arguments")]
    pub launch_parameters: CoreLaunchParameters,
}

impl CoreLaunchSpec {
    pub fn new(
        configuration_identity: CoreLaunchConfigurationIdentity,
        working_directory: PathBuf,
        launch_parameters: Vec<String>,
    ) -> Result<Self, CoreLaunchSpecValidationError> {
        Ok(Self {
            configuration_identity,
            working_directory: CoreLaunchWorkingDirectory::new(working_directory)?,
            launch_parameters: CoreLaunchParameters::new(launch_parameters)?,
        })
    }
}

/// Absolute host path supplied to Android's process spawn adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CoreLaunchWorkingDirectory(PathBuf);

impl CoreLaunchWorkingDirectory {
    pub fn new(path: PathBuf) -> Result<Self, CoreLaunchSpecValidationError> {
        if !path.is_absolute() {
            return Err(CoreLaunchSpecValidationError::RelativeWorkingDirectory);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CoreLaunchWorkingDirectory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        Self::new(path).map_err(de::Error::custom)
    }
}

/// Process arguments authored by the Rust owner and passed as distinct argv
/// elements, preserving spaces without shell parsing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CoreLaunchParameters(Vec<String>);

impl CoreLaunchParameters {
    pub fn new(values: Vec<String>) -> Result<Self, CoreLaunchSpecValidationError> {
        if values.is_empty() {
            return Err(CoreLaunchSpecValidationError::EmptyLaunchParameters);
        }
        for (index, value) in values.iter().enumerate() {
            if value.trim().is_empty() {
                return Err(CoreLaunchSpecValidationError::EmptyLaunchParameter { index });
            }
            if value.contains('\0') {
                return Err(CoreLaunchSpecValidationError::NullByteInLaunchParameter { index });
            }
        }
        Ok(Self(values))
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CoreLaunchParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        Self::new(values).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreLaunchSpecValidationError {
    RelativeWorkingDirectory,
    EmptyLaunchParameters,
    EmptyLaunchParameter { index: usize },
    NullByteInLaunchParameter { index: usize },
}

impl fmt::Display for CoreLaunchSpecValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeWorkingDirectory => {
                formatter.write_str("Core launch working directory must be absolute")
            }
            Self::EmptyLaunchParameters => {
                formatter.write_str("Core launch parameters must not be empty")
            }
            Self::EmptyLaunchParameter { index } => {
                write!(formatter, "Core launch parameter {index} must not be blank")
            }
            Self::NullByteInLaunchParameter { index } => {
                write!(
                    formatter,
                    "Core launch parameter {index} contains a null byte"
                )
            }
        }
    }
}

impl error::Error for CoreLaunchSpecValidationError {}

#[cfg(test)]
mod core_launch_spec_tests {
    use super::{
        CoreLaunchConfigurationIdentity, CoreLaunchParameters, CoreLaunchSpec,
        CoreLaunchSpecValidationError,
    };
    use crate::core_identity::EngineId;
    use crate::profile_document_identity::{ProfileDocumentRevision, ProfileId};
    use std::path::PathBuf;

    fn identity() -> CoreLaunchConfigurationIdentity {
        CoreLaunchConfigurationIdentity {
            engine_id: EngineId::new("mihomo").expect("engine ID"),
            profile_id: ProfileId::new("primary").expect("profile ID"),
            document_revision: ProfileDocumentRevision::new("sha256:abc123")
                .expect("configuration revision"),
        }
    }

    #[test]
    fn core_launch_spec_rejects_invalid_paths_and_parameters_before_wire_use() {
        assert_eq!(
            CoreLaunchSpec::new(identity(), PathBuf::from("relative"), vec!["-f".to_owned()],)
                .expect_err("relative working directory is rejected"),
            CoreLaunchSpecValidationError::RelativeWorkingDirectory,
        );
        assert_eq!(
            CoreLaunchParameters::new(Vec::new()).expect_err("empty argument vector is rejected"),
            CoreLaunchSpecValidationError::EmptyLaunchParameters,
        );
        assert_eq!(
            CoreLaunchParameters::new(vec!["-f".to_owned(), "  ".to_owned()])
                .expect_err("blank argument is rejected"),
            CoreLaunchSpecValidationError::EmptyLaunchParameter { index: 1 },
        );
        assert_eq!(
            CoreLaunchParameters::new(vec!["bad\0argument".to_owned()])
                .expect_err("NUL cannot be passed to a process"),
            CoreLaunchSpecValidationError::NullByteInLaunchParameter { index: 0 },
        );
    }

    #[test]
    fn core_launch_spec_deserialization_applies_the_same_validation() {
        let relative_directory = r#"{
            "configuration_identity": {
                "engine_id": "mihomo",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
            },
            "working_directory": "relative",
            "launch_arguments": ["-f", "profile.yaml"]
        }"#;
        assert!(serde_json::from_str::<CoreLaunchSpec>(relative_directory).is_err());

        let blank_argument = r#"{
            "configuration_identity": {
                "engine_id": "mihomo",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
            },
            "working_directory": "/data/app/files",
            "launch_arguments": ["-f", " "]
        }"#;
        assert!(serde_json::from_str::<CoreLaunchSpec>(blank_argument).is_err());
    }
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
    use crate::session::SessionToken;
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

#[cfg(test)]
mod tests {
    use super::{
        AndroidChildProcessIdentity, AndroidControlIntent, AndroidCoreLifecycleSnapshot,
        AndroidCoreProcessExitEvidence, AndroidCoreProcessObservation,
        AndroidCoreProcessUnavailableReason, AndroidHostProcessRole, AndroidOwnedCoreProcess,
        AndroidProductCommand, AndroidProductCommandOutcome, AndroidProductCommandRequest,
        AndroidProductControlIntent, CoreLaunchConfigurationIdentity, CoreLaunchSpec,
        VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact,
        VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
        validate_android_control_owner_observation, validate_android_control_terminal_readback,
    };
    use crate::command::{CommandIntent, CommandResult, RequestId};
    use crate::core_identity::{
        CoreCommandOutcome, CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use crate::error::{ErrorCode, Failure};
    use crate::profile_document_identity::{ProfileDocumentRevision, ProfileId};
    use crate::session::SessionToken;
    use crate::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
    use std::path::{Path, PathBuf};

    #[test]
    fn core_launch_spec_round_trips_owner_selected_profile_and_process_arguments() {
        let spec = CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id: EngineId::new("mihomo").expect("engine ID"),
                profile_id: ProfileId::new("primary").expect("profile ID"),
                document_revision: ProfileDocumentRevision::new("sha256:abc123")
                    .expect("configuration revision"),
            },
            PathBuf::from("/data/app/files"),
            vec![
                "-d".to_owned(),
                "/data/app/files".to_owned(),
                "-f".to_owned(),
                "/data/app/files/configs/engines/mihomo/primary.yaml".to_owned(),
            ],
        )
        .expect("launch spec inputs are valid");

        let encoded = serde_json::to_string(&spec).expect("serialize launch spec");
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("parse launch spec");
        assert_eq!(value["configuration_identity"]["engine_id"], "mihomo");
        assert_eq!(value["configuration_identity"]["profile_id"], "primary");
        assert_eq!(
            value["configuration_identity"]["document_revision"],
            "sha256:abc123"
        );
        assert_eq!(value["working_directory"], "/data/app/files");
        assert_eq!(value["launch_arguments"][0], "-d");
        assert_eq!(
            spec.working_directory.as_path(),
            Path::new("/data/app/files")
        );
        assert_eq!(spec.launch_parameters.as_slice()[0], "-d");
        assert_eq!(
            serde_json::from_str::<CoreLaunchSpec>(&encoded).expect("deserialize launch spec"),
            spec,
        );
    }

    #[test]
    fn android_lifecycle_snapshot_uses_decimal_text_for_full_width_session_tokens() {
        let snapshot = AndroidCoreLifecycleSnapshot {
            identity: Some(CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("android-active").expect("instance"),
                None,
            )),
            lifecycle: CoreLifecycle::Running,
            generation: 4,
            session_token: Some(SessionToken::new(u128::MAX)),
            revision: 9,
            failure: None,
        };

        let encoded = serde_json::to_string(&snapshot).expect("serialize lifecycle snapshot");
        assert!(encoded.contains("\"session_token\":\"340282366920938463463374607431768211455\""));
        assert_eq!(
            serde_json::from_str::<AndroidCoreLifecycleSnapshot>(&encoded)
                .expect("deserialize decimal session token"),
            snapshot
        );
    }

    #[test]
    fn control_intent_wire_codes_round_trip_and_reject_unknown_values() {
        let intents = [
            AndroidControlIntent::StartProxy,
            AndroidControlIntent::StopProxy,
            AndroidControlIntent::ResumeKernel,
            AndroidControlIntent::PauseKernel,
            AndroidControlIntent::RestartKernel,
        ];

        for intent in intents {
            assert_eq!(
                AndroidControlIntent::from_wire_code(intent.wire_code()),
                Some(intent)
            );
        }
        assert_eq!(AndroidControlIntent::from_wire_code(-1), None);
        for old_code in 5..=11 {
            assert_eq!(AndroidControlIntent::from_wire_code(old_code), None);
        }
    }

    #[test]
    fn android_product_commands_and_results_round_trip_as_typed_json() {
        let scope = CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("android-active").expect("instance"),
                None,
            ),
            generation: CoreGeneration::new(4),
            session_token: None,
        };
        let command = AndroidProductCommandRequest {
            scope: scope.clone(),
            command: AndroidProductCommand::Application(Box::new(CommandIntent::StartProxy)),
        };
        let encoded = serde_json::to_string(&command).expect("serialize command");
        assert_eq!(
            serde_json::from_str::<AndroidProductCommandRequest>(&encoded)
                .expect("deserialize command"),
            command
        );

        let control_command = AndroidProductCommandRequest {
            scope: scope.clone(),
            command: AndroidProductCommand::Control(AndroidProductControlIntent::ResumeKernel),
        };
        let encoded = serde_json::to_string(&control_command).expect("serialize control command");
        assert_eq!(
            serde_json::from_str::<AndroidProductCommandRequest>(&encoded)
                .expect("deserialize control command"),
            control_command
        );

        let legacy_stop = format!(
            "{{\"scope\":{},\"command\":{{\"Control\":\"StopProxy\"}}}}",
            serde_json::to_string(&scope).expect("serialize scope")
        );
        assert!(serde_json::from_str::<AndroidProductCommandRequest>(&legacy_stop).is_err());

        let outcome = AndroidProductCommandOutcome::Terminal(Box::new(CoreCommandOutcome {
            scope: Some(scope),
            result: CommandResult::Completed {
                request_id: RequestId(17),
            },
        }));
        let encoded = serde_json::to_string(&outcome).expect("serialize result");
        assert_eq!(
            serde_json::from_str::<AndroidProductCommandOutcome>(&encoded)
                .expect("deserialize result"),
            outcome
        );

        let unavailable = AndroidProductCommandOutcome::Unavailable {
            scope: None,
            failure: Failure::new(ErrorCode::NotReady, "owner unavailable", true),
        };
        let encoded = serde_json::to_string(&unavailable).expect("serialize failure");
        assert_eq!(
            serde_json::from_str::<AndroidProductCommandOutcome>(&encoded)
                .expect("deserialize failure"),
            unavailable
        );
    }

    #[test]
    fn typed_core_process_observations_round_trip_exact_child_facts() {
        let owned = AndroidOwnedCoreProcess {
            context: crate::core_process::CoreProcessStartContext::managed(
                CoreIdentity::new(
                    EngineId::new("mihomo").expect("engine ID"),
                    CoreInstanceId::new("android-active").expect("instance ID"),
                    None,
                ),
                CoreGeneration::new(7),
                crate::session::SessionToken::new(42),
            ),
            child: AndroidChildProcessIdentity {
                pid: 321,
                start_time_ticks: 987_654,
            },
        };
        let observations = [
            AndroidCoreProcessObservation::NeverStarted,
            AndroidCoreProcessObservation::Running {
                owned: owned.clone(),
            },
            AndroidCoreProcessObservation::Exited {
                owned,
                evidence: AndroidCoreProcessExitEvidence::ProcessStartIdentityChanged,
            },
            AndroidCoreProcessObservation::Unavailable {
                reason: AndroidCoreProcessUnavailableReason::OwnershipRecordCorrupt,
            },
        ];

        for observation in observations {
            let encoded = serde_json::to_string(&observation).expect("serialize observation");
            assert_eq!(
                serde_json::from_str::<AndroidCoreProcessObservation>(&encoded)
                    .expect("deserialize observation"),
                observation,
            );
        }
    }

    #[test]
    fn android_host_process_role_wire_codes_are_typed_and_fail_closed() {
        let roles = [
            AndroidHostProcessRole::UiClient,
            AndroidHostProcessRole::CoreOwner,
            AndroidHostProcessRole::Unsupported,
        ];

        for role in roles {
            assert_eq!(
                AndroidHostProcessRole::from_wire_code(role.wire_code()),
                Some(role)
            );
        }
        assert_eq!(AndroidHostProcessRole::from_wire_code(-1), None);
        assert_eq!(AndroidHostProcessRole::from_wire_code(3), None);
    }

    #[test]
    fn vpn_owner_snapshot_round_trips_core_identity_resources_and_decimal_session_token() {
        let snapshot = VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 14,
            vpn_generation: crate::vpn::VpnGeneration::new(9),
            vpn_state: VpnOwnerLifecycle::Running,
            readiness: VpnOwnerReadiness::Ready,
            resources: [
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: VpnOwnerResourceState::Held { generation: 9 },
            })
            .collect(),
            foreground: true,
            mtu: Some(1500),
            route_count: 3,
            dns_servers: vec!["1.1.1.1".to_owned()],
            ipv6: true,
            core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
                identity: Some(CoreIdentity::new(
                    EngineId::new("mihomo").expect("engine"),
                    CoreInstanceId::new("android-active").expect("instance"),
                    None,
                )),
                lifecycle: CoreLifecycle::Running,
                generation: 21,
                session_token: Some(SessionToken::new(u128::MAX)),
                revision: 32,
                failure: None,
            }),
        };

        let encoded = serde_json::to_string(&snapshot).expect("serialize owner snapshot");
        assert!(encoded.contains("\"session_token\":\"340282366920938463463374607431768211455\""));
        assert_eq!(
            serde_json::from_str::<VpnOwnerSnapshot>(&encoded).expect("deserialize owner snapshot"),
            snapshot
        );
        assert!(snapshot.is_running());
    }

    #[test]
    fn vpn_owner_snapshot_preserves_failed_core_failure_facts() {
        let failure = Failure::new(
            ErrorCode::Configuration,
            "profile YAML could not be parsed",
            true,
        )
        .with_reason(crate::error::FailureReason::YamlSyntax {
            line: 23,
            column: 6,
        });
        let core = CoreLifecycleSnapshot {
            identity: Some(CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("android-active").expect("instance"),
                Some(crate::core_identity::CoreVersion::new("1.20.0").expect("version")),
            )),
            lifecycle: CoreLifecycle::Failed,
            generation: 21,
            session_token: Some(SessionToken::new(u128::MAX)),
            revision: 33,
            failure: Some(failure.clone()),
        };
        let snapshot = VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 14,
            vpn_generation: crate::vpn::VpnGeneration::new(9),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: [
                VpnOwnerResourceKind::PreparedConfiguration,
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
                VpnOwnerResourceKind::RoutingLease,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: VpnOwnerResourceState::Absent,
            })
            .collect(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(core.clone()),
        };

        let encoded = serde_json::to_string(&snapshot).expect("serialize owner snapshot");
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("owner JSON parses");
        assert_eq!(
            value["core"]["Available"]["failure"],
            serde_json::to_value(&failure).expect("typed failure serializes"),
        );
        let decoded =
            serde_json::from_str::<VpnOwnerSnapshot>(&encoded).expect("deserialize owner snapshot");

        assert_eq!(decoded.core, VpnOwnerCoreReadback::Available(core));
        let VpnOwnerCoreReadback::Available(decoded_core) = decoded.core else {
            panic!("failed Core snapshot facts must remain Available")
        };
        assert_eq!(decoded_core.lifecycle, CoreLifecycle::Failed);
        assert_eq!(decoded_core.failure, Some(failure));
        assert_eq!(
            decoded_core.session_token,
            Some(SessionToken::new(u128::MAX))
        );
    }

    #[test]
    fn stop_terminal_requires_typed_terminal_readiness_and_every_resource_absent() {
        let mut snapshot = VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 4,
            vpn_generation: crate::vpn::VpnGeneration::new(3),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: Vec::new(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Unavailable {
                failure_message: "Core readback unavailable".to_owned(),
            },
        };
        assert!(!snapshot.is_stopped());

        snapshot.resources = [
            VpnOwnerResourceKind::PreparedConfiguration,
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
            VpnOwnerResourceKind::RoutingLease,
        ]
        .into_iter()
        .map(|kind| VpnOwnerResourceFact {
            kind,
            state: VpnOwnerResourceState::Absent,
        })
        .collect();
        assert!(snapshot.is_stopped());

        snapshot.resources[1].state = VpnOwnerResourceState::CleanupPending {
            generation: 3,
            failure_message: "worker stop is pending".to_owned(),
        };
        snapshot.readiness = VpnOwnerReadiness::CleanupPending;
        assert!(!snapshot.is_stopped());
    }

    #[test]
    fn control_owner_observation_requires_session_revision_and_complete_resources() {
        let mut snapshot = VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 1,
            vpn_generation: crate::vpn::VpnGeneration::new(3),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: [
                VpnOwnerResourceKind::PreparedConfiguration,
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
                VpnOwnerResourceKind::RoutingLease,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: VpnOwnerResourceState::Absent,
            })
            .collect(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
                identity: Some(CoreIdentity::new(
                    EngineId::new("mihomo").expect("engine"),
                    CoreInstanceId::new("android-active").expect("instance"),
                    None,
                )),
                lifecycle: CoreLifecycle::Stopped,
                generation: 3,
                session_token: None,
                revision: 9,
                failure: None,
            }),
        };
        assert_eq!(
            validate_android_control_owner_observation(&snapshot),
            Ok(())
        );

        let before = snapshot.clone();
        let mut after = before.clone();
        if let VpnOwnerCoreReadback::Available(core) = &mut after.core {
            core.revision += 1;
        }
        let terminal_scope = match &after.core {
            VpnOwnerCoreReadback::Available(core) => core.scope(),
            VpnOwnerCoreReadback::Unavailable { .. } => None,
        };
        let failure = validate_android_control_terminal_readback(
            AndroidProductControlIntent::ResumeKernel,
            &before,
            &after,
            terminal_scope.as_ref(),
        )
        .expect_err("Core facts in an owner snapshot must advance the owner revision");
        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(failure.message.contains("without advancing its revision"));

        let mut before = snapshot.clone();
        before.revision = 4;

        let mut stale_owner_revision = before.clone();
        stale_owner_revision.revision = 3;
        assert!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::ResumeKernel,
                &before,
                &stale_owner_revision,
                terminal_scope.as_ref(),
            )
            .expect_err("owner revision cannot regress")
            .message
            .contains("stale lifecycle revision")
        );

        let mut stale_vpn_generation = before.clone();
        stale_vpn_generation.revision = 5;
        stale_vpn_generation.vpn_generation = crate::vpn::VpnGeneration::new(2);
        assert!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::ResumeKernel,
                &before,
                &stale_vpn_generation,
                terminal_scope.as_ref(),
            )
            .expect_err("VPN generation cannot regress")
            .message
            .contains("regressed Core or VPN generation/revision facts")
        );

        let mut stale_core_generation = before.clone();
        stale_core_generation.revision = 5;
        if let VpnOwnerCoreReadback::Available(core) = &mut stale_core_generation.core {
            core.generation = 2;
        }
        let stale_core_scope = match &stale_core_generation.core {
            VpnOwnerCoreReadback::Available(core) => core.scope(),
            VpnOwnerCoreReadback::Unavailable { .. } => None,
        };
        assert!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::ResumeKernel,
                &before,
                &stale_core_generation,
                stale_core_scope.as_ref(),
            )
            .expect_err("Core generation cannot regress")
            .message
            .contains("regressed Core or VPN generation/revision facts")
        );

        let mut stale_core_revision = before.clone();
        stale_core_revision.revision = 5;
        if let VpnOwnerCoreReadback::Available(core) = &mut stale_core_revision.core {
            core.revision = 8;
        }
        assert!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::ResumeKernel,
                &before,
                &stale_core_revision,
                terminal_scope.as_ref(),
            )
            .expect_err("Core revision cannot regress")
            .message
            .contains("regressed Core or VPN generation/revision facts")
        );

        snapshot.owner_session_id.clear();
        assert_eq!(
            validate_android_control_owner_observation(&snapshot)
                .expect_err("readback without an owner session is stale")
                .code,
            ErrorCode::InvalidState,
        );
        snapshot.owner_session_id = "owner-session".to_owned();

        snapshot.revision = 0;
        assert_eq!(
            validate_android_control_owner_observation(&snapshot)
                .expect_err("readback without a revision is stale")
                .code,
            ErrorCode::InvalidState,
        );
        snapshot.revision = 1;

        snapshot.resources.pop();
        assert_eq!(
            validate_android_control_owner_observation(&snapshot)
                .expect_err("readback without every resource fact is incomplete")
                .code,
            ErrorCode::InvalidState,
        );
    }
}
