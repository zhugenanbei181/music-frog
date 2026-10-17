//! Behavior cases for proxies.
//! test-intent: behavior

use super::*;
use crate::support::allow_outbound_probe;
use bevy::ecs::query::With;
use bevy::ui::prelude;
use bevy::ui::prelude::Val;
use infiltrator_bevy_ui::pages::proxies::{
    LatencySkeletonPulse, NodeDetailButton, ProxyGroupMoveUpButton, ProxyNode,
    ResetProxyGroupOrderButton, ToggleViewModeButton,
};
use infiltrator_bevy_ui::pages::proxies_filter::{format_protocol_chip, matches_proxy_filter};
use infiltrator_bevy_ui::pages::proxies_virtual::ProxiesVirtualNodes;
use infiltrator_bevy_ui::pages::proxy_group_order::{GroupOrderAction, GroupOrderState};
use infiltrator_bevy_ui::pages::proxy_probe_settings::{
    OpenProbeSettings, ProbeTimeoutField, ProbeUrlField,
};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::fluid_grid::FluidCardGrid;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::latency_projection::project_proxy_latency;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::details::OutboundHealthObservation;
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceRequest;
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use lurker_core_contract::outbound::{
    details::OutboundSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundFavoriteState, OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout,
        OutboundSortOrder, OutboundUiPreferenceIntent,
    },
    probe::OutboundSelectionRequest,
};

fn set_outbound_inventory(
    snapshot: &mut SurfaceSnapshot,
    collection_name: &str,
    outbound_names: Vec<String>,
    availability: SelectionAvailability,
    selected_name: Option<&str>,
    inventory_revision: u64,
) {
    let scope = snapshot
        .core
        .scope()
        .expect("demo snapshot has an accepted core scope");
    let engine = scope.identity.engine.clone();
    let outbound_ids: Vec<_> = outbound_names
        .iter()
        .map(|name| OutboundId::new(engine.clone(), name.clone()).expect("valid outbound ID"))
        .collect();
    let entries = outbound_ids
        .iter()
        .zip(&outbound_names)
        .map(|(id, name)| OutboundSnapshot {
            id: id.clone(),
            display_name: Some(name.clone()),
        })
        .collect();
    let selection = selected_name
        .map(|name| OutboundSelectionState::Ready {
            selected: OutboundId::new(engine.clone(), name).expect("valid selected ID"),
        })
        .unwrap_or(OutboundSelectionState::Unknown);
    let inventory = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(inventory_revision),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![OutboundSelectionSet {
                id: OutboundId::new(engine, collection_name).expect("valid collection ID"),
                member_ids: outbound_ids,
                availability,
                selection,
            }],
        },
    )
    .expect("test outbound inventory is valid");
    snapshot.outbound_inventory = PageData::ready(inventory);
}

