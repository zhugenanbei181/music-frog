//! test-intent: behavior
use crate::command_harness::{RecordingHandler, recording_application};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_application::proxy_inspection_fixtures::{INSPECTION_NODE, observed_proxy};
use infiltrator_application::proxy_inspection_projection::{
    field_value, history_plot, project_proxy_inspection,
};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::command::UiCommand;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::proxies::NodeDetailButton;
use infiltrator_bevy_ui::pages::proxy_inspection::ProxyDetailValue;
use infiltrator_bevy_ui::pages::proxy_inspection::{
    CloseProxyInspection, ProxyInspectionCard, ProxyInspectionChart, ProxyInspectionRoot,
    ProxyInspectionState, ProxyInspectionTitle,
};
use infiltrator_bevy_ui::pages::proxy_probe::{ProbeInspectedProxy, ProxyProbeStatus};
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
    overview_projection,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_bevy_widgets::chart::bezier::ScaleMode;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_inspection::ProxyDetailField;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Source(SurfaceSnapshot);
impl OverviewSource for Source {
    fn current(&self) -> OverviewProjection {
        overview_projection(&self.0)
    }
    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }
}
impl SurfaceSource for Source {
    fn surface_snapshot(&self) -> SurfaceSnapshot {
        self.0.clone()
    }
}
fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
}
fn activate(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}
fn publish(app: &mut App, snapshot: SurfaceSnapshot) {
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
fn setup() -> (App, Arc<RecordingHandler>) {
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let page = snapshot.pages.proxies.data.as_mut().unwrap();
    page.groups[0].proxies[0].name = INSPECTION_NODE.into();
    page.groups[0].current = INSPECTION_NODE.into();
    page.node_details
        .push(project_proxy_inspection(INSPECTION_NODE, &observed_proxy()));
    let (application, handler) = recording_application();
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new_surface(Source(snapshot)));
    app.add_plugins(CommandPumpPlugin::for_application(Arc::new(application)));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    (app, handler)
}
fn open(app: &mut App) {
    let button = app
        .world_mut()
        .query::<(Entity, &NodeDetailButton)>()
        .iter(app.world())
        .find(|(_, button)| button.node_name == INSPECTION_NODE)
        .unwrap()
        .0;
    activate(app, button);
}
fn finish(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while app
        .world()
        .resource::<ProxyInspectionState>()
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "tracked probe must receive its terminal result"
        );
        app.update();
        yield_now();
    }
}

#[test]
fn native_inspection_replays_real_metadata_and_history_and_close_never_selects_a_proxy() {
    let (mut app, handler) = setup();
    open(&mut app);
    assert_eq!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .as_deref(),
        Some(INSPECTION_NODE)
    );
    let root = entity::<ProxyInspectionRoot>(&mut app);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::Flex
    );
    let title = entity::<ProxyInspectionTitle>(&mut app);
    assert_eq!(app.world().get::<Text>(title).unwrap().0, INSPECTION_NODE);
    let chart = entity::<ProxyInspectionChart>(&mut app);
    assert_eq!(
        app.world().get::<ChartPlate>(chart).unwrap().0.scale_mode,
        ScaleMode::Fixed(42.0)
    );
    let values = &app.world().get::<ChartPlate>(chart).unwrap().0.up;
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], 18.0);
    assert!(values[1].is_nan());
    assert_eq!(values[2], 42.0);
    assert_eq!(
        history_plot(&project_proxy_inspection(
            INSPECTION_NODE,
            &observed_proxy()
        ))
        .samples
        .len(),
        values.len()
    );
    assert!(handler.0.lock().unwrap().is_empty());
    let detail = project_proxy_inspection(INSPECTION_NODE, &observed_proxy());
    for field in ProxyDetailField::ALL {
        let mut values = app.world_mut().query::<(&ProxyDetailValue, &Text)>();
        assert_eq!(
            values
                .iter(app.world())
                .find(|(marker, _)| marker.0 == field)
                .unwrap()
                .1
                .0,
            field_value(&detail, field, &|key| Lang("zh-CN").tr(key).into_owned())
        );
    }
    let timing = app
        .world_mut()
        .query::<(&LocalizedText, &Text)>()
        .iter(app.world())
        .find(|(copy, _)| copy.key == "proxy_inspection_timing_unavailable")
        .unwrap()
        .1
        .0
        .clone();
    assert_eq!(
        timing,
        Lang("zh-CN").tr("proxy_inspection_timing_unavailable")
    );
    let close = entity::<CloseProxyInspection>(&mut app);
    activate(&mut app, close);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_none()
    );
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn native_escape_navigation_and_stale_probe_results_cannot_reopen_or_unlock_an_inspection() {
    let (mut app, handler) = setup();
    open(&mut app);
    let card = entity::<ProxyInspectionCard>(&mut app);
    let button = entity::<ProbeInspectedProxy>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.world_mut().flush();
    let pending = app
        .world()
        .resource::<ProxyInspectionState>()
        .pending
        .clone()
        .unwrap();
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::TestProxyNode {
            node: INSPECTION_NODE.into(),
        },
        request_id: RequestId(pending.request_id.0.wrapping_add(999)),
        result: Err(Failure::new(ErrorCode::Permission, "old failure", false)),
    });
    app.world_mut().flush();
    assert_eq!(
        app.world().resource::<ProxyInspectionState>().pending,
        Some(pending.clone())
    );
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.insert_resource(keys);
    app.update();
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_none()
    );
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .pending
            .is_none()
    );
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::TestProxyNode {
            node: INSPECTION_NODE.into(),
        },
        request_id: pending.request_id,
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_none()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    open(&mut app);
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Profiles));
    app.update();
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_none()
    );
    assert!(app.world().get_entity(card).is_err());
    assert!(
        !handler
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|intent| matches!(intent, CommandIntent::SelectProxyNode { .. }))
    );
}

