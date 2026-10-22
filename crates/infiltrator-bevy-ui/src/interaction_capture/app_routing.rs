//! Edit one application's split-tunneling rule through the shared command.
//!
//! The app-routing page owns one policy per application. The capture activates
//! the row's own policy control, so the surface really submits the shared
//! `SetAppRule` command. The capture's own routing owner persists the rule,
//! and the capture publishes that owner readback into the page's own
//! projection, so the rendered rule really changes.
use super::capability::{capture_command_application, capture_host_capabilities};
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::app_routing::{
    AppRoutingPageRoot, AppRoutingProjectionUpdated, AppRuleText, SwitchAppRuleButton,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, app_routing_projection};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_contract::app_routing::AppRoutingRule;
use infiltrator_contract::parity::FeatureId;
use infiltrator_domain::app_routing::AppRoutingConfig;
use infiltrator_ports::app_routing_store::AppRoutingStore;
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};

/// The application whose rule the capture edits.
const TARGET_APP: &str = "app-1";

/// The projection index of the edited row.
const TARGET_ROW: usize = 0;

/// The capture's own routing owner: one in-memory policy the shared
/// application writes through.
///
/// The owner is a real [`AppRoutingStore`] port, so the shared routing
/// application reads, mutates and writes exactly the way the product host
/// does; nothing about the rule is fabricated by the capture.
struct CaptureRoutingStore(Arc<Mutex<AppRoutingConfig>>);

impl CaptureRoutingStore {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(AppRoutingConfig::default())))
    }

    /// The rule the owner persisted for one application.
    fn rule(&self, app_id: &str) -> Option<AppRoutingRule> {
        self.0
            .lock()
            .expect("capture routing store")
            .rules
            .get(app_id)
            .copied()
            .map(|rule| rule.into())
    }
}

impl AppRoutingStore for CaptureRoutingStore {
    fn load(&self) -> Result<AppRoutingConfig, PortError> {
        Ok(self.0.lock().expect("capture routing store").clone())
    }
    fn save(&self, config: &AppRoutingConfig) -> Result<(), PortError> {
        *self.0.lock().expect("capture routing store") = config.clone();
        Ok(())
    }
}

/// The capture's own progress through the policy edit.
#[derive(Resource)]
pub struct RoutingCapture {
    store: Arc<CaptureRoutingStore>,
    initial_rule_text: Option<String>,
    submitted: bool,
    published: bool,
}

pub fn install(app: &mut App) {
    let store = Arc::new(CaptureRoutingStore::new());
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        capture_command_application()
            .with_host_capabilities(capture_host_capabilities([
                Capability::AppRouting,
                Capability::CoreLifecycle,
                Capability::Dns,
                Capability::Logs,
                Capability::Profiles,
            ]))
            .with_routing(RoutingApplication::new(store.clone())),
        CaptureCapability::AppRouting,
    ))));
    app.insert_resource(RoutingCapture {
        store,
        initial_rule_text: None,
        submitted: false,
        published: false,
    });
}

/// The page's own row marker and its rendered policy label.
#[derive(SystemParam)]
pub struct RoutingSurface<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, RoutingCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    buttons: Query<'w, 's, (Entity, &'static SwitchAppRuleButton)>,
    rules: Query<'w, 's, (Entity, &'static AppRuleText, &'static Text)>,
    pages: Query<'w, 's, Entity, With<AppRoutingPageRoot>>,
}

/// Drive the policy edit: the capture installs its own owner, activates the
/// row's own control, and publishes the owner's readback.
pub fn activate(
    mut surface: RoutingSurface,
    feature: Res<InteractionCapture>,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &surface.route,
        FeatureId::AppRoutingPolicyEditor,
        Route::AppRouting,
    ) {
        return;
    }
    if surface.capture.initial_rule_text.is_none() {
        let Some((_, _, text)) = surface
            .rules
            .iter()
            .find(|(_, marker, _)| marker.0 == TARGET_ROW)
        else {
            return;
        };
        surface.capture.initial_rule_text = Some(text.0.clone());
        return;
    }
    if !surface.capture.submitted {
        let Some((entity, _button)) = surface
            .buttons
            .iter()
            .find(|(_, button)| button.app_id == TARGET_APP)
        else {
            return;
        };
        // The compact viewport starts the row below the fold: the capture
        // scrolls the page's own viewport until the control is really on
        // screen, and activates it on the next frame.
        if reveal(&surface, &mut geometry, &mut commands, entity) {
            return;
        }
        surface.capture.submitted = true;
        commands.trigger(Activate { entity });
        return;
    }
    if surface.capture.published {
        return;
    }
    let Some(rule) = surface.capture.store.rule(TARGET_APP) else {
        return;
    };
    surface.capture.published = true;
    publish_owner_rule(&mut surface.latest, rule, &mut commands);
}

/// Scroll the app-routing page until the control is really rendered.
///
/// Returns `true` when a scroll was requested, so the caller waits for the
/// next frame before it activates or measures the control.
fn reveal(
    surface: &RoutingSurface<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
    target: Entity,
) -> bool {
    let Ok(page) = surface.pages.single() else {
        return false;
    };
    let Some(viewport) = geometry.rect(page) else {
        return false;
    };
    let Some(bounds) = geometry.rect(target) else {
        return false;
    };
    !request_scroll(commands, page, bounds, viewport)
}

/// Publish the owner's readback into the page's own projection.
///
/// The capture's snapshot belongs to another core scope, so the whole-snapshot
/// event is fenced out; the page's own projection event carries the owner's
/// rule instead.
fn publish_owner_rule(
    latest: &mut LatestSurfaceSnapshot,
    rule: AppRoutingRule,
    commands: &mut Commands,
) {
    {
        let Some(page) = latest.0.pages.app_routing.data.as_mut() else {
            return;
        };
        let Some(app) = page.apps.iter_mut().find(|app| app.id == TARGET_APP) else {
            return;
        };
        app.rule = rule;
    }
    let projection = app_routing_projection(&latest.0);
    commands.trigger(AppRoutingProjectionUpdated(projection));
}

/// Observe the edited row's own policy label.
pub fn observe(
    surface: RoutingSurface,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    let bounds = (|| {
        let initial = surface.capture.initial_rule_text.as_ref()?;
        let (entity, _, text) = surface
            .rules
            .iter()
            .find(|(_, marker, _)| marker.0 == TARGET_ROW)?;
        if text.0 == *initial {
            return None;
        }
        // The compact viewport starts the edited row below the fold: the
        // capture scrolls the page's own viewport until the label is really
        // rendered, and measures it on the next frame.
        if !geometry.unclipped(entity) {
            reveal(&surface, &mut geometry, &mut commands, entity);
            return None;
        }
        geometry.bounds(entity, "app routing rule")
    })();
    observation.publish(&mut feature, bounds);
}
