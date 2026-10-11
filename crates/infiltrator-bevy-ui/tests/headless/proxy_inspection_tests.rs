//! test-intent: behavior
//! Inspector behavior is driven only by engine-neutral outbound observations.
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::proxies::NodeDetailButton;
use infiltrator_bevy_ui::pages::proxy_inspection;
use infiltrator_bevy_ui::pages::proxy_inspection::ProxyDetailField;
use infiltrator_bevy_ui::pages::proxy_inspection::ProxyDetailValue;
use infiltrator_bevy_ui::pages::proxy_inspection::{
    CloseProxyInspection, ProxyInspectionCard, ProxyInspectionChart, ProxyInspectionPhase,
    ProxyInspectionReadPhase, ProxyInspectionReadState, ProxyInspectionRoot,
    ProxyInspectionSession, ProxyInspectionState, ProxyInspectionTitle, ProxyProbePhase,
};
use infiltrator_bevy_ui::pages::proxy_probe::ProbeInspectedProxy;
use infiltrator_bevy_ui::projection::OverviewProjection;
use infiltrator_bevy_ui::projection::OverviewSource;
use infiltrator_bevy_ui::projection::SourceKind;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::overview_projection;
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_bevy_widgets::chart::bezier::ScaleMode;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreCapabilitySnapshot, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision, OutboundOperation,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::core_identity::{CoreScope, CoreVersion};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::OutboundProbeAvailability;
use infiltrator_contract::outbound::preferences::OutboundLayout;
use infiltrator_contract::outbound::{
    details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundEgressSample,
        OutboundEndpoint, OutboundFeature, OutboundHealthObservation, OutboundLatencySample,
        OutboundObservation, OutboundProtocolObservation, OutboundRttStatistics, OutboundSnapshot,
        OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot,
        OutboundProbeState, OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundHealthFilter, OutboundSortOrder, OutboundUiPreferencesSnapshot,
        OutboundUiPreferencesValues,
    },
    probe::ProbeOutboundRequest,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use mem::take;
use std::collections::VecDeque;
use std::mem;
use std::sync::{Arc, Mutex};

const INSPECTION_GROUP: &str = "GLOBAL";
const INSPECTION_NODE: &str = "HK-Inspect";
const OTHER_NODE: &str = "Other-Node";

fn inspection_session(state: &ProxyInspectionState) -> Option<&ProxyInspectionSession> {
    match &state.phase {
        ProxyInspectionPhase::Closed => None,
        ProxyInspectionPhase::Open(session) => Some(session),
    }
}

fn selected_outbound(state: &ProxyInspectionState) -> Option<&OutboundId> {
    inspection_session(state).map(|session| &session.selected)
}

fn inspection_read(state: &ProxyInspectionState) -> Option<&ProxyInspectionReadState> {
    inspection_session(state).map(|session| &session.read)
}

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

#[derive(Clone)]
struct QueuedProbe {
    request_id: RequestId,
    request: ProbeOutboundRequest,
    outcome: Result<OutboundProbeState, Failure>,
}

#[derive(Default)]
struct ProbeSinkState {
    commands: Vec<UiCommand>,
    events: Vec<CommandExecutedEvent>,
    pending: VecDeque<QueuedProbe>,
    outcomes: VecDeque<Result<OutboundProbeState, Failure>>,
    scope: Option<CoreScope>,
    next_request_id: u64,
    next_detail_revision: u64,
}