#[test]
fn test_proxies_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);
    // TextRuns are replayed on the next frame after the route scene is spawned.
    app.update();
    let locale = app.world().resource::<UiLocale>().code().to_string();
    let lang = Lang(&locale);

    assert!(subtree_has_text(
        app.world(),
        root,
        "代理策略 · 共 3 个策略组 (8 个节点)"
    ));
    assert!(subtree_has_text(app.world(), root, "HK-01"));
    assert!(subtree_has_text(app.world(), root, "测速就绪"));
    assert!(subtree_has_text(app.world(), root, "PROXIES"));
    assert!(subtree_has_text(app.world(), root, "尚未测速"));
    assert!(subtree_has_text(
        app.world(),
        root,
        Lang("zh-CN").tr("proxies_search_placeholder").as_ref()
    ));
    assert!(subtree_has_text(app.world(), root, "组测速"));
    assert!(subtree_has_text(
        app.world(),
        root,
        lang.tr("custom_node_title").as_ref()
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        lang.tr("custom_node_btn_import_uri").as_ref()
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        lang.tr("btn_save").as_ref()
    ));
    // DUAL-05: the shared studio keeps the card honest before any draft.
    assert!(subtree_has_text(app.world(), root, "尚无节点草稿"));
    assert!(subtree_has_text(
        app.world(),
        root,
        lang.tr("custom_node_name").as_ref()
    ));

    // Enhanced toolbar & controls parity with Iced
    assert!(subtree_has_text(app.world(), root, "只看可用"));
    assert!(subtree_has_text(app.world(), root, "延迟升序"));
    assert!(subtree_has_text(app.world(), root, "延迟降序"));
    assert!(subtree_has_text(app.world(), root, "名称升序"));
    assert!(subtree_has_text(app.world(), root, "名称降序"));
    assert!(subtree_has_text(
        app.world(),
        root,
        lang.tr("proxy_probe_settings_title").as_ref()
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        Lang("zh-CN").tr("proxies_add_node_btn").as_ref()
    ));
    assert!(subtree_has_text(app.world(), root, "网格视图"));
    // Inventory names and selection state come from the engine-neutral read.
    assert!(subtree_has_text(app.world(), root, "JP-02"));

    // Component markers verification
    assert!(
        app.world_mut()
            .query::<&FilterAliveToggle>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&ProxySortPill>()
            .iter(app.world())
            .count()
            >= 4
    );
    assert!(
        app.world_mut()
            .query::<&OpenProbeSettings>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert_eq!(
        app.world_mut()
            .query::<&ProbeUrlField>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&ProbeTimeoutField>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world_mut()
            .query::<&AddCustomNodeButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&ToggleViewModeButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
}

#[test]
fn test_proxies_node_cards_reflow_across_tiers() {
    // Proxy node cards must track the shared tier column operator, not a fixed
    // 49% width. Sizes chosen to land squarely inside each tier band.
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Proxies);

    let node_width = |app: &mut App| -> Val {
        let mut query = app
            .world_mut()
            .query::<(&prelude::Node, &ProxyNodeButton)>();
        query
            .iter(app.world())
            .next()
            .map(|(node, _)| node.width)
            .expect("at least one proxy node card")
    };

    let cases = [(400.0_f32, 1usize), (700.0, 2), (1000.0, 3), (1600.0, 4)];
    for (width, columns) in cases {
        app.world_mut()
            .resource_mut::<ResponsiveContext>()
            .set_dimensions(width, 900.0);
        app.update();
        let expected = Val::Percent(FluidCardGrid::wrapped_item_percent(columns));
        assert_eq!(
            node_width(&mut app),
            expected,
            "proxy node width at {width}px should reflect {columns} columns"
        );
    }
}

#[test]
fn test_proxies_test_all_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);
    allow_outbound_probe(&mut app.world_mut().resource_mut::<LatestSurfaceSnapshot>().0);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<TestAllProxiesButton>>()
        .single(app.world())
        .expect("test all proxies button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    let submitted = sink.submitted();
    let [UiCommand::ProbeOutboundBatch { request }] = submitted.as_slice() else {
        panic!("expected one typed outbound batch request, got {submitted:?}");
    };
    assert_eq!(request.target_scope(), &OutboundProbeBatchScope::All);
    assert_eq!(
        request.expected_inventory_revision(),
        SnapshotRevision::new(1)
    );
}

#[test]
fn test_proxies_test_group_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);
    allow_outbound_probe(&mut app.world_mut().resource_mut::<LatestSurfaceSnapshot>().0);

    let mut query = app.world_mut().query::<(Entity, &TestProxyGroupButton)>();
    let (btn_entity, collection_id) = query
        .iter(app.world())
        .find(|(_, btn)| btn.group_idx == 0)
        .map(|(entity, button)| {
            (
                entity,
                button
                    .collection_id
                    .clone()
                    .expect("group test carries an opaque collection ID"),
            )
        })
        .expect("test proxy group button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    let submitted = sink.submitted();
    let [UiCommand::ProbeOutboundBatch { request }] = submitted.as_slice() else {
        panic!("expected one typed outbound batch request, got {submitted:?}");
    };
    assert_eq!(
        request.target_scope(),
        &OutboundProbeBatchScope::Collection(collection_id)
    );
    assert_eq!(
        request.expected_inventory_revision(),
        SnapshotRevision::new(1)
    );
}

