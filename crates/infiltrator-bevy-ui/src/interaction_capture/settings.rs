//! The Settings page performs its own privilege, port and LAN gestures.
//!
//! Every gesture goes through the page's own control and the shared owner
//! that holds the fact. The capture publishes the owner's real readback, the
//! page's own control answers it, and the receipt measures the copy the page
//! really renders. The capture never touches the desktop host adapters that
//! would rewrite the user's own engine configuration.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::capture::{CaptureInteractionCommitted, CaptureInteractionGate};
use crate::command::CommandSinkHandle;
use crate::pages::settings::settings_core::{SettingsLine, SettingsLineKind};
use crate::pages::settings::settings_dialogs::{
    CoreRollbackConfirmButton, CoreRollbackDialog, CoreRollbackDialogMessage,
};
use crate::pages::settings::settings_lan::LanSharingToggle;
use crate::pages::settings::settings_port_conflict::PortConflictOutcomeText;
use crate::pages::settings::settings_privileged_network::{
    PrivilegedNetworkRunButton, PrivilegedNetworkStatusLine,
};
use crate::pages::settings::{
    CoreRollbackButton, PortConflictButton, PrepareTunPermissionButton, SettingsPageRoot,
};
use crate::route::surface_projection::replay_surface_snapshot;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::privileged_network_application::PrivilegedNetworkApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::{ModalStage, ModalState};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::service_mode::{ServiceModePlatform, ServiceModeState};
use infiltrator_presentation::host_network_projection::privileged;
use infiltrator_presentation::settings_status_projection::format_service_mode;
use infiltrator_shared::locales::Lang;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_test_support::port_conflict_fixtures::PortConflictCapturePort;
use infiltrator_test_support::privileged_network_fixtures::PrivilegedNetworkCapturePort;
use infiltrator_test_support::service_mode_fixtures::ServiceModeCapturePort;
use infiltrator_test_support::version_fixtures::{CAPTURE_ENGINE, VersionCapturePort};
use lurker_core_contract::identity::EngineId;
use std::future::Future;
use std::sync::{Arc, Mutex};

/// The capture's own progress through one settings gesture.
///
/// The gesture is a typed sequence: the owner's facts reach the surface, the
/// page's own control answers them, and the owner's answer comes back. Every
/// step is one state, so the capture can never be "half way" without saying
/// which half.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsStage {
    /// The owner's facts are not on the surface yet.
    #[default]
    Unpublished,
    /// The owner's facts are on the surface; the page's own control is ready.
    Published,
    /// The page's own control was activated; the owner's answer is awaited.
    Activated,
}

/// The rollback capture has its own typed sequence because the review must
/// remain visible for the screenshot before the destructive confirmation is
/// submitted and read back from the version owner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum RollbackStage {
    /// The version owner's journal is not on the surface yet.
    #[default]
    Unpublished,
    /// The owner-backed rollback control is ready for its first activation.
    Published,
    /// The review is open; a real capture defers confirmation until after the
    /// screenshot has had time to capture the visible review state.
    ConfirmationOpen { frames_until_confirm: u16 },
    /// The confirmation was submitted exactly once and its owner readback is
    /// awaited.
    Confirmed {
        expected_target: String,
        remaining_frames: u16,
    },
    /// The owner reports the selected target as the active version.
    ReadbackVerified,
}

/// The isolated owners the settings capture really drives.
#[derive(Resource)]
struct SettingsCapture {
    stage: SettingsStage,
    rollback_stage: RollbackStage,
    port_conflicts: Arc<PortConflictApplication>,
    privileged: Arc<PrivilegedNetworkApplication>,
    service_mode: Arc<ServiceModeApplication>,
    versions: VersionApplication,
}

pub fn install(app: &mut App, feature: FeatureId) {
    let port_conflicts = Arc::new(PortConflictApplication::new(Arc::new(
        PortConflictCapturePort::default(),
    )));
    let privileged = Arc::new(PrivilegedNetworkApplication::new(Arc::new(
        PrivilegedNetworkCapturePort::default(),
    )));
    // The service-mode fixture owns one real privilege transition: the
    // preparation moves an uninstalled service to an installed, stopped one.
    let service_mode = Arc::new(ServiceModeApplication::new(Arc::new(
        ServiceModeCapturePort::new(
            ServiceModePlatform::WindowsService,
            ServiceModeState::NotInstalled,
        ),
    )));
    let versions = VersionApplication::new(
        EngineId::new(CAPTURE_ENGINE).expect("capture version engine ID is valid"),
        Arc::new(VersionCapturePort::default()),
    )
    .expect("capture version owner matches its engine");
    let application = capture_command_application()
        .with_port_conflicts(port_conflicts.as_ref().clone())
        .with_privileged_network(privileged.as_ref().clone())
        .with_service_mode(service_mode.as_ref().clone())
        .with_versions(versions.clone());
    let sink = if feature == FeatureId::CoreVersionRollbackConfirm {
        // `RollbackCore` is a Core-backend command. Drive it through the same
        // validated-scope route as the product surface instead of bypassing
        // the capability gate with an unscoped handler.
        let process = Arc::new(LanguageCaptureProcess);
        let core = Arc::new(CoreApplication::new(
            process.clone(),
            process,
            tokio_application_runtime().expect("isolated settings capture runtime"),
        ));
        CaptureCommandSink::scoped(core, application, CaptureCapability::Settings)
    } else {
        CaptureCommandSink::new(application, CaptureCapability::Settings)
    };
    app.insert_resource(CommandSinkHandle(Arc::new(sink)));
    app.insert_resource(SettingsCapture {
        stage: SettingsStage::Unpublished,
        rollback_stage: RollbackStage::Unpublished,
        port_conflicts,
        privileged,
        service_mode,
        versions,
    });
}

