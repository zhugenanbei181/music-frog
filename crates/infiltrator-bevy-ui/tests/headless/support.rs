use bevy::MinimalPlugins;
use bevy::app::{App, Plugin};
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::Res;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::Text;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_bevy_ui::app::ContentSlot;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::command_execution::ApplicationCommandSink;
use infiltrator_bevy_ui::route::{PageRoot, Route};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CapabilityAccess, CapabilityMaturity,
    CoreCapabilityKey, CoreCapabilityResolution, CoreCapabilityScope, CoreCapabilitySnapshot,
    CoreCapabilityStatus, CoreOperation, EngineCapabilityDecision, HostCapabilityDecision,
    OutboundOperation,
};
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use infiltrator_ports::core_scope::CoreScopeObservation;
use infiltrator_ports::profile_store::ProfileStore;
use std::sync::{Arc, RwLock};

pub fn headless_plugins(app: &mut App) {
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    app.init_asset::<Image>();
}

/// Bind a headless test sink to the initial fixture snapshot, then advance
/// the observation only when the test accepts another surface snapshot.
pub fn command_pump_for_observed_snapshot(
    application: Arc<CoreApplication>,
) -> ObservedSnapshotCommandPump {
    let observed_scope = observed_scope_for_snapshot(&application);
    ObservedSnapshotCommandPump {
        application,
        observed_scope,
    }
}

pub struct ObservedSnapshotCommandPump {
    application: Arc<CoreApplication>,
    observed_scope: Arc<RwLock<CoreScopeObservation>>,
}

#[derive(Resource)]
struct TestScopeObservation(Arc<RwLock<CoreScopeObservation>>);

fn observe_accepted_test_snapshot(
    update: On<SurfaceSnapshotUpdated>,
    latest: Res<LatestSurfaceSnapshot>,
    observed: Res<TestScopeObservation>,
) {
    if !update.0.is_newer_than(&latest.0) {
        return;
    }
    *observed.0.write().expect("test surface scope observation") = update
        .0
        .core
        .scope()
        .map(CoreScopeObservation::Observed)
        .unwrap_or(CoreScopeObservation::Unavailable);
}

impl Plugin for ObservedSnapshotCommandPump {
    fn build(&self, app: &mut App) {
        app.insert_resource(TestScopeObservation(self.observed_scope.clone()));
        app.add_observer(observe_accepted_test_snapshot);
        app.add_plugins(CommandPumpPlugin::new(Arc::new(
            ApplicationCommandSink::new_for_surface(
                self.application.clone(),
                self.observed_scope.clone(),
            ),
        )));
    }
}

pub fn observed_scope_for_snapshot(
    application: &CoreApplication,
) -> Arc<RwLock<CoreScopeObservation>> {
    let scope = application
        .snapshot()
        .scope()
        .expect("headless Core fixture must expose its owner identity");
    Arc::new(RwLock::new(CoreScopeObservation::Observed(scope)))
}

pub async fn execute_at_observed_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CommandResult {
    let scope = application
        .snapshot()
        .scope()
        .expect("headless Core fixture must expose its owner identity");
    application
        .execute_scoped(CoreCommandRequest { scope, intent })
        .await
        .result
}

pub fn page_root(world: &mut World) -> (Entity, Route) {
    let mut query = world.query::<(Entity, &PageRoot)>();
    let (entity, root) = query.single(world).expect("single page root mounted");
    (entity, root.0)
}

pub fn content_slot(world: &mut World) -> Entity {
    let mut query = world.query_filtered::<Entity, With<ContentSlot>>();
    query.single(world).expect("single ContentSlot in shell")
}

pub fn subtree_has_text(world: &World, root: Entity, needle: &str) -> bool {
    if let Some(text) = world.get::<Text>(root)
        && text.0.contains(needle)
    {
        return true;
    }
    if let Some(children) = world.get::<Children>(root) {
        for child in children.iter() {
            if subtree_has_text(world, *child, needle) {
                return true;
            }
        }
    }
    false
}

/// Explicit test-only evidence for exercising an outbound probe action. Demo
/// and capture surfaces keep this operation unavailable unless a test opts in.
pub fn allow_outbound_probe(snapshot: &mut SurfaceSnapshot) {
    let core_scope = snapshot
        .core
        .scope()
        .expect("test fixture has an active Core scope");
    let capability_scope = CoreCapabilityScope::new(
        core_scope.identity.engine,
        core_scope.identity.version,
        HostKind::Desktop,
        AdapterRevision::new(1),
    );
    let operation = CoreOperation::Outbound(OutboundOperation::Probe);
    snapshot.core_capability_snapshot = PageData::ready(CoreCapabilitySnapshot {
        scope: capability_scope.clone(),
        resolutions: vec![CoreCapabilityResolution {
            key: CoreCapabilityKey::new(capability_scope, operation),
            engine: Some(EngineCapabilityDecision::Supported),
            host: Some(HostCapabilityDecision::Supported),
            adapter: Some(AdapterCapabilityDecision::Supported),
            status: CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                maturity: CapabilityMaturity::Stable,
            },
        }],
    });
    snapshot.probe_settings = RuntimeProbeSettingsState::Available {
        settings: RuntimeProbeSettings::default(),
    };
}

/// Headless Bevy tests select the same engine codec as the product host.
pub fn mihomo_profiles(store: Arc<dyn ProfileStore>) -> ProfileApplication {
    profiles_for_mihomo(store)
}