#[test]
fn stale_proxy_group_button_does_not_submit_after_collection_is_replaced() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);
    allow_outbound_probe(&mut app.world_mut().resource_mut::<LatestSurfaceSnapshot>().0);
    let mut buttons = app.world_mut().query::<(Entity, &TestProxyGroupButton)>();
    let button = buttons
        .iter(app.world())
        .find(|(_, button)| button.group_idx == 0)
        .map(|(entity, _)| entity)
        .expect("group probe button");

    let mut latest = app.world_mut().resource_mut::<LatestSurfaceSnapshot>();
    let mut replacement = latest.0.clone();
    set_outbound_inventory(
        &mut replacement,
        "REPLACED",
        vec!["HK-01".to_owned(), "JP-02".to_owned()],
        SelectionAvailability::Selectable,
        Some("HK-01"),
        2,
    );
    // bevy 0.20 `Mut` carries no Drop side effects, so ending the borrow
    // here needs no explicit drop.
    latest.0 = replacement;

    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "stale collection identity must not produce a batch request"
    );
}

#[test]
fn test_proxies_select_node_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.core.revision = snapshot.revision;
    set_outbound_inventory(
        &mut snapshot,
        "PROXIES",
        vec!["HK-01".to_owned(), "JP-02".to_owned()],
        SelectionAvailability::Selectable,
        Some("HK-01"),
        42,
    );
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();

    let mut query = app.world_mut().query::<(Entity, &ProxyNodeButton)>();
    let (node_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.group_name == "PROXIES" && btn.node_name == "JP-02")
        .expect("target proxy node button");

    app.world_mut().commands().trigger(Activate {
        entity: node_entity,
    });
    app.update();

    let request = OutboundSelectionRequest::new(
        OutboundId::new(EngineId::new("mihomo").unwrap(), "PROXIES").unwrap(),
        OutboundId::new(EngineId::new("mihomo").unwrap(), "JP-02").unwrap(),
        SnapshotRevision::new(42),
    )
    .unwrap();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SelectOutbound { request }]
    );
}

#[test]
fn test_proxies_read_only_inventory_members_cannot_submit_selection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.core.revision = snapshot.revision;
    set_outbound_inventory(
        &mut snapshot,
        "READ-ONLY",
        vec!["node-a".to_owned(), "node-b".to_owned()],
        SelectionAvailability::ReadOnly,
        Some("node-a"),
        7,
    );
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();

    let node_entity = app
        .world_mut()
        .query::<(Entity, &ProxyNodeButton)>()
        .iter(app.world())
        .find(|(_, button)| button.group_name == "READ-ONLY" && button.node_name == "node-b")
        .map(|(entity, _)| entity)
        .expect("read-only inventory member is still displayed");
    app.world_mut().commands().trigger(Activate {
        entity: node_entity,
    });
    app.update();

    assert!(sink.submitted().is_empty());
}

#[test]
fn test_proxies_inventory_projection_uses_the_typed_snapshot() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.core.revision = snapshot.revision;
    set_outbound_inventory(
        &mut snapshot,
        "NEUTRAL-COLLECTION",
        vec!["neutral-node-a".to_owned(), "neutral-node-b".to_owned()],
        SelectionAvailability::Selectable,
        Some("neutral-node-a"),
        5,
    );
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();

    assert!(subtree_has_text(app.world(), root, "NEUTRAL-COLLECTION"));
    assert!(subtree_has_text(app.world(), root, "neutral-node-a"));
}

