//! Behavior cases for proxies.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use bevy::ui::prelude;
use bevy::ui::prelude::Val;
use infiltrator_application::latency_projection::project_proxy_latency;
use infiltrator_bevy_ui::pages::proxies::{
    LatencySkeletonPulse, NodeDetailButton, ProxyGroupMoveUpButton, ProxyNode,
    ResetProxyGroupOrderButton, ToggleViewModeButton,
};
use infiltrator_bevy_ui::pages::proxies_filter::{format_protocol_chip, matches_proxy_filter};
use infiltrator_bevy_ui::pages::proxy_group_order::{GroupOrderAction, GroupOrderState};
use infiltrator_bevy_ui::pages::proxy_inspection::ProxyInspectionState;
use infiltrator_bevy_ui::pages::proxy_probe_settings::{
    OpenProbeSettings, ProbeTimeoutField, ProbeUrlField,
};
use infiltrator_bevy_widgets::fluid_grid::FluidCardGrid;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language};
use std::env;

#[test]
fn test_proxies_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);
    let language = env::var("INFILTRATOR_LANG").unwrap_or_else(|_| get_system_language());
    let lang = Lang(&language);

    assert!(subtree_has_text(
        app.world(),
        root,
        "代理策略 · 共 3 个策略组 (8 个节点)"
    ));
    assert!(subtree_has_text(app.world(), root, "HK-01"));
    assert!(subtree_has_text(app.world(), root, "测速就绪"));
    assert!(subtree_has_text(app.world(), root, "PROXIES"));
    assert!(subtree_has_text(app.world(), root, "38 ms"));
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
    // Node capability tags & protocol badges
    assert!(subtree_has_text(app.world(), root, "udp"));
    assert!(subtree_has_text(app.world(), root, "Shadowsocks"));

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

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<TestAllProxiesButton>>()
        .single(app.world())
        .expect("test all proxies button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::TestAllProxyGroups]);
}

#[test]
fn test_proxies_test_group_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let mut query = app.world_mut().query::<(Entity, &TestProxyGroupButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.group_idx == 0)
        .expect("test proxy group button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::TestProxyGroup {
            group: "PROXIES".to_owned(),
        }]
    );
}

#[test]
fn test_proxies_select_node_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let mut query = app.world_mut().query::<(Entity, &ProxyNodeButton)>();
    let (node_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.node_name == "JP-02")
        .expect("target proxy node button");

    app.world_mut().commands().trigger(Activate {
        entity: node_entity,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SelectProxyNode {
            group: "PROXIES".to_owned(),
            node: "JP-02".to_owned(),
        }]
    );
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
    let target_node_name = pin_btn.node_name.clone();

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pin_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ToggleFavoriteProxy(target_node_name)]
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
fn test_proxies_favorite_and_features_rendering() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    // Default demo has favorite stars, pin icons, latency trend waves, and feature chips
    assert!(subtree_has_text(app.world(), root, "★"));
    assert!(subtree_has_text(app.world(), root, "📌"));
    assert!(subtree_has_text(app.world(), root, "📈"));
    assert!(subtree_has_text(app.world(), root, "udp"));
    assert!(subtree_has_text(app.world(), root, "UDP"));
    assert!(subtree_has_text(app.world(), root, "TFO"));
    assert!(subtree_has_text(app.world(), root, "Vision"));
    assert!(subtree_has_text(app.world(), root, "Reality"));
    assert!(subtree_has_text(app.world(), root, "Shadowsocks"));

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
    assert!(
        app.world_mut()
            .query::<&NodeUdpTag>()
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
        UiCommand::ToggleProxyGroupExpand { group } if !group.is_empty()
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

    assert_eq!(sink.submitted(), vec![UiCommand::ToggleFilterAlive(true)]);
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
            .any(|cmd| matches!(cmd, UiCommand::SetProxySortOrder(_)))
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

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::ToggleFavoriteProxy(_)))
    );
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

#[test]
fn test_proxies_pinyin_fuzzy_and_protocol_filtering() {
    let node = ProxyNode {
        name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
        node_type: "VLESS".to_owned(),
        delay_ms: Some(45),
        selected: true,
        favorite: true,
        features: vec!["Reality".to_owned(), "Vision".to_owned()],
    };

    assert!(matches_proxy_filter(&node, "xg"));
    assert!(matches_proxy_filter(&node, "hk"));
    assert!(matches_proxy_filter(&node, "vless"));
    assert!(matches_proxy_filter(&node, "reality"));
    assert!(matches_proxy_filter(&node, "<100"));
    assert!(!matches_proxy_filter(&node, "日本"));
}

#[test]
fn test_proxies_node_detail_drawer_and_group_reorder() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    // 1. Detail button activation (DUAL-04-11)
    let detail_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &NodeDetailButton)>();
        query
            .iter(world)
            .next()
            .expect("node detail button mounted")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: detail_entity,
    });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "inspection must never select a node"
    );
    assert!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .is_some()
    );

    // 2. Group move up activation (DUAL-04-12)
    let move_up_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ProxyGroupMoveUpButton)>();
        query
            .iter(world)
            .nth(1)
            .expect("second group move up button mounted")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: move_up_entity,
    });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "order editing must not submit any command before Apply"
    );
    assert!(app.world().resource::<GroupOrderState>().open);
    assert_ne!(
        app.world().resource::<GroupOrderState>().editor.draft,
        app.world().resource::<GroupOrderState>().editor.baseline
    );

    // 3. Reset group order activation (DUAL-04-12)
    let reset_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ResetProxyGroupOrderButton>>()
        .single(app.world())
        .expect("reset proxy group order button mounted");

    app.world_mut().commands().trigger(Activate {
        entity: reset_entity,
    });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "reset changes only the visible draft"
    );
    let cancel = app
        .world_mut()
        .query::<(Entity, &GroupOrderAction)>()
        .iter(app.world())
        .find(|(_, action)| matches!(action, GroupOrderAction::Cancel))
        .unwrap()
        .0;
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    assert!(!app.world().resource::<GroupOrderState>().open);
    assert!(
        sink.submitted().is_empty(),
        "cancel retains the original shared order"
    );

    // 4. Toggle compact view activation (DUAL-04-13)
    let toggle_view_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ToggleViewModeButton>>()
        .single(app.world())
        .expect("toggle view mode button mounted");

    app.world_mut().commands().trigger(Activate {
        entity: toggle_view_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::SetProxyCompactView(true)))
    );

    // 5. Latency skeleton pulse mounted (DUAL-04-14)
    assert!(
        app.world_mut()
            .query::<&LatencySkeletonPulse>()
            .iter(app.world())
            .next()
            .is_some(),
        "latency skeleton pulse component must be mounted"
    );
}
