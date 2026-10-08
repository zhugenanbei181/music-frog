//! Behavior cases for connections.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::connections_pulse::ConnectionsPulseState;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

#[test]
fn test_connections_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    assert!(subtree_has_text(
        app.world(),
        root,
        "活动连接 · 当前活跃 4 个连接"
    ));
    assert!(subtree_has_text(app.world(), root, "api.github.com:443"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "manifest.googlevideo.com:443"
    ));
    assert!(subtree_has_text(app.world(), root, "关闭全部连接"));
    assert!(subtree_has_text(app.world(), root, "断开"));
    assert!(subtree_has_text(app.world(), root, "实时流"));
    assert!(subtree_has_text(app.world(), root, "按进程聚合"));
    assert!(subtree_has_text(app.world(), root, "按域名聚合"));
    let locale = app.world().resource::<UiLocale>().code().to_string();
    assert!(subtree_has_text(
        app.world(),
        root,
        &Lang(&locale).tr("conn_drawer_title")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        &Lang(&locale).tr("conn_drawer_timing_unsupported")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        &Lang(&locale).tr("quick_rule_btn")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "按域名/IP/进程即时搜索连接"
    ));
    assert!(subtree_has_text(app.world(), root, "断开筛选结果"));
    assert_eq!(
        app.world_mut()
            .query::<&ConnAggregationPill>()
            .iter(app.world())
            .count(),
        3
    );
}

#[test]
fn test_connections_aggregation_pill_switches_shared_mode() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let mut query = app.world_mut().query::<(Entity, &ConnAggregationPill)>();
    let (pill_entity, _) = query
        .iter(app.world())
        .find(|(_, pill)| pill.0 == ConnectionGroupingMode::ByProcess)
        .expect("by-process aggregation pill");

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "按应用进程聚合 · 共 4 组"
    ));
    // Grouped mode hides the flat rows and shows the summary.
    let rows_container_display = app
        .world_mut()
        .query_filtered::<&Node, With<ConnRowsContainer>>()
        .single(app.world())
        .expect("rows container")
        .display;
    assert_eq!(rows_container_display, Display::None);
}

#[test]
fn test_connections_search_hides_non_matching_rows() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Connections);

    let field_entity = app
        .world_mut()
        .query_filtered::<&Children, With<ConnSearchField>>()
        .single(app.world())
        .expect("search field wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("search text field");
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("github");
    app.update();

    let mut rows = app.world_mut().query::<(&Node, &ConnectionRow)>();
    let displays: Vec<(usize, Display)> = rows
        .iter(app.world())
        .map(|(node, row)| (row.0, node.display))
        .collect();
    assert!(displays.contains(&(0, Display::Flex)));
    assert!(displays.contains(&(1, Display::None)));
}

#[test]
fn test_connections_close_filtered_submits_matching_only() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let field_entity = app
        .world_mut()
        .query_filtered::<&Children, With<ConnSearchField>>()
        .single(app.world())
        .expect("search field wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("search text field");
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("github");
    app.update();

    let button_entity = app
        .world_mut()
        .query_filtered::<Entity, With<CloseFilteredConnectionsButton>>()
        .single(app.world())
        .expect("close filtered button");
    app.world_mut().commands().trigger(Activate {
        entity: button_entity,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::CloseConnection {
            id: "c-1".to_owned(),
        }]
    );
}

#[test]
fn test_connections_close_single_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let mut query = app.world_mut().query::<(Entity, &CloseConnectionButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.connection_id == "c-1")
        .expect("c-1 close button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::CloseConnection {
            id: "c-1".to_owned(),
        }]
    );
}

#[test]
fn test_connections_swipe_action_drawer_triggers_close_connection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let item_entity = app
        .world_mut()
        .query_filtered::<Entity, With<SwipeToActionItem>>()
        .iter(app.world())
        .next()
        .expect("connection row swipe item exists");

    app.world_mut()
        .get_mut::<SwipeToActionItem>(item_entity)
        .expect("swipe item")
        .open_leading();

    for _ in 0..15 {
        app.update();
    }

    let mut query = app.world_mut().query::<(Entity, &CloseConnectionButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.connection_idx == 0)
        .expect("connection 0 close button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::CloseConnection {
            id: "c-1".to_owned(),
        }]
    );
}

#[test]
fn test_connections_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let mut updated = ConnectionsProjection::demo();
    updated.total_connections = 12;
    updated.total_upload_bytes = 50_000_000;
    updated.total_download_bytes = 300_000_000;
    updated.connections[0].destination_host = "api.cloudflare.com".to_owned();
    updated.connections[0].host = "api.cloudflare.com:443".to_owned();
    for index in 4..12 {
        let mut row = updated.connections[1].clone();
        row.id = format!("fresh-{index}");
        updated.connections.push(row);
    }
    updated.connections[0].upload_bps = 500_000.0;
    updated.connections[0].download_bps = 1_200_000.0;

    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "活动连接 · 当前活跃 12 个连接"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "api.cloudflare.com:443"
    ));
    assert!(subtree_has_text(app.world(), root, "488.28 KB/s"));
    assert!(subtree_has_text(app.world(), root, "1.14 MB/s"));
}

