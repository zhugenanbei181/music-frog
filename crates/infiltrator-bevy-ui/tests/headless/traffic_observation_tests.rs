//! test-intent: behavior
//! Native rate texts consume real reader results, preserve stale zeros and retain entity identity on locale changes.
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::input::mouse::{MouseScrollPixelsPerLine, MouseScrollUnit};
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerScroll};
use bevy::picking::pointer::{Location, PointerId};
use bevy::ui::widget::Text;
use bevy::ui::{ComputedNode, ScrollPosition};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::LogCaptureProcess;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::telemetry_observation_fixtures::TelemetryObservationReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::overview::{OverviewLine, OverviewLineKind, OverviewPageRoot};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::SurfaceSnapshotUpdated;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;
fn run<T>(future: impl Future<Output = T>) -> T {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn overview_uses_actual_native_scroll_input_and_preserves_the_page_and_rate_entities() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::default()));
    app.init_resource::<MouseScrollPixelsPerLine>();
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Overview));
    app.update();
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<OverviewPageRoot>>()
        .single(app.world())
        .unwrap();
    let rate = upload(&mut app);
    {
        let mut node = app.world_mut().get_mut::<ComputedNode>(root).unwrap();
        node.size = Vec2::new(620.0, 360.0);
        node.content_size = Vec2::new(620.0, 1500.0);
        node.inverse_scale_factor = 1.0;
    }
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 0.0);
    app.world_mut().trigger(PointerScroll {
        entity: root,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 620,
                    height: 360,
                },
                position: Vec2::new(10.0, 10.0),
            },
        ),
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: -300.0,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        phase: TouchPhase::Moved,
    });
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 300.0);
    assert_eq!(upload(&mut app), rate);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<OverviewPageRoot>>()
            .single(app.world())
            .unwrap(),
        root
    );
}
fn upload(app: &mut App) -> (Entity, String) {
    let (entity, _, text) = app
        .world_mut()
        .query::<(Entity, &OverviewLine, &Text)>()
        .iter(app.world())
        .find(|(_, line, _)| line.0 == OverviewLineKind::Upload)
        .unwrap();
    (entity, text.0.clone())
}
#[test]
fn native_rates_unknown_zero_failed_stale_locale_and_new_session_follow_the_shared_observation_owner()
 {
    let process = Arc::new(LogCaptureProcess::default());
    let telemetry = Arc::new(TelemetryObservationReader::default());
    let core = Arc::new(CoreApplication::new_with_overview(
        process.clone(),
        process,
        telemetry.clone(),
        tokio_application_runtime().unwrap(),
    ));
    run(core.execute(CommandIntent::StartCore))
        .into_unit()
        .unwrap();
    let reader =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::default()));
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Overview));
    app.update();
    let publish = |app: &mut App| {
        app.world_mut()
            .trigger(SurfaceSnapshotUpdated(run(reader.read()).unwrap()));
        app.update();
    };
    publish(&mut app);
    let (entity, text) = upload(&mut app);
    assert_eq!(text, "↑ Not observed");
    publish(&mut app);
    assert_eq!(upload(&mut app), (entity, "↑ 0 B/s".into()));
    telemetry.denied.store(true, Ordering::Release);
    publish(&mut app);
    assert_eq!(upload(&mut app), (entity, "↑ 0 B/s (stale)".into()));
    let reason = app
        .world_mut()
        .query::<(&OverviewLine, &Text)>()
        .iter(app.world())
        .find(|(line, _)| line.0 == OverviewLineKind::TelemetryFailure)
        .unwrap()
        .1
        .0
        .clone();
    assert!(reason.contains("isolated telemetry read denied"));
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(upload(&mut app), (entity, "↑ 0 B/s（已失效）".into()));
    telemetry.denied.store(false, Ordering::Release);
    run(core.execute(CommandIntent::RestartCore))
        .into_unit()
        .unwrap();
    publish(&mut app);
    assert_eq!(upload(&mut app).0, entity);
    assert!(upload(&mut app).1.contains("未观测"));
    run(core.close()).unwrap();
}
