//! test-intent: behavior
//! Native shell controls distinguish missing, observed and failed controller facts.
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, InteractionDisabled};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::runtime_control_projection::RuntimeControlApplication;
use infiltrator_bevy_ui::app::{
    GlobalModeCapsule, GlobalStatusDot, PendingModeAck, ShellPlugin, SidebarScriptModePill,
    SidebarTunToggle,
};
use infiltrator_bevy_ui::pages::overview::OverviewModePill;
use infiltrator_bevy_ui::route::{OverviewSourceHandle, PagesPlugin};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::runtime::{ConfigSnapshot, TunSnapshot};
use infiltrator_ports::error::PortError;

fn caption(app: &App, capsule: Entity) -> String {
    app.world()
        .get::<Children>(capsule)
        .unwrap()
        .iter()
        .find_map(|child| app.world().get::<Text>(*child).map(|text| text.0.clone()))
        .unwrap()
}

#[test]
fn native_mode_and_tun_controls_remain_unknown_until_observed_and_failed_reads_block_actions() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ButtonPlugin, ShellPlugin::default(), PagesPlugin::default()));
    app.update();
    let capsule = app
        .world_mut()
        .query_filtered::<Entity, With<GlobalModeCapsule>>()
        .single(app.world())
        .unwrap();
    let script = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarScriptModePill>>()
        .single(app.world())
        .unwrap();
    let tun = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarTunToggle>>()
        .single(app.world())
        .unwrap();
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let observations = RuntimeControlApplication::default();
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.proxy_mode = None;
    snapshot.pages.overview.data.as_mut().unwrap().proxy_mode = None;
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    snapshot.runtime_control = observations.observe(snapshot.generation, snapshot.revision, None);
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(caption(&app, capsule), "未观测");
    assert!(app.world().get::<ButtonDisabled>(script).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(script).is_some());
    assert!(app.world().get::<ButtonDisabled>(tun).unwrap().0);
    let before = app
        .world()
        .resource::<OverviewSourceHandle>()
        .0
        .current()
        .proxy_mode
        .current;
    click_entity(&mut app, script);
    assert!(app.world().resource::<PendingModeAck>().0.is_none());
    assert_eq!(
        app.world()
            .resource::<OverviewSourceHandle>()
            .0
            .current()
            .proxy_mode
            .current,
        before
    );
    let config = ConfigSnapshot {
        mode: "global".into(),
        tun: Some(TunSnapshot {
            enable: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    snapshot.revision += 1;
    snapshot.runtime_control = observations.observe(
        snapshot.generation,
        snapshot.revision,
        Some(&Ok(config.clone())),
    );
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(caption(&app, capsule), "全局");
    let global = app
        .world_mut()
        .query::<(Entity, &OverviewModePill)>()
        .iter(app.world())
        .find(|(_, mode)| mode.0 == ProxyMode::Global)
        .unwrap()
        .0;
    assert!(app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_none());
    assert!(
        !app.world()
            .get::<ButtonDisabled>(tun)
            .is_some_and(|disabled| disabled.0)
    );
    assert!(
        app.world().get::<ButtonDisabled>(script).unwrap().0,
        "no declared script block"
    );
    let denied = Failure::new(ErrorCode::Authentication, "controller read denied", false);
    snapshot.revision += 1;
    snapshot.runtime_control = observations.observe(
        snapshot.generation,
        snapshot.revision,
        Some(&Err(PortError::Rejected(denied.clone()))),
    );
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<(Entity, &OverviewModePill)>()
            .iter(app.world())
            .filter(|(entity, _)| !app.world().get::<ButtonDisabled>(*entity).unwrap().0)
            .count(),
        0
    );
    assert!(app.world().get::<ButtonDisabled>(tun).unwrap().0);
    assert!(app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_some());
    assert_eq!(caption(&app, capsule), "全局（已失效）");
    assert_eq!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .runtime_control
            .status,
        RuntimeControlStatus::Failed { failure: denied }
    );
    snapshot.revision += 1;
    snapshot.runtime_control =
        observations.observe(snapshot.generation, snapshot.revision, Some(&Ok(config)));
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
    assert!(app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_none());
    assert!(!app.world().get::<ButtonDisabled>(global).unwrap().0);
    assert_eq!(caption(&app, capsule), "全局");
}

#[test]
fn native_global_status_dot_replays_each_actual_lifecycle_without_remounting() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::default()));
    app.update();
    let dot = app
        .world_mut()
        .query_filtered::<Entity, With<GlobalStatusDot>>()
        .single(app.world())
        .unwrap();
    let palette = *app.world().resource::<UiPalette>();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    for (lifecycle, color) in [
        (CoreLifecycle::Stopped, palette.ink_dim),
        (CoreLifecycle::Starting, palette.warning),
        (CoreLifecycle::Ready, palette.success),
        (CoreLifecycle::Running, palette.success),
        (CoreLifecycle::Stopping, palette.warning),
        (CoreLifecycle::Failed, palette.danger),
    ] {
        snapshot.revision += 1;
        snapshot.core.lifecycle = lifecycle;
        app.world_mut()
            .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
        app.update();
        app.update();
        assert_eq!(app.world().get::<BackgroundColor>(dot).unwrap().0, color);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<GlobalStatusDot>>()
                .single(app.world())
                .unwrap(),
            dot
        );
    }
}