/// The page's own controls the settings gestures drive.
#[derive(SystemParam)]
pub struct SettingsControls<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, SettingsCapture>,
    capture_gate: Option<Res<'w, CaptureInteractionGate>>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    repairs: Query<'w, 's, Entity, With<PortConflictButton>>,
    prepares: Query<'w, 's, Entity, With<PrepareTunPermissionButton>>,
    regressions: Query<'w, 's, Entity, With<PrivilegedNetworkRunButton>>,
    rollbacks: Query<'w, 's, Entity, With<CoreRollbackButton>>,
    confirms: Query<'w, 's, Entity, With<CoreRollbackConfirmButton>>,
    rollback_dialogs: Query<'w, 's, &'static ModalState, With<CoreRollbackDialog>>,
    rollback_messages: Query<'w, 's, &'static Text, With<CoreRollbackDialogMessage>>,
}

/// Drive the Settings page's own privilege, port and regression gestures.
pub fn activate(mut controls: SettingsControls, mut commands: Commands) {
    if controls.route.0 != Some(Route::Settings) {
        return;
    }
    match controls.feature.feature {
        FeatureId::CoreVersionRollbackConfirm => {
            if !selected(
                &controls.feature,
                &controls.route,
                FeatureId::CoreVersionRollbackConfirm,
                Route::Settings,
            ) {
                return;
            }
            activate_core_rollback(&mut controls, &mut commands);
        }
        FeatureId::SettingsLanSecurity => {
            controls.capture.stage = SettingsStage::Activated;
        }
        FeatureId::SettingsPortConflictRepair => {
            if !selected(
                &controls.feature,
                &controls.route,
                FeatureId::SettingsPortConflictRepair,
                Route::Settings,
            ) {
                return;
            }
            activate_port_conflict(&mut controls, &mut commands);
        }
        FeatureId::SettingsTunPermission => {
            if !selected(
                &controls.feature,
                &controls.route,
                FeatureId::SettingsTunPermission,
                Route::Settings,
            ) {
                return;
            }
            activate_tun_permission(&mut controls, &mut commands);
        }
        FeatureId::SettingsPermissionPanel => {
            if !selected(
                &controls.feature,
                &controls.route,
                FeatureId::SettingsPermissionPanel,
                Route::Settings,
            ) {
                return;
            }
            activate_privileged_network(&mut controls, &mut commands);
        }
        _ => {}
    }
}