#[derive(Clone, Default)]
struct ProbeSink(Arc<Mutex<ProbeSinkState>>);
impl ProbeSink {
    fn initialize(&self, scope: CoreScope) {
        let mut state = self.0.lock().unwrap();
        state.scope = Some(scope);
        state.next_detail_revision = 3;
    }
    fn fail_next(&self, failure: Failure) {
        self.0.lock().unwrap().outcomes.push_back(Err(failure));
    }
    fn commands(&self) -> Vec<UiCommand> {
        self.0.lock().unwrap().commands.clone()
    }
    fn complete_all(&self) {
        let mut state = self.0.lock().unwrap();
        while let Some(pending) = state.pending.pop_front() {
            let result = match pending.outcome {
                Err(failure) => Err(failure),
                Ok(probe_state) => {
                    let scope = state.scope.clone().expect("probe fixture scope is set");
                    let revision = SnapshotRevision::new(state.next_detail_revision);
                    state.next_detail_revision += 1;
                    OutboundProbeSnapshot::new(
                        scope,
                        pending.request.expected_inventory_revision(),
                        revision,
                        pending.request.outbound_id().clone(),
                        probe_state,
                    )
                    .map(CommandOutput::OutboundProbe)
                    .map_err(|error| {
                        Failure::new(ErrorCode::InvalidState, error.to_string(), false)
                    })
                }
            };
            state.events.push(CommandExecutedEvent {
                command: UiCommand::ProbeOutbound {
                    request: pending.request,
                },
                request_id: pending.request_id,
                result,
            });
        }
    }
}
impl UiCommandSink for ProbeSink {
    fn submit(&self, command: UiCommand) {
        self.0.lock().unwrap().commands.push(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let UiCommand::ProbeOutbound { request } = command.clone() else {
            self.0.lock().unwrap().commands.push(command);
            return None;
        };
        let mut state = self.0.lock().unwrap();
        state.commands.push(command);
        state.next_request_id += 1;
        let request_id = RequestId(state.next_request_id);
        let outcome = state
            .outcomes
            .pop_front()
            .unwrap_or(Ok(OutboundProbeState::Measured { delay_ms: 23 }));
        state.pending.push_back(QueuedProbe {
            request_id,
            request,
            outcome,
        });
        Some(request_id)
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        take(&mut self.0.lock().unwrap().events)
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
    let mut snapshot = snapshot;
    snapshot.revision = app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .revision
        .saturating_add(1);
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}

fn typed_snapshot() -> SurfaceSnapshot {
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let version = CoreVersion::new("1.19.32").unwrap();
    snapshot.core.identity.as_mut().unwrap().version = Some(version.clone());
    snapshot.core.core_version = Some(version.raw().to_owned());
    let scope = snapshot.core.scope().expect("demo scope is accepted");
    let engine = scope.identity.engine.clone();
    let target = OutboundId::new(engine.clone(), INSPECTION_NODE).unwrap();
    let other = OutboundId::new(engine.clone(), OTHER_NODE).unwrap();
    let collection = OutboundId::new(engine.clone(), INSPECTION_GROUP).unwrap();
    let entries = vec![
        OutboundSnapshot {
            id: target.clone(),
            display_name: Some(INSPECTION_NODE.to_owned()),
        },
        OutboundSnapshot {
            id: other.clone(),
            display_name: Some(OTHER_NODE.to_owned()),
        },
    ];
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(7),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![OutboundSelectionSet {
                id: collection.clone(),
                member_ids: vec![target.clone(), other.clone()],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: target.clone(),
                },
            }],
        },
    )
    .unwrap();
    let details = details_snapshot(
        &scope,
        2,
        vec![detail(&engine, target, 42), detail(&engine, other, 27)],
    );
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: Vec::new(),
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::LatencyAscending,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::Grid,
        },
    )
    .unwrap();
    snapshot.outbound_inventory = PageData::ready(inventory);
    snapshot.outbound_details = PageData::ready(details);
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    let capability_scope = CoreCapabilityScope::new(
        scope.identity.engine.clone(),
        scope.identity.version.clone(),
        HostKind::Desktop,
        AdapterRevision::new(1),
    );
    let capability_matrix = CoreCapabilityMatrix::new(
        capability_scope,
        vec![CoreCapabilityEntry::new(
            CoreOperation::Outbound(OutboundOperation::Probe),
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .unwrap();
    snapshot.core_capability_snapshot =
        PageData::ready(CoreCapabilitySnapshot::from_matrix(&capability_matrix));
    snapshot
}

fn detail(engine: &EngineId, id: OutboundId, delay: u32) -> OutboundDetail {
    OutboundDetail {
        id,
        protocol: OutboundProtocolObservation::Reported(
            OutboundProtocolId::new(engine.clone(), "shadowsocks").unwrap(),
        ),
        protocol_display_name: Some("Shadowsocks".to_owned()),
        cipher: OutboundObservation::Observed(
            OutboundCipherId::new(engine.clone(), "chacha20-ietf-poly1305").unwrap(),
        ),
        cipher_display_name: Some("chacha20-ietf-poly1305".to_owned()),
        collection_kind: OutboundObservation::Unknown,
        features: OutboundObservation::Observed(vec![OutboundFeature::Udp]),
        endpoint: OutboundObservation::Observed(OutboundEndpoint {
            host: "198.51.100.12".to_owned(),
            port: OutboundObservation::Observed(443),
        }),
        udp: OutboundUdpSupport::Supported,
        health: OutboundHealthObservation::Healthy,
        current_latency_ms: OutboundObservation::Observed(delay),
        latency_history: OutboundObservation::Observed(vec![
            OutboundLatencySample {
                observed_at: "2026-10-10T09:00:00Z".to_owned(),
                delay_ms: 18,
            },
            OutboundLatencySample {
                observed_at: "2026-10-10T09:01:00Z".to_owned(),
                delay_ms: 0,
            },
            OutboundLatencySample {
                observed_at: "2026-10-10T09:02:00Z".to_owned(),
                delay_ms: delay,
            },
        ]),
        rtt: OutboundObservation::Observed(OutboundRttStatistics {
            min_ms: Some(18),
            max_ms: Some(delay),
            avg_ms: Some((18 + delay) / 2),
            valid_count: 2,
        }),
        historical_egress: OutboundObservation::Observed(vec![OutboundEgressSample {
            ip: "203.0.113.8".to_owned(),
            country: Some("JP".to_owned()),
            observed_at_epoch_ms: Some(1_760_000_000_000),
        }]),
        probe_availability: OutboundProbeAvailability::Supported,
    }
}

fn details_snapshot(
    scope: &CoreScope,
    revision: u64,
    details: Vec<OutboundDetail>,
) -> OutboundDetailsSnapshot {
    OutboundDetailsSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(revision),
        OutboundDetailsState::Ready { details },
    )
    .unwrap()
}

fn setup_with_snapshot(snapshot: SurfaceSnapshot) -> (App, ProbeSink) {
    let scope = snapshot.core.scope().unwrap();
    let sink = ProbeSink::default();
    sink.initialize(scope);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        ThemePreference::Fixed(ThemeSkin::Dark),
        1180.0,
    ));
    app.add_plugins(PagesPlugin::new_surface(Source(snapshot)));
    app.add_plugins(CommandPumpPlugin::new(Arc::new(sink.clone())));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    (app, sink)
}
fn setup() -> (App, ProbeSink) {
    setup_with_snapshot(typed_snapshot())
}
fn open(app: &mut App) {
    let button = app
        .world_mut()
        .query::<(Entity, &NodeDetailButton)>()
        .iter(app.world())
        .find(|(_, button)| {
            button
                .outbound_id
                .as_ref()
                .is_some_and(|id| id.value() == INSPECTION_NODE)
        })
        .unwrap()
        .0;
    activate(app, button);
}
fn replace_details(snapshot: &mut SurfaceSnapshot, details: Vec<OutboundDetail>, revision: u64) {
    let scope = snapshot.core.scope().unwrap();
    snapshot.outbound_details = PageData::ready(details_snapshot(&scope, revision, details));
}
fn field_text(app: &mut App, field: ProxyDetailField) -> String {
    app.world_mut()
        .query::<(&ProxyDetailValue, &Text)>()
        .iter(app.world())
        .find(|(marker, _)| marker.0 == field)
        .unwrap()
        .1
        .0
        .clone()
}

