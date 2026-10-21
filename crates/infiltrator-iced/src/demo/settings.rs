//! The Settings page performs its own privilege, ACL, port and rollback gestures.
//!
//! Every gesture goes through the real surface command and the shared
//! application that owns the fact. The capture never touches the desktop
//! host adapters that would rewrite the user's own engine configuration.

use crate::demo::owner::attach_capture_owner_with_versions;
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_ports::port_conflict::PortConflictPort;
use lurker_core_contract::error::InfiltratorError;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

/// The LAN allow-list draft the capture types into the shared editor.
pub(super) const LAN_ALLOWED_IPS: &str = "10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16";

/// The isolated port-conflict owner of the capture.
///
/// The desktop host port repairs the *real* engine configuration, which a demo
/// session must never rewrite, so the capture owns an isolated port that
/// answers the same shared repair command with the same typed conflict.
struct DemoPortConflictPort;

#[async_trait::async_trait]
impl PortConflictPort for DemoPortConflictPort {
    async fn snapshot(&self) -> Result<PortConflictSnapshot, PortError> {
        Ok(occupied_mixed_port())
    }

    async fn repair(&self) -> Result<PortConflictSnapshot, PortError> {
        Ok(occupied_mixed_port())
    }
}

/// The typed conflict the isolated owner reports.
fn occupied_mixed_port() -> PortConflictSnapshot {
    PortConflictSnapshot {
        revision: 2,
        conflicts: vec![PortConflict {
            binding: PortBinding::MixedProxy,
            port: 7890,
            available: false,
            owner_pid: Some(4242),
            owner_name: Some("mihomo".to_string()),
            can_release: true,
        }],
    }
}

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::SettingsTunPermission => activate_tun_permission(state),
        FeatureId::SettingsLanSecurity => activate_lan_security(state),
        FeatureId::SettingsPortConflictRepair => activate_port_conflict(state),
        FeatureId::SettingsPermissionPanel => activate_permission_panel(state),
        FeatureId::CoreVersionRollbackConfirm => activate_core_rollback(state),
        _ => Task::none(),
    }
}

/// The host reports that the TUN service is installed without the privilege
/// the preparation flow asks for; the user then asks the host for it.
fn activate_tun_permission(state: &mut AppState) -> Task<Message> {
    state.runtime.tun_service_status = Some(TunServiceStatus::MissingPrivilege);
    // The preparation is the TUN card's own control: the host answers with
    // its typed refusal when the service cannot be prepared here.
    state.update(Message::InstallTunService)
}

/// The user edits the LAN allow list and asks the owner to apply it.
fn activate_lan_security(state: &mut AppState) -> Task<Message> {
    let edit = state.update(Message::UpdateLanAllowedIps(LAN_ALLOWED_IPS.to_string()));
    edit.chain(state.update(Message::ApplyLanSecurity))
}

/// The user asks the owner to inspect and repair the taken proxy port.
fn activate_port_conflict(_state: &mut AppState) -> Task<Message> {
    Task::perform(
        async move {
            let application = PortConflictApplication::new(Arc::new(DemoPortConflictPort));
            application
                .repair()
                .await
                .map_err(|failure| InfiltratorError::Privilege(failure.message))
        },
        Message::PortConflictsRepaired,
    )
}

/// The user runs the privileged-network regression; the host answers with its
/// typed unsupported status when no privileged adapter is composed.
fn activate_permission_panel(state: &mut AppState) -> Task<Message> {
    state.update(Message::RunPrivilegedNetworkRegression)
}

/// The owner publishes its local version-selection journal: the rollback
/// control only exists while a previous runnable core is installed, and the
/// destructive rollback is armed behind the shared confirmation surface.
fn activate_core_rollback(state: &mut AppState) -> Task<Message> {
    let mut versions = CoreVersionSnapshot::for_engine(EngineId::new("mihomo").expect("engine"));
    versions.rollback.current = Some("v1.19.12".to_string());
    versions.rollback.target = Some("v1.18.8".to_string());
    versions.rollback.history = vec!["v1.19.12".to_string(), "v1.18.8".to_string()];
    attach_capture_owner_with_versions(state, versions);
    // The user asks for the rollback: the surface arms the confirmation
    // before the destructive command is dispatched, so the capture proves the
    // confirmation step and never the rollback itself.
    assert_eq!(
        state
            .update(Message::RequestConfirmation(ConfirmAction::RollbackCore(
                "v1.18.8".to_string(),
            )))
            .units(),
        0
    );
    Task::none()
}