fn activate_core_rollback(controls: &mut SettingsControls<'_, '_>, commands: &mut Commands) {
    match controls.capture.rollback_stage.clone() {
        RollbackStage::Unpublished => {
            let owner = controls.capture.versions.clone();
            let snapshot = block_on(async move { owner.probe_channels().await });
            assert!(
                snapshot.rollback.current.is_some() && snapshot.rollback.target.is_some(),
                "the rollback capture requires an installed current/target version"
            );
            controls.latest.0.versions = snapshot;
            publish(controls, commands);
            controls.capture.rollback_stage = RollbackStage::Published;
        }
        RollbackStage::Published => {
            let Some(entity) = controls.rollbacks.iter().next() else {
                return;
            };
            commands.trigger(Activate { entity });
            let frames_until_confirm = controls.capture_gate.as_ref().map_or(0, |_| 180);
            controls.capture.rollback_stage = RollbackStage::ConfirmationOpen {
                frames_until_confirm,
            };
        }
        RollbackStage::ConfirmationOpen {
            frames_until_confirm,
        } => {
            if frames_until_confirm > 0 {
                controls.capture.rollback_stage = RollbackStage::ConfirmationOpen {
                    frames_until_confirm: frames_until_confirm - 1,
                };
                return;
            }
            let Ok(dialog) = controls.rollback_dialogs.single() else {
                return;
            };
            if dialog.stage != ModalStage::Open {
                return;
            }
            let Some(target) = controls.latest.0.versions.rollback.target.clone() else {
                return;
            };
            let current = controls
                .latest
                .0
                .versions
                .rollback
                .current
                .as_deref()
                .unwrap_or("?");
            let Ok(message) = controls.rollback_messages.single() else {
                return;
            };
            assert_eq!(
                message.0,
                format!("{current} -> {target}"),
                "the review must render the version owner's current/target readback"
            );
            let Some(confirm) = controls.confirms.iter().next() else {
                return;
            };
            commands.trigger(Activate { entity: confirm });
            controls.capture.rollback_stage = RollbackStage::Confirmed {
                expected_target: target,
                remaining_frames: 240,
            };
        }
        RollbackStage::Confirmed {
            expected_target,
            remaining_frames,
        } => {
            let owner = controls.capture.versions.clone();
            let snapshot = block_on(async move { owner.probe_channels().await });
            if snapshot.rollback.current.as_deref() != Some(expected_target.as_str()) {
                assert!(
                    remaining_frames > 0,
                    "the version owner did not publish the selected rollback target"
                );
                controls.capture.rollback_stage = RollbackStage::Confirmed {
                    expected_target,
                    remaining_frames: remaining_frames - 1,
                };
                return;
            }
            controls.latest.0.versions = snapshot;
            publish(controls, commands);
            controls.capture.rollback_stage = RollbackStage::ReadbackVerified;
            commands.trigger(CaptureInteractionCommitted {
                scenario: "core-version-rollback-confirm",
                readback: expected_target,
            });
        }
        RollbackStage::ReadbackVerified => {}
    }
}

/// The port-conflict repair: the owner publishes the taken port, the page's
/// own repair control asks for the safe repair, and the page renders the
/// owner's answer to it.
fn activate_port_conflict(controls: &mut SettingsControls<'_, '_>, commands: &mut Commands) {
    match controls.capture.stage {
        SettingsStage::Unpublished => {
            let owner = controls.capture.port_conflicts.clone();
            let snapshot = block_on(async move { owner.snapshot().await })
                .expect("the capture's own port-conflict owner answers");
            controls.latest.0.port_conflicts = snapshot;
            publish(controls, commands);
            controls.capture.stage = SettingsStage::Published;
        }
        SettingsStage::Published => {
            let Some(entity) = controls.repairs.iter().next() else {
                return;
            };
            controls.capture.stage = SettingsStage::Activated;
            commands.trigger(Activate { entity });
        }
        SettingsStage::Activated => {}
    }
}

/// The TUN privilege preparation: the owner publishes an uninstalled service,
/// the page's own preparation control asks the host for it, and the owner's
/// answer moves the service to its installed, stopped state.
fn activate_tun_permission(controls: &mut SettingsControls<'_, '_>, commands: &mut Commands) {
    match controls.capture.stage {
        SettingsStage::Unpublished => {
            let owner = controls.capture.service_mode.clone();
            let snapshot = block_on(async move { owner.snapshot().await })
                .expect("the capture's own service-mode owner answers");
            controls.latest.0.service_mode = snapshot;
            publish(controls, commands);
            controls.capture.stage = SettingsStage::Published;
        }
        SettingsStage::Published => {
            let Some(entity) = controls.prepares.iter().next() else {
                return;
            };
            controls.capture.stage = SettingsStage::Activated;
            commands.trigger(Activate { entity });
        }
        SettingsStage::Activated => {
            // The owner's answer is the state the host really left behind.
            let owner = controls.capture.service_mode.clone();
            let snapshot = block_on(async move { owner.snapshot().await })
                .expect("the capture's own service-mode owner answers");
            if snapshot.state != controls.latest.0.service_mode.state {
                controls.latest.0.service_mode = snapshot;
                publish(controls, commands);
            }
        }
    }
}

/// The privileged-network regression: the owner publishes the idle state, the
/// page's own control runs the transaction, and the owner's readback shows
/// what the transaction really did.
fn activate_privileged_network(controls: &mut SettingsControls<'_, '_>, commands: &mut Commands) {
    match controls.capture.stage {
        SettingsStage::Unpublished => {
            let owner = controls.capture.privileged.clone();
            let snapshot = block_on(async move { owner.snapshot().await });
            controls.latest.0.privileged_network = snapshot;
            publish(controls, commands);
            controls.capture.stage = SettingsStage::Published;
        }
        SettingsStage::Published => {
            let Some(entity) = controls.regressions.iter().next() else {
                return;
            };
            controls.capture.stage = SettingsStage::Activated;
            commands.trigger(Activate { entity });
        }
        SettingsStage::Activated => {
            let owner = controls.capture.privileged.clone();
            let snapshot = block_on(async move { owner.snapshot().await });
            if snapshot.state != controls.latest.0.privileged_network.state {
                controls.latest.0.privileged_network = snapshot;
                publish(controls, commands);
            }
        }
    }
}