#[test]
fn test_proxies_swipe_action_drawer_triggers_favorite_and_test_group() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let item_entity = app
        .world_mut()
        .query_filtered::<Entity, With<SwipeToActionItem>>()
        .iter(app.world())
        .next()
        .expect("proxy card swipe item exists");

    app.world_mut()
        .get_mut::<SwipeToActionItem>(item_entity)
        .expect("swipe item")
        .open_leading();

    for _ in 0..15 {
        app.update();
    }

    let mut pin_query = app.world_mut().query::<(Entity, &NodePinButton)>();
    let (pin_entity, pin_btn) = pin_query.iter(app.world()).next().expect("node pin button");
    let target_outbound_id = pin_btn
        .outbound_id
        .clone()
        .expect("product node has an outbound identity");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pin_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ApplyOutboundUiPreference {
            request: OutboundUiPreferenceRequest::new(
                SnapshotRevision::new(1),
                OutboundUiPreferenceIntent::SetFavorite {
                    outbound_id: target_outbound_id,
                    state: OutboundFavoriteState::Pinned,
                },
            )
            .unwrap(),
        }]
    );
}

#[test]
fn test_proxies_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    let mut updated = ProxiesProjection::demo();
    updated.active_exit = "🇸🇬 新加坡 01 · Anycast".to_owned();
    updated.testing = true;
    updated.groups[0].current = "🇸🇬 新加坡 01 · Anycast".to_owned();
    updated.groups[0].proxies[0].selected = false;
    updated.groups[0].proxies[0].delay_ms = Some(180);
    updated.groups[0].proxies[2].selected = true;
    updated.groups[0].proxies[2].delay_ms = Some(28);

    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "🇸🇬 新加坡 01 · Anycast"
    ));
    assert!(subtree_has_text(app.world(), root, "正在全面测速中..."));
    assert!(subtree_has_text(app.world(), root, "180 ms"));
    assert!(subtree_has_text(app.world(), root, "28 ms"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "选中: 🇸🇬 新加坡 01 · Anycast"
    ));

    let mut node_query = app
        .world_mut()
        .query::<(&ProxyNodeButton, &ControlVisual)>();
    let selected_node = node_query
        .iter(app.world())
        .find(|(btn, _)| btn.group_idx == 0 && btn.node_idx == 2)
        .map(|(_, visual)| visual.0);
    assert_eq!(
        selected_node,
        Some(true),
        "newly selected node visual is true"
    );
}

#[test]
fn test_proxies_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    let empty = ProxiesProjection {
        name_runs: Default::default(),
        search_query: String::new(),
        groups: vec![],
        testing: false,
        filter_alive: false,
        compact_view: false,
        active_exit: "无可用出口".to_owned(),
        custom_node: Default::default(),
    };
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "代理策略 · 共 0 个策略组 (0 个节点)"
    ));
    assert!(subtree_has_text(app.world(), root, "无可用出口"));
    assert!(subtree_has_text(app.world(), root, "测速就绪"));
}

#[test]
fn test_proxies_inventory_projection_does_not_fabricate_missing_node_facts() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let mut inventory_only = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    inventory_only.revision += 1;
    inventory_only.core.revision = inventory_only.revision;
    inventory_only.outbound_details = PageData::unavailable(Failure::unsupported(
        "the test observes inventory without outbound details",
    ));
    app.world_mut()
        .insert_resource(LatestSurfaceSnapshot(inventory_only));
    let (root, _) = navigate_to(&mut app, Route::Proxies);
    app.update();

    let node_rows: Vec<_> = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<ProxyNodeButton>>()
            .iter(world)
            .collect()
    };
    assert!(!node_rows.is_empty(), "neutral inventory nodes are mounted");

    // Neutral inventory carries identity and selection, not protocol, feature,
    // favorite, or probe facts.
    assert!(subtree_has_text(app.world(), root, "📌"));
    assert!(subtree_has_text(app.world(), root, "📈"));
    assert!(subtree_has_text(app.world(), root, "尚未测速"));
    assert!(
        app.world_mut()
            .query::<&NodeProtoText>()
            .iter(app.world())
            .next()
            .is_none()
    );
    assert!(
        app.world_mut()
            .query::<&NodeUdpTag>()
            .iter(app.world())
            .next()
            .is_none()
    );
    for row in &node_rows {
        for fact in ["udp", "TFO", "Vision", "Reality", "Shadowsocks"] {
            assert!(
                !subtree_has_text(app.world(), *row, fact),
                "neutral outbound node cards must not show {fact} facts"
            );
        }
    }

    assert!(
        app.world_mut()
            .query::<&NodePinButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&LatencyTrendIcon>()
            .iter(app.world())
            .next()
            .is_some()
    );
}

