//! test-intent: behavior
//! Native proxy search persists typed shared preferences and retains text input state.
use crate::command_harness::TypedOutboundPreferenceSink;
use crate::support::headless_plugins;
use bevy::app::{App, Update};
use bevy::ecs::change_detection::{DetectChanges, Ref};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, ResMut};
use bevy::ui::widget::Text;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::proxies::{
    LastProxiesProjection, NodeNameText, ProxiesScrollArea, ProxyNodeButton,
};
use infiltrator_bevy_ui::pages::proxies_highlight::sync_name_highlights;
use infiltrator_bevy_ui::pages::proxies_search::{
    ClearProxySearch, ProxySearchInput, ProxySearchPhase, ProxySearchState, ProxySearchStatus,
    RetryProxySearch,
};
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::overview_projection;
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_bevy_widgets::text_runs::TextRuns;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::outbound::inventory::OutboundInventorySnapshot;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::{
    details::{
        OutboundDetailsSnapshot, OutboundDetailsState, OutboundObservation, OutboundSnapshot,
    },
    identity::SnapshotRevision,
    preferences::{OutboundUiPreferenceIntent, OutboundUiPreferenceRequest},
};
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
fn base_snapshot() -> SurfaceSnapshot {
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let inventory = snapshot.outbound_inventory.data.as_ref().unwrap();
    let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
        unreachable!("demo inventory is ready")
    };
    let entries = entries
        .iter()
        .map(|entry| OutboundSnapshot {
            id: entry.id.clone(),
            display_name: Some(if entry.id.value().starts_with("HK-") {
                format!("🇭🇰 香港 {}", entry.id.value())
            } else {
                entry.id.value().to_owned()
            }),
        })
        .collect();
    snapshot.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            inventory.scope().clone(),
            inventory.revision(),
            OutboundInventoryState::Ready {
                entries,
                collections: match inventory.inventory() {
                    OutboundInventoryState::Ready { collections, .. } => collections.clone(),
                    _ => unreachable!(),
                },
            },
        )
        .unwrap(),
    );
    snapshot
}
fn setup() -> (App, TypedOutboundPreferenceSink, SurfaceSnapshot) {
    let snapshot = base_snapshot();
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .clone()
        .expect("demo has typed preferences");
    let sink = TypedOutboundPreferenceSink::new(preferences);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        ThemePreference::Fixed(ThemeSkin::Dark),
        1180.0,
    ));
    app.add_plugins(PagesPlugin::new_surface(Source(snapshot.clone())));
    app.add_plugins(CommandPumpPlugin::new(Arc::new(sink.clone())));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    publish(&mut app, &sink, &snapshot);
    (app, sink, snapshot)
}
fn field(app: &mut App) -> Entity {
    let wrapper = entity::<ProxySearchInput>(app);
    app.world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .unwrap()
}
fn publish(app: &mut App, sink: &TypedOutboundPreferenceSink, base: &SurfaceSnapshot) {
    let mut snapshot = base.clone();
    snapshot.revision = app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .revision
        .saturating_add(1);
    snapshot.outbound_ui_preferences = PageData::ready(sink.preferences());
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}
fn settle(app: &mut App) {
    app.update();
    let deadline = Instant::now() + Duration::from_secs(3);
    while matches!(
        &app.world().resource::<ProxySearchState>().phase,
        ProxySearchPhase::Pending { .. }
    ) {
        assert!(Instant::now() < deadline, "typed search must finish");
        app.update();
        yield_now();
    }
}
fn edit(app: &mut App, field: Entity, query: &str) {
    app.world_mut()
        .get_mut::<TextField>(field)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(query.into()));
    app.update();
    settle(app);
}