#[test]
fn native_inspection_replays_typed_metadata_and_history_and_close_never_selects_an_outbound() {
    let (mut app, sink) = setup();
    open(&mut app);
    let selected = app.world().resource::<ProxyInspectionState>();
    let selected = selected_outbound(selected).cloned().unwrap();
    assert_eq!(selected.value(), INSPECTION_NODE);
    let root = entity::<ProxyInspectionRoot>(&mut app);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::Flex
    );
    let title = entity::<ProxyInspectionTitle>(&mut app);
    assert_eq!(app.world().get::<Text>(title).unwrap().0, INSPECTION_NODE);
    let chart = entity::<ProxyInspectionChart>(&mut app);
    let plate = &app.world().get::<ChartPlate>(chart).unwrap().0;
    assert_eq!(plate.scale_mode, ScaleMode::Fixed(42.0));
    assert_eq!(plate.up.len(), 3);
    assert_eq!(plate.up[0], 18.0);
    assert!(plate.up[1].is_nan());
    assert_eq!(plate.up[2], 42.0);
    assert_eq!(
        field_text(&mut app, ProxyDetailField::Protocol),
        "Shadowsocks"
    );
    assert_eq!(
        field_text(&mut app, ProxyDetailField::Server),
        "198.51.100.12"
    );
    assert_eq!(field_text(&mut app, ProxyDetailField::Port), "443");
    assert_eq!(
        field_text(&mut app, ProxyDetailField::Cipher),
        "chacha20-ietf-poly1305"
    );
    assert_eq!(field_text(&mut app, ProxyDetailField::Delay), "42 ms");
    assert_eq!(field_text(&mut app, ProxyDetailField::Udp), "Supported");
    assert_eq!(
        field_text(&mut app, ProxyDetailField::Health),
        "Kernel reports alive"
    );
    assert_eq!(field_text(&mut app, ProxyDetailField::RttMin), "18 ms");
    assert_eq!(field_text(&mut app, ProxyDetailField::RttMax), "42 ms");
    assert_eq!(field_text(&mut app, ProxyDetailField::RttAvg), "30 ms");
    assert_eq!(
        field_text(&mut app, ProxyDetailField::Egress),
        "203.0.113.8 (JP)"
    );
    assert_eq!(
        field_text(&mut app, ProxyDetailField::EgressTime),
        "1760000000000 ms"
    );
    let history = app
        .world_mut()
        .query::<(&proxy_inspection::ProxyHistoryText, &Text)>()
        .iter(app.world())
        .next()
        .unwrap()
        .1
        .0
        .clone();
    assert!(history.contains("3/3"));
    assert!(history.contains("2026-10-10T09:01:00Z"));
    assert!(sink.commands().is_empty());
    let close = entity::<CloseProxyInspection>(&mut app);
    activate(&mut app, close);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    assert!(selected_outbound(app.world().resource::<ProxyInspectionState>()).is_none());
    assert!(sink.commands().is_empty());
}

