//! Typed batch probe request and receipt behavior for Bevy.
//! test-intent: behavior

use crate::support::{allow_outbound_probe, headless_plugins};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::outbound_batch_probe::{
    OutboundBatchProbeReadback, request_from_snapshot,
};
use infiltrator_bevy_ui::pages::proxies::{TestAllProxiesButton, TestProxyGroupButton};
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::runtime_settings::RuntimeProbeSettingsState;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::OutboundInventorySnapshot;
use lurker_core_contract::outbound::probe::ProbeOutboundBatchRequest;
use lurker_core_contract::outbound::{
    identity::SnapshotRevision,
    inventory::OutboundInventoryState,
    probe::{
        OutboundProbeBatchItem, OutboundProbeBatchOutcome, OutboundProbeBatchScope,
        OutboundProbeBatchSnapshot,
    },
};
use std::sync::Arc;

fn mounted_proxy_page() -> (App, Arc<DemoCommandSink>) {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(
        sink.clone() as Arc<dyn UiCommandSink>
    ));
    app.update();
    allow_outbound_probe(&mut app.world_mut().resource_mut::<LatestSurfaceSnapshot>().0);
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    (app, sink)
}

#[test]
fn selected_probe_request_keeps_explicit_order_and_rejects_collections() {
    let (app, _) = mounted_proxy_page();
    let snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let inventory = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("typed inventory");
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        panic!("demo inventory is ready");
    };
    let concrete_ids = entries
        .iter()
        .filter(|entry| {
            !collections
                .iter()
                .any(|collection| collection.id == entry.id)
        })
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let selected = concrete_ids
        .iter()
        .rev()
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    let request = request_from_snapshot(
        &snapshot,
        OutboundProbeBatchScope::Selected(selected.clone()),
    )
    .expect("selected concrete outbounds remain bound to the accepted snapshot");
    assert_eq!(
        request.target_scope(),
        &OutboundProbeBatchScope::Selected(selected)
    );
    assert_eq!(request.expected_inventory_revision(), inventory.revision());
    let RuntimeProbeSettingsState::Available { settings } = &snapshot.probe_settings else {
        panic!("demo fixture explicitly supplies probe settings");
    };
    assert_eq!(request.test_url(), settings.url());
    assert_eq!(request.timeout_ms(), u64::from(settings.timeout_ms()));

    let mut unavailable_snapshot = snapshot.clone();
    let unavailable = Failure::unsupported("probe settings have not been observed");
    unavailable_snapshot.probe_settings = RuntimeProbeSettingsState::Unavailable {
        failure: unavailable.clone(),
    };
    assert_eq!(
        request_from_snapshot(&unavailable_snapshot, OutboundProbeBatchScope::All)
            .expect_err("unavailable settings must not fall back to defaults"),
        unavailable
    );

    let collection = collections
        .first()
        .expect("demo inventory has a selection collection");
    let failure = request_from_snapshot(
        &snapshot,
        OutboundProbeBatchScope::Selected(vec![collection.id.clone()]),
    )
    .expect_err("collection containers are not selectable probe targets");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

fn click_all(app: &mut App) {
    let button = app
        .world_mut()
        .query_filtered::<Entity, query::With<TestAllProxiesButton>>()
        .single(app.world())
        .expect("all-outbounds probe button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
}

fn click_partial_collection(app: &mut App) -> (RequestId, Vec<OutboundId>) {
    let inventory = app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .outbound_inventory
        .data
        .as_ref()
        .expect("typed inventory")
        .clone();
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        panic!("demo inventory is ready");
    };
    let collection = collections
        .iter()
        .find(|collection| collection.member_ids.len() < entries.len())
        .expect("fixture has a collection with a strict direct-member subset");
    let collection_id = collection.id.clone();
    let direct_members = collection.member_ids.clone();

    let button = app
        .world_mut()
        .query::<(Entity, &TestProxyGroupButton)>()
        .iter(app.world())
        .find(|(_, button)| button.collection_id.as_ref() == Some(&collection_id))
        .map(|(entity, _)| entity)
        .expect("group probe button carries collection ID");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    let command = app
        .world()
        .resource::<OutboundBatchProbeReadback>()
        .pending
        .iter()
        .find(|(_, pending)| {
            pending.request.target_scope()
                == &OutboundProbeBatchScope::Collection(collection_id.clone())
        })
        .expect("typed collection request is pending");
    (*command.0, direct_members)
}

fn receipt(
    snapshot: &SurfaceSnapshot,
    request: &ProbeOutboundBatchRequest,
    items: Vec<OutboundProbeBatchItem>,
) -> OutboundProbeBatchSnapshot {
    let inventory = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("inventory");
    OutboundProbeBatchSnapshot::new(
        snapshot.core.scope().expect("accepted core scope"),
        request.clone(),
        SnapshotRevision::new(inventory.revision().value() + 1),
        items,
    )
    .expect("valid batch receipt")
}

fn deliver_receipt(
    app: &mut App,
    request_id: RequestId,
    request: ProbeOutboundBatchRequest,
    result: OutboundProbeBatchSnapshot,
) {
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::ProbeOutboundBatch { request },
        request_id,
        result: Ok(CommandOutput::OutboundProbeBatch(result)),
    });
    app.update();
}