#[test]
fn native_search_uses_pinyin_preferences_highlights_and_clear_restores_nodes_without_losing_focus()
{
    let (mut app, sink, base) = setup();
    let field = field(&mut app);
    app.world_mut()
        .get_mut::<TextFieldFocused>(field)
        .unwrap()
        .0 = true;
    let initial = app
        .world_mut()
        .query::<&ProxyNodeButton>()
        .iter(app.world())
        .count();
    edit(&mut app, field, "xg");
    assert_eq!(sink.preferences().search_query, "xg");
    publish(&mut app, &sink, &base);
    let nodes: Vec<_> = app
        .world_mut()
        .query::<&ProxyNodeButton>()
        .iter(app.world())
        .map(|node| node.node_name.clone())
        .collect();
    assert!(!nodes.is_empty());
    assert!(nodes.len() < initial);
    assert!(nodes.iter().all(|node| node.contains("香港")));
    let name = entity::<NodeNameText>(&mut app);
    assert!(app.world().get::<TextRuns>(name).is_some_and(|runs| {
        runs.0
            .iter()
            .any(|run| run.highlighted && run.text.contains("香港"))
    }));
    let rendered = app
        .world()
        .get::<TextRuns>(name)
        .unwrap()
        .0
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();
    assert!(nodes.contains(&rendered));
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    app.world_mut()
        .get_mut::<TextField>(field)
        .unwrap()
        .0
        .set_preedit("draft");
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.preedit(),
        "draft"
    );
    assert!(matches!(
        &app.world().resource::<ProxySearchState>().phase,
        ProxySearchPhase::Submitted { query } if query == "xg"
    ));

    let mut refreshed = base.clone();
    let details_snapshot = refreshed.outbound_details.data.as_ref().unwrap();
    let OutboundDetailsState::Ready { details } = details_snapshot.state() else {
        unreachable!()
    };
    let mut rows = details.clone();
    let matching_id = rows
        .iter()
        .find(|row| row.id.value().starts_with("HK-"))
        .unwrap()
        .id
        .clone();
    let matching = rows.iter_mut().find(|row| row.id == matching_id).unwrap();
    matching.current_latency_ms = OutboundObservation::Observed(142);
    refreshed.outbound_details = PageData::ready(
        OutboundDetailsSnapshot::new(
            details_snapshot.scope().clone(),
            SnapshotRevision::new(details_snapshot.detail_revision().value() + 1),
            OutboundDetailsState::Ready { details: rows },
        )
        .unwrap(),
    );
    publish(&mut app, &sink, &refreshed);
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "142 ms")
    );
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "xg");
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);

    edit(&mut app, field, "missing-node");
    publish(&mut app, &sink, &base);
    assert_eq!(
        app.world_mut()
            .query::<&ProxyNodeButton>()
            .iter(app.world())
            .count(),
        0
    );
    let status = entity::<ProxySearchStatus>(&mut app);
    assert_eq!(
        app.world().get::<Text>(status).unwrap().0,
        LocalizedText::plain("proxies_search_empty").render(app.world().resource::<UiLocale>())
    );
    let clear = entity::<ClearProxySearch>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: clear });
    app.update();
    settle(&mut app);
    publish(&mut app, &sink, &base);
    assert!(sink.preferences().search_query.is_empty());
    assert_eq!(
        app.world_mut()
            .query::<&ProxyNodeButton>()
            .iter(app.world())
            .count(),
        initial
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(
        app.world()
            .resource::<LastProxiesProjection>()
            .0
            .as_ref()
            .unwrap()
            .groups[0]
            .current,
        "🇭🇰 香港 HK-01",
        "search and clear preserve the typed selection observation"
    );
}

#[test]
fn native_search_keeps_rejected_draft_ignores_stale_result_and_retries() {
    let (mut app, sink, base) = setup();
    let failure = Failure::new(ErrorCode::Permission, "search denied", false);
    sink.fail_next(failure.clone());
    let field = field(&mut app);
    edit(&mut app, field, "hk");
    assert!(matches!(
        &app.world().resource::<ProxySearchState>().phase,
        ProxySearchPhase::Failed { query, failure: actual }
            if query == "hk" && actual == &failure
    ));
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "hk");
    assert!(sink.preferences().search_query.is_empty());
    app.world_mut().commands().trigger(CommandExecutedEvent {
        request_id: RequestId(999999),
        command: UiCommand::ApplyOutboundUiPreference {
            request: OutboundUiPreferenceRequest::new(
                SnapshotRevision::new(1),
                OutboundUiPreferenceIntent::SetSearchQuery("stale".to_owned()),
            )
            .unwrap(),
        },
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert!(matches!(
        &app.world().resource::<ProxySearchState>().phase,
        ProxySearchPhase::Failed { query, failure: actual }
            if query == "hk" && actual == &failure
    ));
    let retry = entity::<RetryProxySearch>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: retry });
    app.update();
    settle(&mut app);
    publish(&mut app, &sink, &base);
    assert_eq!(sink.preferences().search_query, "hk");
    assert!(matches!(
        &app.world().resource::<ProxySearchState>().phase,
        ProxySearchPhase::Submitted { query } if query == "hk"
    ));
}

#[derive(Resource, Default)]
struct NameChanges(usize);
fn observe_name_changes(
    names: Query<Ref<Text>, With<NodeNameText>>,
    mut changes: ResMut<NameChanges>,
) {
    changes.0 += names.iter().filter(|text| text.is_changed()).count();
}

#[test]
fn idle_highlights_do_not_relayout_text_and_short_spacing_keeps_input_entity() {
    let (mut app, _, _) = setup();
    let input = field(&mut app);
    app.init_resource::<NameChanges>();
    app.add_systems(Update, observe_name_changes.after(sync_name_highlights));
    app.update();
    app.world_mut().resource_mut::<NameChanges>().0 = 0;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<NameChanges>().0, 0);
    let scroll = entity::<ProxiesScrollArea>(&mut app);
    for (height, gap) in [(480.0, 8.0), (780.0, 16.0)] {
        app.world_mut()
            .insert_resource(ResponsiveContext::new(720.0, height));
        app.update();
        assert_eq!(
            app.world().get::<Node>(scroll).unwrap().row_gap,
            Val::Px(gap)
        );
        assert_eq!(field(&mut app), input);
    }
}