#[test]
fn native_inspection_keeps_identity_on_reorder_filtering_refresh_and_closes_on_removal() {
    let (mut app, handler) = setup();
    open(&mut app);
    let card = entity::<ProxyInspectionCard>(&mut app);
    let title = entity::<ProxyInspectionTitle>(&mut app);
    let chart = entity::<ProxyInspectionChart>(&mut app);
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    let page = snapshot.pages.proxies.data.as_mut().unwrap();
    page.groups.reverse();
    for group in &mut page.groups {
        group.proxies.retain(|node| node.name != INSPECTION_NODE);
    }
    page.node_details
        .iter_mut()
        .find(|detail| detail.name == INSPECTION_NODE)
        .unwrap()
        .delay_ms = Some(91);
    publish(&mut app, snapshot);
    assert_eq!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .as_deref(),
        Some(INSPECTION_NODE)
    );
    assert!(app.world().get::<ProxyInspectionCard>(card).is_some());
    assert_eq!(app.world().get::<Text>(title).unwrap().0, INSPECTION_NODE);
    assert!(app.world().get::<ChartPlate>(chart).is_some());
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot
        .pages
        .proxies
        .data
        .as_mut()
        .unwrap()
        .node_details
        .retain(|detail| detail.name != INSPECTION_NODE);
    publish(&mut app, snapshot);
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_none()
    );
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn native_probe_preserves_typed_failure_and_retry_submits_only_the_shared_leaf_intent() {
    let (mut app, handler) = setup();
    open(&mut app);
    let failure = Failure::new(ErrorCode::Permission, "grant proxy probe permission", true);
    *handler.1.lock().unwrap() = Some(failure.clone());
    let button = entity::<ProbeInspectedProxy>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.world_mut().flush();
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .pending
            .is_some()
    );
    finish(&mut app);
    assert_eq!(
        app.world().resource::<ProxyInspectionState>().failure,
        Some(failure)
    );
    assert_eq!(handler.0.lock().unwrap().len(), 1);
    let status = entity::<ProxyProbeStatus>(&mut app);
    assert_eq!(
        app.world().get::<Text>(status).unwrap().0,
        "grant proxy probe permission"
    );
    *handler.1.lock().unwrap() = None;
    activate(&mut app, button);
    finish(&mut app);
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .failure
            .is_none()
    );
    assert_eq!(
        *handler.0.lock().unwrap(),
        vec![
            CommandIntent::TestNodeDelay {
                node: INSPECTION_NODE.into(),
                url: None,
                timeout_ms: None
            };
            2
        ]
    );
}

#[test]
fn failed_or_loading_reads_keep_visible_last_observations_and_recover_until_successful_deletion() {
    let (mut app, handler) = setup();
    open(&mut app);
    let root = entity::<ProxyInspectionRoot>(&mut app);
    let title = entity::<ProxyInspectionTitle>(&mut app);
    let chart = entity::<ProxyInspectionChart>(&mut app);
    let probe = entity::<ProbeInspectedProxy>(&mut app);
    let original = app
        .world()
        .resource::<ProxyInspectionState>()
        .read
        .detail
        .clone();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let mut page = snapshot.pages.proxies.data.clone().unwrap();
    let failure = Failure::new(ErrorCode::Network, "controller read failed", true);
    for bad in [
        PageData::failed(failure.clone()),
        PageData::unavailable(failure.clone()),
        PageData::loading(),
    ] {
        snapshot.revision += 1;
        snapshot.pages.proxies = bad;
        publish(&mut app, snapshot.clone());
        let state = app.world().resource::<ProxyInspectionState>();
        assert_eq!(state.selected.as_deref(), Some(INSPECTION_NODE));
        assert_eq!(state.read.detail, original);
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::Flex
        );
        assert_eq!(app.world().get::<Text>(title).unwrap().0, INSPECTION_NODE);
        assert!(app.world().get::<ButtonDisabled>(probe).unwrap().0);
        activate(&mut app, probe);
        assert!(
            handler.0.lock().unwrap().is_empty(),
            "failed reads cannot submit stale probe operations"
        );
        assert_eq!(entity::<ProxyInspectionChart>(&mut app), chart);
    }
    page.node_details
        .iter_mut()
        .find(|detail| detail.name == INSPECTION_NODE)
        .unwrap()
        .delay_ms = Some(91);
    snapshot.revision += 1;
    snapshot.pages.proxies = PageData::ready(page.clone());
    publish(&mut app, snapshot.clone());
    assert_eq!(
        app.world()
            .resource::<ProxyInspectionState>()
            .read
            .detail
            .as_ref()
            .unwrap()
            .delay_ms,
        Some(91)
    );
    assert!(!app.world().get::<ButtonDisabled>(probe).unwrap().0);
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .read
            .failure
            .is_none()
    );
    page.node_details.clear();
    page.groups.clear();
    snapshot.revision += 1;
    snapshot.pages.proxies = PageData::empty(page);
    publish(&mut app, snapshot);
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_none()
    );
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    activate(&mut app, probe);
    assert!(handler.0.lock().unwrap().is_empty());
}