#[test]
fn test_connections_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let empty = ConnectionsProjection {
        total_connections: 0,
        total_upload_bytes: 0,
        total_download_bytes: 0,
        stream_phase: ConnectionStreamPhase::Live,
        connections: vec![],
    };
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "活动连接 · 当前活跃 0 个连接"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "累积上传: 0 B | 累积下载: 0 B"
    ));
}

#[test]
fn test_connections_stream_badge_reflects_shared_phase() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // The demo projection carries the shared `Live` phase.
    assert!(subtree_has_text(app.world(), root, "连接流 · 实时"));

    let mut reconnecting = ConnectionsProjection::demo();
    reconnecting.stream_phase = ConnectionStreamPhase::Reconnecting;
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(reconnecting));
    app.update();
    assert!(subtree_has_text(app.world(), root, "连接流 · 重连中"));
}

#[test]
fn test_connections_route_chain_renders_each_hop() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // DUAL-13-06: each hop is its own text node through the shared model.
    assert!(subtree_has_text(app.world(), root, "PROXIES"));
    assert!(subtree_has_text(app.world(), root, "HK-01"));
    assert!(subtree_has_text(app.world(), root, "STREAMING"));
    // The pre-joined snapshot string is no longer what the surface renders.
    assert!(!subtree_has_text(app.world(), root, "PROXIES -> HK-01"));
}

#[test]
fn test_connections_inspect_opens_shared_drawer() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let inspect_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnInspectButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.0 == 0)
            .map(|(entity, _)| entity)
            .expect("row 0 inspect button")
    };

    app.world_mut().commands().trigger(Activate {
        entity: inspect_entity,
    });
    app.update();

    let state = app.world().resource::<ConnectionsDrawerState>();
    assert!(state.open);
    assert_eq!(state.selected, Some(0));

    let layer_display = app
        .world_mut()
        .query_filtered::<&Node, With<ConnectionDrawerLayer>>()
        .single(app.world())
        .expect("drawer layer")
        .display;
    assert_eq!(layer_display, Display::Flex);
    assert!(subtree_has_text(app.world(), root, "api.github.com:443"));
}

#[test]
fn test_connections_add_rule_draft_uses_shared_seam() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Connections);

    // Select row 0, then draft a reverse rule from the drawer action.
    let inspect_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnInspectButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.0 == 0)
            .map(|(entity, _)| entity)
            .expect("row 0 inspect button")
    };
    app.world_mut().commands().trigger(Activate {
        entity: inspect_entity,
    });
    app.update();

    let add_entity = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerAddRuleButton>>()
        .single(app.world())
        .expect("add rule button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: add_entity });
    app.update();

    let first = app.world().resource::<ConnectionsRuleDraft>().clone();
    assert_eq!(first.entries.len(), 1);
    assert_eq!(first.entries[0].rule, "DOMAIN-SUFFIX,api.github.com,DIRECT");

    // The shared seam de-duplicates the same rule line.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: add_entity });
    app.update();
    assert_eq!(
        app.world().resource::<ConnectionsRuleDraft>().entries.len(),
        1
    );
}

#[test]
fn test_connections_idle_timeout_pill_switches_shared_choice() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Connections);

    let pill_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnIdleTimeoutPill)>();
        query
            .iter(app.world())
            .find(|(_, pill)| pill.0 == 1800)
            .map(|(entity, _)| entity)
            .expect("30m idle timeout pill")
    };
    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ConnectionsIdleState>().timeout_secs,
        1800
    );
}

#[test]
fn test_connections_idle_sweep_submits_and_reports() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // Rewind the tracker's activity clock to the epoch so the demo rows count
    // as idle, then run the sweep button.
    let demo = ConnectionsProjection::demo();
    {
        let mut state = app.world_mut().resource_mut::<ConnectionsIdleState>();
        let mut changed = demo.connections.clone();
        for item in &mut changed {
            item.upload_total += 1;
        }
        state.tracker.observe(&changed, 1);
        state.tracker.observe(&demo.connections, 1);
        state.timeout_secs = 0;
    }

    let sweep_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ConnIdleSweepButton>>()
        .single(app.world())
        .expect("idle sweep button");
    app.world_mut().commands().trigger(Activate {
        entity: sweep_entity,
    });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 4);
    assert!(
        submitted
            .iter()
            .all(|command| matches!(command, UiCommand::CloseConnection { .. }))
    );
    let locale = app.world().resource::<UiLocale>().code().to_string();
    assert!(subtree_has_text(
        app.world(),
        root,
        &interpolate(
            Lang(&locale).tr("conn_idle_last_sweep").as_ref(),
            &[("count", "4")]
        )
    ));
}