#[test]
fn native_inspection_keeps_identity_on_inventory_reorder_filter_and_refresh_then_closes_on_removal()
{
    let (mut app, sink) = setup();
    open(&mut app);
    let selected = app.world().resource::<ProxyInspectionState>();
    let selected = selected_outbound(selected).cloned();
    let card = entity::<ProxyInspectionCard>(&mut app);
    let title = entity::<ProxyInspectionTitle>(&mut app);
    let chart = entity::<ProxyInspectionChart>(&mut app);
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let inventory = snapshot.outbound_inventory.data.as_ref().unwrap();
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        panic!("typed inventory fixture is ready")
    };
    let mut reversed = collections.clone();
    reversed.reverse();
    snapshot.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            inventory.scope().clone(),
            SnapshotRevision::new(inventory.revision().value() + 1),
            OutboundInventoryState::Ready {
                entries: entries.clone(),
                collections: reversed,
            },
        )
        .unwrap(),
    );
    let mut preferences = snapshot.outbound_ui_preferences.data.clone().unwrap();
    preferences = OutboundUiPreferencesSnapshot::new(
        preferences.scope().clone(),
        SnapshotRevision::new(preferences.revision().value() + 1),
        OutboundUiPreferencesValues {
            search_query: "no-match".to_owned(),
            favorite_outbound_ids: preferences.favorite_outbound_ids,
            group_order: preferences.group_order.iter().cloned().rev().collect(),
            collapsed_group_ids: preferences.collapsed_group_ids,
            sort_order: preferences.sort_order,
            health_filter: preferences.health_filter,
            layout: preferences.layout,
        },
    )
    .unwrap();
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    let detail_snapshot = snapshot.outbound_details.data.as_ref().unwrap();
    let detail_revision = detail_snapshot.detail_revision().value() + 1;
    let OutboundDetailsState::Ready { details: rows } = detail_snapshot.state() else {
        panic!("typed details fixture is ready")
    };
    let refreshed = rows
        .iter()
        .map(|row| {
            if row.id.value() == INSPECTION_NODE {
                detail(row.id.engine_id(), row.id.clone(), 91)
            } else {
                row.clone()
            }
        })
        .collect();
    replace_details(&mut snapshot, refreshed, detail_revision);
    publish(&mut app, snapshot.clone());
    assert_eq!(
        selected_outbound(app.world().resource::<ProxyInspectionState>()).cloned(),
        selected
    );
    assert!(app.world().get::<ProxyInspectionCard>(card).is_some());
    assert_eq!(app.world().get::<Text>(title).unwrap().0, INSPECTION_NODE);
    assert!(app.world().get::<ChartPlate>(chart).is_some());
    assert_eq!(field_text(&mut app, ProxyDetailField::Delay), "91 ms");

    let current_inventory = snapshot.outbound_inventory.data.as_ref().unwrap();
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = current_inventory.inventory()
    else {
        unreachable!()
    };
    let remaining = OutboundId::new(
        current_inventory.scope().identity.engine.clone(),
        OTHER_NODE,
    )
    .unwrap();
    let mut removed = snapshot.clone();
    removed.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            current_inventory.scope().clone(),
            SnapshotRevision::new(current_inventory.revision().value() + 1),
            OutboundInventoryState::Ready {
                entries: entries
                    .iter()
                    .filter(|entry| entry.id == remaining)
                    .cloned()
                    .collect(),
                collections: collections
                    .iter()
                    .map(|collection| OutboundSelectionSet {
                        id: collection.id.clone(),
                        member_ids: vec![remaining.clone()],
                        availability: collection.availability,
                        selection: OutboundSelectionState::Ready {
                            selected: remaining.clone(),
                        },
                    })
                    .collect(),
            },
        )
        .unwrap(),
    );
    let detail_snapshot = removed.outbound_details.data.as_ref().unwrap();
    let detail_revision = detail_snapshot.detail_revision().value() + 1;
    let OutboundDetailsState::Ready { details: rows } = detail_snapshot.state() else {
        unreachable!()
    };
    let remaining_rows = rows
        .iter()
        .filter(|row| row.id == remaining)
        .cloned()
        .collect();
    replace_details(&mut removed, remaining_rows, detail_revision);
    publish(&mut app, removed);
    assert!(selected_outbound(app.world().resource::<ProxyInspectionState>()).is_none());
    let root = entity::<ProxyInspectionRoot>(&mut app);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    assert!(sink.commands().is_empty());
}

