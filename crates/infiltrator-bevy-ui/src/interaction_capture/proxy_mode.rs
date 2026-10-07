//! Scope-limited capture activates the production mode observer and checks its real failed result.
use super::geometry::CaptureGeometry;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::app::ModeActionState;
use crate::pages::overview_cards::OverviewModeSegmentPill;
use crate::projection::{OverviewProjection, OverviewSource, SourceKind};
use crate::route::{ActiveRoute, OverviewSourceHandle, Route};
use crate::shell_mode_issue::{
    DismissModeIssue, ModeIssueRoot, ModeIssueText, ModeSettingsGuide, RetryModeChange,
};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated, overview_projection};
use bevy::app::{App, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::proxy_mode_application::ProxyModeApplication;
use infiltrator_application::proxy_mode_fixtures::ModeScenarioController;
use infiltrator_application::proxy_mode_projection::mode_failure_copy;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::SurfaceKind;
use std::sync::Arc;
use std::sync::mpsc::Sender;

struct ModeSource {
    application: CoreApplication,
    controller: Arc<ModeScenarioController>,
}
impl OverviewSource for ModeSource {
    fn current(&self) -> OverviewProjection {
        overview_projection(
            &self
                .controller
                .snapshot(self.application.snapshot(), SurfaceKind::BevyDesktop),
        )
    }
    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }
    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<ProxyMode, Failure>>) {
        let application = self.application.clone();
        tokio_application_runtime()
            .expect("capture runtime")
            .block_on(Box::pin(async move {
                let _ = ack.send(ProxyModeApplication::change(&application, mode).await);
            }));
    }
}
#[derive(Resource)]
struct ModeCapture {
    source: Arc<ModeSource>,
    initialized: bool,
}
pub fn install(app: &mut App, feature: FeatureId) {
    let controller = Arc::new(ModeScenarioController::default());
    if feature == FeatureId::ShellProxyModeAuthentication {
        controller.reject_with(Some(Failure::new(
            ErrorCode::Authentication,
            "Controller token rejected",
            false,
        )));
    }
    let runtime = tokio_application_runtime().expect("capture runtime");
    let application = CoreApplication::new_with_overview(
        controller.clone(),
        controller.clone(),
        controller.clone(),
        runtime.clone(),
    );
    let ready = application.clone();
    runtime.block_on(Box::pin(async move {
        assert!(ready.adopt_if_running().await.expect("fixture readiness"));
    }));
    let source = Arc::new(ModeSource {
        application,
        controller,
    });
    app.insert_resource(OverviewSourceHandle(source.clone()));
    app.insert_resource(ModeCapture {
        source,
        initialized: false,
    });
    app.add_systems(Update, initialize.before(activate));
}
fn initialize(mut capture: ResMut<ModeCapture>, mut commands: Commands) {
    if !capture.initialized {
        commands.trigger(SurfaceSnapshotUpdated(capture.source.controller.snapshot(
            capture.source.application.snapshot(),
            SurfaceKind::BevyDesktop,
        )));
        capture.initialized = true;
    }
}
pub fn activate(
    feature: Res<InteractionCapture>,
    route: Res<ActiveRoute>,
    state: Res<ModeActionState>,
    controls: Query<(Entity, &OverviewModeSegmentPill, &ButtonDisabled)>,
    mut commands: Commands,
) {
    if !selected(&feature, &route, feature.feature, Route::Overview)
        || state.0.pending.is_some()
        || state.0.failure.is_some()
    {
        return;
    }
    if let Some((entity, _, _)) = controls
        .iter()
        .find(|(_, mode, disabled)| mode.0 == ProxyMode::Global && !disabled.0)
    {
        commands.trigger(Activate { entity });
    }
}
#[derive(SystemParam)]
pub struct ModeSurface<'w, 's> {
    state: Res<'w, ModeActionState>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    capture: Res<'w, ModeCapture>,
    locale: Res<'w, UiLocale>,
    roots: Query<'w, 's, Entity, With<ModeIssueRoot>>,
    retry: Query<'w, 's, (Entity, &'static ButtonDisabled), With<RetryModeChange>>,
    dismiss: Query<'w, 's, Entity, With<DismissModeIssue>>,
    guides: Query<'w, 's, Entity, With<ModeSettingsGuide>>,
    texts: Query<'w, 's, (Entity, &'static Text), With<ModeIssueText>>,
}
pub fn observe(
    surface: ModeSurface,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        let state = &surface.state.0;
        let authentication = feature.feature == FeatureId::ShellProxyModeAuthentication;
        let failure_code = if authentication {
            ErrorCode::Authentication
        } else {
            ErrorCode::Network
        };
        if state.pending.is_some()
            || state.observed.current != Some(ProxyMode::Rule)
            || state.failure.as_ref()?.code != failure_code
            || surface.capture.source.controller.requests() != [ProxyMode::Global]
            || surface.latest.0.core.lifecycle
                != surface.capture.source.application.snapshot().lifecycle
        {
            return None;
        }
        let (label, text) = surface.texts.single().ok()?;
        if text.0 != mode_failure_copy(state, surface.locale.code()) {
            return None;
        }
        geometry.bounds(label, "mode failure label")?;
        let (retry, disabled) = surface.retry.single().ok()?;
        if disabled.0 != authentication {
            return None;
        }
        if authentication {
            if !state.needs_controller_settings() || state.retry_target().is_some() {
                return None;
            }
            geometry.bounds(surface.guides.single().ok()?, "controller settings guide")?;
        } else if state.retry_target() != Some(ProxyMode::Global) {
            return None;
        }
        geometry.bounds(retry, "mode retry")?;
        geometry.bounds(surface.dismiss.single().ok()?, "mode dismiss")?;
        geometry.bounds(surface.roots.single().ok()?, "mode failure surface")
    })();
    observed.publish(&mut feature, bounds);
}