#[test]
fn test_connections_high_throughput_pulse_follows_shared_threshold() {
    use infiltrator_bevy_ui::pages::connections_pulse::ConnHighThroughputPulse;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // DUAL-13-10: the demo fixture has exactly one row above the shared
    // 5 MB/s threshold (c-2 at 8.5 MB/s); every other row stays unlit.
    let pulse_states = |app: &mut App| -> Vec<(usize, Display)> {
        let mut query = app.world_mut().query::<(&Node, &ConnHighThroughputPulse)>();
        query
            .iter(app.world())
            .map(|(node, pulse)| (pulse.0, node.display))
            .collect()
    };
    let states = pulse_states(&mut app);
    assert_eq!(states.len(), 4);
    assert!(states.contains(&(1, Display::Flex)));
    assert!(states.contains(&(0, Display::None)));
    assert!(subtree_has_text(app.world(), root, "高吞吐脉冲"));

    // A snapshot with every rate below the threshold hides every pulse.
    let mut slow = ConnectionsProjection::demo();
    slow.connections[1].download_bps = 1_000.0;
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(slow));
    app.update();
    assert!(
        pulse_states(&mut app)
            .iter()
            .all(|(_, display)| *display == Display::None)
    );

    // The animation system keeps the lit rows breathing from the shared phase.
    let mut app2 = setup_matrix_a_app(Arc::new(DemoCommandSink::accepting()));
    navigate_to(&mut app2, Route::Connections);
    let mut frames = 0;
    while frames < 5 {
        app2.update();
        frames += 1;
    }
    let phase = app2.world().resource::<ConnectionsPulseState>().phase;
    assert!(
        (0.0..1.0).contains(&phase),
        "the breathing phase stays inside one breath, got {phase}"
    );
}

#[test]
fn test_connections_sort_pills_reorder_rows_by_instantaneous_rate() {
    use infiltrator_bevy_ui::pages::connections_view::ConnSortPill;
    use infiltrator_domain::connection_view::ConnectionSortKey;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // DUAL-13-12: the header exposes the shared sort keys and starts on the
    // same cumulative-download order the Iced surface defaults to.
    let locale = app.world().resource::<UiLocale>().code().to_string();
    for key in [
        "runtime_conn_sort_download_desc",
        "runtime_conn_sort_upload_desc",
        "runtime_conn_sort_download_rate",
        "runtime_conn_sort_upload_rate",
        "runtime_conn_sort_latest_desc",
        "runtime_conn_sort_host_asc",
    ] {
        assert!(subtree_has_text(app.world(), root, &Lang(&locale).tr(key)));
    }
    assert_eq!(
        app.world_mut()
            .query::<&ConnSortPill>()
            .iter(app.world())
            .count(),
        6
    );
    assert_eq!(connection_row_order(&mut app), vec![1, 0, 2, 3]);

    // A fresh snapshot where instantaneous trends disagree with the totals:
    // c-1 is idle and c-3 is the fastest download despite the smallest total.
    let mut updated = ConnectionsProjection::demo();
    updated.connections[0].download_bps = 0.0;
    updated.connections[2].download_bps = 9_000_000.0;
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(updated));
    app.update();
    assert_eq!(
        connection_row_order(&mut app),
        vec![1, 0, 2, 3],
        "the cumulative default order is unchanged"
    );

    // Clicking the instantaneous-download pill reorders by the derived rate:
    // c-3 leads, then c-2, then the two idle rows in stable id order.
    let download_rate_pill = {
        let mut query = app.world_mut().query::<(Entity, &ConnSortPill)>();
        query
            .iter(app.world())
            .find(|(_, pill)| pill.0 == ConnectionSortKey::DownloadRateDesc)
            .map(|(entity, _)| entity)
            .expect("instantaneous download sort pill")
    };
    app.world_mut().commands().trigger(Activate {
        entity: download_rate_pill,
    });
    app.update();
    assert_eq!(
        app.world().resource::<ConnectionsViewState>().sort,
        ConnectionSortKey::DownloadRateDesc
    );
    assert_eq!(connection_row_order(&mut app), vec![2, 1, 0, 3]);

    // The instantaneous-upload pill ranks c-1 first (24 000 B/s vs 8 500).
    let upload_rate_pill = {
        let mut query = app.world_mut().query::<(Entity, &ConnSortPill)>();
        query
            .iter(app.world())
            .find(|(_, pill)| pill.0 == ConnectionSortKey::UploadRateDesc)
            .map(|(entity, _)| entity)
            .expect("instantaneous upload sort pill")
    };
    app.world_mut().commands().trigger(Activate {
        entity: upload_rate_pill,
    });
    app.update();
    assert_eq!(connection_row_order(&mut app), vec![0, 1, 2, 3]);

    // A fresh snapshot is re-sorted under the active key, so the order stays
    // live instead of freezing at the click.
    let mut updated = ConnectionsProjection::demo();
    updated.connections[2].upload_bps = 900_000.0;
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(updated));
    app.update();
    assert_eq!(connection_row_order(&mut app), vec![2, 0, 1, 3]);
    // Row markers stay bound to their projection index, so the restamped
    // texts still describe the same connection the row carries.
    assert!(subtree_has_text(
        app.world(),
        root,
        "gateway.discord.gg:443",
    ));
}