#[test]
fn native_probe_preserves_typed_failure_retries_with_revision_and_ignores_late_results_after_close()
{
    let (mut app, sink) = setup();
    open(&mut app);
    let probe = entity::<ProbeInspectedProxy>(&mut app);
    assert!(!app.world().get::<ButtonDisabled>(probe).unwrap().0);
    let failure = Failure::new(ErrorCode::Network, "probe transport failed", true);
    sink.fail_next(failure.clone());
    activate(&mut app, probe);
    assert!(matches!(
        &inspection_session(app.world().resource::<ProxyInspectionState>())
            .expect("inspection remains open while probing")
            .probe,
        ProxyProbePhase::Pending(_)
    ));
    sink.complete_all();
    app.update();
    assert!(matches!(
        &inspection_session(app.world().resource::<ProxyInspectionState>())
            .expect("failed probe keeps inspection open")
            .probe,
        ProxyProbePhase::Failed(actual) if actual == &failure
    ));
    activate(&mut app, probe);
    sink.complete_all();
    app.update();
    assert!(matches!(
        &inspection_session(app.world().resource::<ProxyInspectionState>())
            .expect("successful retry keeps inspection open")
            .probe,
        ProxyProbePhase::Idle
    ));
    let commands = sink.commands();
    assert_eq!(commands.len(), 2);
    for command in &commands {
        let UiCommand::ProbeOutbound { request } = command else {
            panic!("inspector must submit only the typed leaf probe")
        };
        assert_eq!(request.outbound_id().value(), INSPECTION_NODE);
        assert_eq!(
            request.expected_inventory_revision(),
            SnapshotRevision::new(7)
        );
        assert_eq!(request.test_url(), "http://www.gstatic.com/generate_204");
        assert_eq!(request.timeout_ms(), 5000);
    }
    assert!(
        commands
            .iter()
            .all(|command| !matches!(command, UiCommand::SelectOutbound { .. }))
    );

    activate(&mut app, probe);
    assert!(matches!(
        &inspection_session(app.world().resource::<ProxyInspectionState>())
            .expect("pending probe keeps inspection open")
            .probe,
        ProxyProbePhase::Pending(_)
    ));
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.insert_resource(keys);
    app.update();
    assert!(selected_outbound(app.world().resource::<ProxyInspectionState>()).is_none());
    sink.complete_all();
    app.update();
    assert_eq!(
        app.world().resource::<ProxyInspectionState>().phase,
        ProxyInspectionPhase::Closed
    );
}

#[test]
fn native_probe_remains_disabled_without_an_owner_capability_snapshot() {
    let mut snapshot = typed_snapshot();
    snapshot.core_capability_snapshot =
        PageData::unavailable(Failure::unsupported("probe capability was not observed"));
    let (mut app, sink) = setup_with_snapshot(snapshot);
    open(&mut app);
    let probe = entity::<ProbeInspectedProxy>(&mut app);
    app.update();

    assert!(app.world().get::<ButtonDisabled>(probe).unwrap().0);
    activate(&mut app, probe);
    assert!(sink.commands().is_empty());
}

#[path = "proxy_inspection_removal_tests.rs"]
mod removal_tests;
