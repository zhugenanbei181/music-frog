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
use crate::command::CommandSinkHandle;
use crate::pages::settings::settings_core::{SettingsLine, SettingsLineKind};
use crate::pages::settings::settings_port_conflict::PortConflictOutcomeText;
use crate::pages::settings::settings_privileged_network::{
    PrivilegedNetworkRunButton, PrivilegedNetworkStatusLine,
};
use crate::pages::settings::{PortConflictButton, PrepareTunPermissionButton, SettingsPageRoot};
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
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::privileged_network_application::PrivilegedNetworkApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::service_mode::{ServiceModePlatform, ServiceModeState};
use infiltrator_presentation::host_network_projection::privileged;
use infiltrator_presentation::settings_status_projection::format_service_mode;
use infiltrator_shared::locales::Lang;
use infiltrator_test_support::port_conflict_fixtures::PortConflictCapturePort;
use infiltrator_test_support::privileged_network_fixtures::PrivilegedNetworkCapturePort;
use infiltrator_test_support::service_mode_fixtures::ServiceModeCapturePort;
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

/// The isolated owners the settings capture really drives.
#[derive(Resource)]
struct SettingsCapture {
    stage: SettingsStage,
    port_conflicts: Arc<PortConflictApplication>,
    privileged: Arc<PrivilegedNetworkApplication>,
    service_mode: Arc<ServiceModeApplication>,
}

pub fn install(app: &mut App) {
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
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        capture_command_application()
            .with_port_conflicts(port_conflicts.as_ref().clone())
            .with_privileged_network(privileged.as_ref().clone())
            .with_service_mode(service_mode.as_ref().clone()),
        CaptureCapability::Settings,
    ))));
    app.insert_resource(SettingsCapture {
        stage: SettingsStage::Unpublished,
        port_conflicts,
        privileged,
        service_mode,
    });
}

/// The page's own controls the settings gestures drive.
#[derive(SystemParam)]
pub struct SettingsControls<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, SettingsCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    locale: Res<'w, UiLocale>,
    repairs: Query<'w, 's, Entity, With<PortConflictButton>>,
    prepares: Query<'w, 's, Entity, With<PrepareTunPermissionButton>>,
    regressions: Query<'w, 's, Entity, With<PrivilegedNetworkRunButton>>,
    pages: Query<'w, 's, Entity, With<SettingsPageRoot>>,
}

/// Drive the Settings page's own privilege, port and regression gestures.
pub fn activate(mut controls: SettingsControls, mut commands: Commands) {
    if controls.route.0 != Some(Route::Settings) {
        return;
    }
    match controls.feature.feature {
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
    pages: Query<'w, 's, Entity, With<SettingsPageRoot>>,
}

/// Observe the copy the Settings page really renders for the business.
pub fn observe(
    mut controls: SettingsObservation,
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
    if controls.capture.stage != SettingsStage::Activated {
        return;
    }
    // The page's own copy is the receipt's subject; the line itself is
    // measured only once it is really rendered.
    let subject = (|| {
        match target {
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
                    .find(|(_, text, line)| line.0 == SettingsLineKind::ServiceMode)?;
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