#[test]
fn test_connections_drawer_parity_exposes_shared_fields_and_close_action() {
    use infiltrator_bevy_ui::pages::connections_drawer::DrawerCloseConnectionButton;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let inspect_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnInspectButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.0 == 0)
            .map(|(entity, _)| entity)
            .expect("row 0 inspect button")
    };
    app.world_mut().commands().trigger(Activate {
        entity: inspect_entity,
    });
    app.update();

    // DUAL-13-14: the drawer carries the same host-backed fields the Iced
    // drawer renders: endpoints, transport, matched rule payload and the
    // derived instantaneous rates.
    assert!(subtree_has_text(
        app.world(),
        root,
        "192.168.1.20:51432 → 140.82.121.5:443"
    ));
    assert!(subtree_has_text(app.world(), root, "TCP"));
    assert!(subtree_has_text(app.world(), root, "github.com"));
    let locale = app.world().resource::<UiLocale>().code().to_string();
    let lang = Lang(&locale);
    let expected_rate = format!(
        "{} ↑ 23.44 KB/s  ↓ 175.78 KB/s",
        lang.tr("conn_drawer_rate_prefix")
    );
    assert!(subtree_has_text(app.world(), root, &expected_rate));

    // The drawer's disconnect action submits the shared teardown command and
    // closes the drawer, matching the Iced drawer's action.
    let close_entity = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .expect("drawer close connection button");
    app.world_mut().commands().trigger(Activate {
        entity: close_entity,
    });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::CloseConnection {
            id: "c-1".to_owned(),
        }]
    );
    assert!(!app.world().resource::<ConnectionsDrawerState>().open);
}

#[test]
fn test_connections_drawer_renders_kernel_asn_and_geo_without_guessing() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // DUAL-13-05: the kernel's own rule-evaluation results. Row 0 has real
    // values, row 1 was evaluated with no record, row 2 was never evaluated.
    let mut projection = ConnectionsProjection::demo();
    projection.connections[0].destination_ip_asn = "15169 Google LLC".to_owned();
    projection.connections[0].destination_geo_ip = Some(vec!["us".to_owned()]);
    projection.connections[1].destination_ip_asn = " ".to_owned();
    projection.connections[1].destination_geo_ip = Some(Vec::new());
    projection.connections[2].destination_ip_asn = String::new();
    projection.connections[2].destination_geo_ip = None;
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(projection));
    app.update();

    let open_drawer = |app: &mut App, row: usize| {
        let inspect_entity = {
            let mut query = app.world_mut().query::<(Entity, &ConnInspectButton)>();
            query
                .iter(app.world())
                .find(|(_, button)| button.0 == row)
                .map(|(entity, _)| entity)
                .expect("row inspect button")
        };
        app.world_mut().commands().trigger(Activate {
            entity: inspect_entity,
        });
        app.update();
    };

    let locale = app.world().resource::<UiLocale>().code().to_string();
    let lang = Lang(&locale);
    let asn_header = lang.tr("conn_drawer_kernel_asn");
    let geo_header = lang.tr("conn_drawer_kernel_geo");
    let no_result = lang.tr("conn_drawer_kernel_no_result");
    let not_evaluated = lang.tr("conn_drawer_kernel_not_evaluated");

    // Row 0: the kernel value is rendered verbatim (no client-side `AS` prefix).
    open_drawer(&mut app, 0);
    assert!(subtree_has_text(
        app.world(),
        root,
        &format!("{asn_header}: 15169 Google LLC")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        &format!("{geo_header}: us")
    ));

    // Row 1: the kernel evaluated and its database had no record.
    open_drawer(&mut app, 1);
    assert!(subtree_has_text(
        app.world(),
        root,
        &format!("{asn_header}: {no_result}")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        &format!("{geo_header}: {no_result}")
    ));

    // Row 2: no such rule ran, so the drawer says the kernel did not evaluate
    // instead of inventing a location or an ASN.
    open_drawer(&mut app, 2);
    assert!(subtree_has_text(
        app.world(),
        root,
        &format!("{asn_header}: {not_evaluated}")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        &format!("{geo_header}: {not_evaluated}")
    ));
}