/// Publish the capture's own snapshot to the surface.
fn publish(controls: &mut SettingsControls<'_, '_>, commands: &mut Commands) {
    controls.latest.0.revision += 1;
    replay_surface_snapshot(controls.latest.0.clone(), commands);
}

/// The Settings page's own outcome copy.
#[derive(SystemParam)]
pub struct SettingsObservation<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: Res<'w, SettingsCapture>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    locale: Res<'w, UiLocale>,
    outcomes: Query<'w, 's, (Entity, &'static Text), With<PortConflictOutcomeText>>,
    regressions: Query<'w, 's, (Entity, &'static Text), With<PrivilegedNetworkStatusLine>>,
    service: Query<'w, 's, (Entity, &'static Text, &'static SettingsLine)>,
    rollback_dialogs: Query<'w, 's, &'static ModalState, With<CoreRollbackDialog>>,
    rollback_messages: Query<'w, 's, (Entity, &'static Text), With<CoreRollbackDialogMessage>>,
    lan_toggles: Query<'w, 's, Entity, With<LanSharingToggle>>,
    pages: Query<'w, 's, Entity, With<SettingsPageRoot>>,
}

/// Observe the copy the Settings page really renders for the business.
pub fn observe(
    controls: SettingsObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    if controls.route.0 != Some(Route::Settings) {
        return;
    }
    let language = controls.locale.code().to_string();
    let catalog = Lang(&language);
    let target = state.feature;
    match target {
        FeatureId::CoreVersionRollbackConfirm => {
            if !matches!(
                controls.capture.rollback_stage,
                RollbackStage::ConfirmationOpen { .. }
            ) {
                return;
            }
        }
        _ if controls.capture.stage != SettingsStage::Activated => return,
        _ => {}
    }
    // The page's own copy is the receipt's subject; the line itself is
    // measured only once it is really rendered.
    let subject = (|| {
        match target {
            FeatureId::CoreVersionRollbackConfirm => {
                let dialog = controls.rollback_dialogs.single().ok()?;
                if dialog.stage != ModalStage::Open {
                    return None;
                }
                let (entity, text) = controls.rollback_messages.single().ok()?;
                if text.0.is_empty() {
                    return None;
                }
                Some((entity, "core rollback confirmation"))
            }
            FeatureId::SettingsLanSecurity => {
                let entity = controls.lan_toggles.iter().next()?;
                Some((entity, "lan sharing toggle"))
            }
            FeatureId::SettingsPortConflictRepair => {
                let (entity, text) = controls.outcomes.single().ok()?;
                if text.0.is_empty() {
                    return None;
                }
                Some((entity, "port conflict repair outcome"))
            }
            FeatureId::SettingsTunPermission => {
                // The page replays the owner's own state, so the capture asks
                // the shared copy for the exact text the page must show
                // instead of pinning one language of its own.
                let expected = format_service_mode(&controls.latest.0.service_mode, &catalog);
                let (entity, text, _) = controls
                    .service
                    .iter()
                    .find(|(_, _text, line)| line.0 == SettingsLineKind::ServiceMode)?;
                if text.0 != expected {
                    return None;
                }
                Some((entity, "service mode privilege state"))
            }
            FeatureId::SettingsPermissionPanel => {
                let view = privileged(&controls.latest.0.privileged_network, &catalog);
                let expected = format!("{} · {}", view.status, view.details);
                let (entity, text) = controls.regressions.single().ok()?;
                if text.0 != expected {
                    return None;
                }
                Some((entity, "privileged network regression outcome"))
            }
            _ => None,
        }
    })();
    let Some((entity, label)) = subject else {
        return;
    };
    // The outcome line sits below the fold of the Settings page, so the
    // capture scrolls the page's own viewport until the line is really
    // rendered, and measures it on the next frame.
    if !geometry.unclipped(entity) {
        if let Ok(page) = controls.pages.single()
            && let Some(viewport) = geometry.rect(page)
            && let Some(bounds) = geometry.rect(entity)
        {
            request_scroll(&mut commands, page, bounds, viewport);
        }
        return;
    }
    let bounds = geometry.bounds(entity, label);
    observation.publish(&mut state, bounds);
}

/// Run one isolated owner call on the capture's own runtime.
fn block_on<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> T {
    let slot = Arc::new(Mutex::new(None));
    let output = slot.clone();
    tokio_application_runtime()
        .expect("isolated settings capture runtime")
        .block_on(Box::pin(async move {
            *output.lock().expect("capture result slot") = Some(future.await);
        }));
    slot.lock()
        .expect("capture result slot")
        .take()
        .expect("the capture's own owner answered")
}