#[test]
fn all_probe_receipt_keeps_only_concrete_inventory_ids_and_each_typed_outcome() {
    let (mut app, _) = mounted_proxy_page();
    click_all(&mut app);

    let request_id = *app
        .world()
        .resource::<OutboundBatchProbeReadback>()
        .pending
        .keys()
        .next()
        .expect("batch request tracked");
    let pending = app.world().resource::<OutboundBatchProbeReadback>().pending[&request_id].clone();
    assert_eq!(
        pending.request.target_scope(),
        &OutboundProbeBatchScope::All
    );

    let snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("inventory")
        .inventory()
    else {
        panic!("fixture inventory ready");
    };
    let items = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| OutboundProbeBatchItem {
            outbound_id: entry.id.clone(),
            outcome: if index == 0 {
                OutboundProbeBatchOutcome::Failed {
                    failure: Failure::new(ErrorCode::Network, "probe failed", true),
                }
            } else {
                OutboundProbeBatchOutcome::Measured {
                    delay_ms: index as u32,
                }
            },
        })
        .collect::<Vec<_>>();
    assert!(
        collections
            .iter()
            .all(|collection| items.iter().all(|item| item.outbound_id != collection.id))
    );
    let expected_ids: Vec<_> = entries.iter().map(|entry| entry.id.clone()).collect();
    let result = receipt(&snapshot, &pending.request, items);
    deliver_receipt(&mut app, request_id, pending.request, result.clone());

    let readback = app.world().resource::<OutboundBatchProbeReadback>();
    let actual = readback
        .last_result
        .as_ref()
        .expect("typed receipt retained");
    assert_eq!(
        actual
            .items()
            .iter()
            .map(|item| item.outbound_id.clone())
            .collect::<Vec<_>>(),
        expected_ids
    );
    assert!(matches!(
        actual.items()[0].outcome,
        OutboundProbeBatchOutcome::Failed { .. }
    ));
    assert_eq!(actual, &result);
}

#[test]
fn collection_probe_request_and_receipt_preserve_only_direct_members() {
    let (mut app, _) = mounted_proxy_page();
    let (request_id, direct_members) = click_partial_collection(&mut app);
    let pending = app.world().resource::<OutboundBatchProbeReadback>().pending[&request_id].clone();
    let OutboundProbeBatchScope::Collection(collection_id) = pending.request.target_scope() else {
        panic!("group action targets a collection");
    };
    assert!(direct_members.iter().all(|member| member != collection_id));
    let snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let items = direct_members
        .iter()
        .cloned()
        .map(|outbound_id| OutboundProbeBatchItem {
            outbound_id,
            outcome: OutboundProbeBatchOutcome::Unsupported,
        })
        .collect::<Vec<_>>();
    let result = receipt(&snapshot, &pending.request, items);
    deliver_receipt(&mut app, request_id, pending.request, result);

    let readback = app.world().resource::<OutboundBatchProbeReadback>();
    let result = readback
        .last_result
        .as_ref()
        .expect("typed receipt retained");
    assert_eq!(
        result
            .items()
            .iter()
            .map(|item| item.outbound_id.clone())
            .collect::<Vec<_>>(),
        direct_members
    );
    assert!(
        result
            .items()
            .iter()
            .all(|item| item.outcome == OutboundProbeBatchOutcome::Unsupported)
    );
}

#[test]
fn stale_revision_receipt_is_discarded_after_inventory_advances() {
    let (mut app, _) = mounted_proxy_page();
    click_all(&mut app);
    let (request_id, pending) = {
        let readback = app.world().resource::<OutboundBatchProbeReadback>();
        let (id, pending) = readback.pending.iter().next().expect("pending request");
        (*id, pending.clone())
    };
    let original = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let stale_result = receipt(&original, &pending.request, Vec::new());
    {
        let mut latest = app.world_mut().resource_mut::<LatestSurfaceSnapshot>();
        let inventory = latest
            .0
            .outbound_inventory
            .data
            .as_ref()
            .expect("inventory")
            .clone();
        latest.0.outbound_inventory = PageData::ready(
            OutboundInventorySnapshot::new(
                inventory.scope().clone(),
                SnapshotRevision::new(inventory.revision().value() + 1),
                inventory.inventory().clone(),
            )
            .expect("advanced revision keeps inventory valid"),
        );
    }
    deliver_receipt(&mut app, request_id, pending.request, stale_result);
    let readback = app.world().resource::<OutboundBatchProbeReadback>();
    assert!(readback.last_result.is_none());
    assert!(readback.last_failure.is_none());
}

#[test]
fn late_receipts_after_page_close_or_newer_request_do_not_resurrect_results() {
    let (mut app, _) = mounted_proxy_page();
    click_all(&mut app);
    let (first_id, first) = {
        let readback = app.world().resource::<OutboundBatchProbeReadback>();
        let (id, pending) = readback
            .pending
            .iter()
            .next()
            .expect("first pending request");
        (*id, pending.clone())
    };
    click_all(&mut app);
    let second_id = app
        .world()
        .resource::<OutboundBatchProbeReadback>()
        .last_request_id
        .expect("second request became current");
    assert_ne!(first_id, second_id);
    let current = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let stale_out_of_order = receipt(&current, &first.request, Vec::new());
    deliver_receipt(
        &mut app,
        first_id,
        first.request.clone(),
        stale_out_of_order,
    );
    assert!(
        app.world()
            .resource::<OutboundBatchProbeReadback>()
            .last_result
            .is_none()
    );

    let second = app.world().resource::<OutboundBatchProbeReadback>().pending[&second_id].clone();
    let closed_result = receipt(&current, &second.request, Vec::new());
    app.world_mut().resource_mut::<ActiveRoute>().0 = Some(Route::Settings);
    deliver_receipt(&mut app, second_id, second.request, closed_result);
    let readback = app.world().resource::<OutboundBatchProbeReadback>();
    assert!(readback.last_result.is_none());
    assert!(readback.last_failure.is_none());
}