#[test]
fn test_proxies_toggle_group_expand_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let fold_btn = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &ProxyGroupFoldButton)>();
        buttons.iter(world).next().expect("fold button mounted").0
    };

    app.world_mut()
        .commands()
        .trigger(Activate { entity: fold_btn });
    app.update();

    assert!(sink.submitted().iter().any(|cmd| matches!(
        cmd,
        UiCommand::ApplyOutboundUiPreference { request }
            if matches!(request.intent(), OutboundUiPreferenceIntent::SetGroupExpansion { expansion: OutboundGroupExpansion::Collapsed, .. })
    )));
}

#[test]
fn test_proxies_filter_alive_toggle_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let toggle_entity = app
        .world_mut()
        .query_filtered::<Entity, With<FilterAliveToggle>>()
        .single(app.world())
        .expect("filter alive toggle");

    app.world_mut().commands().trigger(Activate {
        entity: toggle_entity,
    });
    app.update();

    assert!(sink.submitted().iter().any(|cmd| matches!(
        cmd,
        UiCommand::ApplyOutboundUiPreference { request }
            if request.intent() == &OutboundUiPreferenceIntent::SetHealthFilter(OutboundHealthFilter::HealthyOnly)
    )));
}

#[test]
fn test_proxies_sort_pills_submit_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let pill_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ProxySortPill)>();
        query.iter(world).next().expect("sort pill").0
    };

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(
                cmd,
                UiCommand::ApplyOutboundUiPreference { request }
                    if request.intent() == &OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::LatencyAscending)
            ))
    );
}

#[test]
fn test_proxies_favorite_pin_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let pin_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &NodePinButton)>();
        query.iter(world).next().expect("pin button").0
    };

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pin_entity });
    app.update();

    assert!(sink.submitted().iter().any(|cmd| matches!(
        cmd,
        UiCommand::ApplyOutboundUiPreference { request }
            if matches!(request.intent(), OutboundUiPreferenceIntent::SetFavorite { .. })
    )));
}

#[test]
fn test_proxies_advanced_chips_and_color_ladder() {
    assert_eq!(format_protocol_chip("Shadowsocks"), "Shadowsocks");
    assert_eq!(format_protocol_chip("vless"), "Vless");
    assert_eq!(format_protocol_chip("hy2"), "Hysteria2");

    let caption = project_proxy_latency(Some(45));
    let tier = caption.band;
    let label = LocalizedText::new(caption.key, caption.params).render(&UiLocale::new("zh-CN"));
    assert_eq!(label, "45 ms");
    assert_eq!(tier, LatencyBand::Fast);

    let caption = project_proxy_latency(Some(0));
    let tier = caption.band;
    let label = LocalizedText::new(caption.key, caption.params).render(&UiLocale::new("zh-CN"));
    assert_eq!(label, "0 ms（结果未区分）");
    assert_eq!(tier, LatencyBand::UnconfirmedZero);
}

#[path = "proxies_group_order_tests.rs"]
mod group_order_tests;

#[path = "proxies_large_list_tests.rs"]
mod large_list_tests;

#[path = "proxies_filtering_tests.rs"]
mod filtering_tests;
