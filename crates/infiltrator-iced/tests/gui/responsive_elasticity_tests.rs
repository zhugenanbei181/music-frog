//! Responsive multi-size elasticity tests for the Iced shell.
//!
//! Verifies that the window-resize message drives the shared 4-tier viewport
//! projection and that the shell's navigation form and content padding follow
//! it, instead of the sidebar staying static. Mounted via `src/test_mounts.rs`.
//! test-intent: behavior

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::profiles::list::profile_grid_geometry;
use crate::view::proxies::node_cards::proxy_node_grid_columns;
use crate::view::sidebar::sidebar_form_for_width;
use infiltrator_contract::responsive_viewport::{
    PAGE_PADDING_PX, ProfileGridForm, ProxyGridForm, SidebarForm, ViewportTier, profile_columns,
    proxy_node_columns,
};
use lurker_core_contract::outbound::preferences::OutboundLayout;

#[test]
fn window_resize_updates_shared_viewport_tier() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.shell.viewport.tier, ViewportTier::Expanded);

    let cases = [
        (390.0, 844.0, ViewportTier::Compact),
        (720.0, 1024.0, ViewportTier::Medium),
        (1000.0, 780.0, ViewportTier::Expanded),
        (1920.0, 1080.0, ViewportTier::Ultra),
    ];
    for (w, h, expected) in cases {
        let _ = state.update(Message::WindowResized(w, h));
        assert_eq!(
            state.shell.viewport.tier, expected,
            "width {w} should classify as {expected:?}"
        );
        assert_eq!(state.shell.viewport.width_px, w);
        assert_eq!(state.shell.viewport.height_px, h);
        // The columns come from the shared contract, not a local table.
        assert_eq!(
            state.shell.viewport.card_columns,
            expected.overview_card_columns()
        );
    }
}

#[test]
fn sidebar_form_follows_tier_and_is_width_consistent() {
    // Compact: bottom navigation, no vertical rail.
    assert_eq!(sidebar_form_for_width(500.0), SidebarForm::BottomNav);
    assert_eq!(SidebarForm::BottomNav.width_px(), None);

    // Medium: slim rail at the rail width.
    assert_eq!(sidebar_form_for_width(700.0), SidebarForm::Rail);
    assert_eq!(SidebarForm::Rail.width_px(), Some(64));

    // Expanded: standard labelled sidebar, width from the shared contract.
    assert_eq!(sidebar_form_for_width(1000.0), SidebarForm::Standard);
    assert_eq!(SidebarForm::Standard.width_px(), Some(240));

    // Ultra: wide sidebar.
    assert_eq!(sidebar_form_for_width(1600.0), SidebarForm::Wide);
    assert_eq!(SidebarForm::Wide.width_px(), Some(280));
}

#[test]
fn the_page_gutter_is_one_contract_value_at_every_tier() {
    // `SPACEUTIL-018`: this asserted a per-tier ramp (16 / 24 / 40 / 48), which
    // is how a desktop page spent 80px of side ground where the reference
    // spends 32dp on a phone page and a 4K page alike. The gutter is one
    // exported constant, so resizing must never move it - only the tier's own
    // navigation column may narrow the body.
    let (mut state, _) = AppState::new();
    for (width, expected_body) in [(400.0, 368u16), (720.0, 624), (1180.0, 908), (1920.0, 1608)] {
        let _ = state.update(Message::WindowResized(width, 800.0));
        let tier = state.shell.viewport.tier;
        assert_eq!(
            tier.page_body_width_px(state.shell.viewport.width_px.round() as u16),
            expected_body,
            "page body at {width}px must be the window less one gutter twice and the tier column"
        );
        assert_eq!(
            expected_body + 2 * PAGE_PADDING_PX + tier.nav_column_px(),
            width as u16,
            "the gutter must be subtracted exactly once at {width}px"
        );
    }
    assert_eq!(PAGE_PADDING_PX, 16);
}

#[test]
fn grid_columns_track_tier_for_overview_and_proxies() {
    // Overview metrics band and the proxy node grid both read the same shared
    // tier operators the render path uses, so the two surfaces agree.
    let (mut state, _) = AppState::new();

    let cases = [
        (400.0, 2usize, 1usize), // Compact
        (700.0, 3, 2),           // Medium
        (1000.0, 6, 3),          // Expanded
        (1600.0, 6, 4),          // Ultra
    ];
    for (width, metrics, proxies) in cases {
        let _ = state.update(Message::WindowResized(width, 800.0));
        assert_eq!(
            state.shell.viewport.metrics_columns, metrics,
            "metrics columns at {width}px"
        );
        assert_eq!(
            state.shell.viewport.tier.proxy_grid_columns(false),
            proxies,
            "proxy columns at {width}px"
        );
    }
}

#[test]
fn proxy_compact_view_forces_single_column_every_tier() {
    // The user's high-density preference overrides only the grid mode, not the
    // tier classification itself.
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::WindowResized(1920.0, 1080.0));
    assert_eq!(state.shell.viewport.tier, ViewportTier::Ultra);
    assert_eq!(state.shell.viewport.tier.proxy_grid_columns(false), 4);
    assert_eq!(state.shell.viewport.tier.proxy_grid_columns(true), 1);
}

#[test]
fn page_width_rules_size_profile_and_proxy_grids() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::WindowResized(1180.0, 780.0));
    state
        .runtime
        .proxies_model
        .select_layout(OutboundLayout::Grid);
    let tier = state.shell.viewport.tier;
    let page_width = tier.page_body_width_px(1180);
    // 1180 less the 240 labelled column less one 16px gutter twice. This was
    // 860 while the gutter ramped per tier (40px here), which spent 80px of
    // side ground where the reference spends 32dp on the same page
    // (`SPACEUTIL-018`).
    assert_eq!(page_width, 908);

    let (profile_gap, profile_columns_actual) = profile_grid_geometry(&state);
    assert_eq!(
        profile_columns_actual as u16,
        profile_columns(ProfileGridForm::Desktop, page_width, profile_gap as u16,)
    );

    let (proxy_form, proxy_columns_actual) = proxy_node_grid_columns(&state);
    assert_eq!(proxy_form, ProxyGridForm::Desktop);
    assert_eq!(
        proxy_columns_actual as u16,
        proxy_node_columns(proxy_form, page_width)
    );

    // A phone has no labelled column; both grids stay readable and stacked.
    let _ = state.update(Message::WindowResized(400.0, 800.0));
    let (_, phone_profile_columns) = profile_grid_geometry(&state);
    let (phone_proxy_form, phone_proxy_columns) = proxy_node_grid_columns(&state);
    assert_eq!(phone_profile_columns, 1);
    assert_eq!(phone_proxy_form, ProxyGridForm::Stacked);
    assert_eq!(phone_proxy_columns, 1);
}

#[test]
fn centered_modals_shrink_on_narrow_viewports() {
    // A centred dialog must never be wider than the window; on a wide desktop
    // it keeps its preferred width.
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::WindowResized(1920.0, 1080.0));
    assert_eq!(state.shell.viewport.clamped_modal_width(560.0), 560.0);

    let _ = state.update(Message::WindowResized(420.0, 800.0));
    let clamped = state.shell.viewport.clamped_modal_width(560.0);
    assert!(
        clamped <= 420.0,
        "dialog width {clamped} exceeds a 420px window"
    );
    // A dialog already narrower than the cap keeps its preferred width.
    assert_eq!(state.shell.viewport.clamped_modal_width(100.0), 100.0);
}

#[test]
fn list_page_budget_tracks_window_tier() {
    // Rules and connections are paginated; shrinking the window must shrink the
    // per-page row budget so a short viewport does not build hundreds of rows.
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::WindowResized(1920.0, 1080.0));
    assert_eq!(state.editor.rules_page_size, 200);
    assert_eq!(state.diag.connections_page_size, 100);

    let _ = state.update(Message::WindowResized(400.0, 700.0));
    assert_eq!(state.editor.rules_page_size, 66);
    assert_eq!(state.diag.connections_page_size, 33);

    // Growing back restores the desktop budget.
    let _ = state.update(Message::WindowResized(1600.0, 1000.0));
    assert_eq!(state.editor.rules_page_size, 200);
    assert_eq!(state.diag.connections_page_size, 100);
}

#[test]
fn connection_drawer_goes_full_bleed_on_narrow_viewport() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::WindowResized(1920.0, 1080.0));
    assert_eq!(state.shell.viewport.detail_panel_width_px(480.0), 480.0);

    let _ = state.update(Message::WindowResized(420.0, 800.0));
    let width = state.shell.viewport.detail_panel_width_px(480.0);
    assert!(width <= 420.0, "drawer {width} exceeds a 420px window");
    assert!(width > 300.0, "drawer {width} should stay near full-bleed");
}

#[test]
fn overview_card_order_follows_shared_layout_moves() {
    use crate::test_mounts::command_harness::attach_product_application_with_accepted_owner_snapshot;
    use crate::test_mounts::surface_task_harness::{
        drain_surface_task, overview_layout_application,
    };
    use infiltrator_contract::overview_layout::OverviewCardKind;

    let (mut state, _) = AppState::new();
    // The card order is the owner's fact: the surface dispatches the shared
    // move intent and renders the order the owner publishes, so the test
    // needs an owner that answers that intent.
    attach_product_application_with_accepted_owner_snapshot(
        &mut state,
        overview_layout_application(),
    );
    assert_eq!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );

    // Move the first card down: it swaps with the second card.
    let moved = state.update(Message::MoveOverviewCardDown(OverviewCardKind::Traffic));
    drain_surface_task(&mut state, moved, AppState::update);
    assert_eq!(state.diag.overview_card_order[0], OverviewCardKind::Metrics);
    assert_eq!(state.diag.overview_card_order[1], OverviewCardKind::Traffic);

    // Move it back up restores the canonical order.
    let moved = state.update(Message::MoveOverviewCardUp(OverviewCardKind::Traffic));
    drain_surface_task(&mut state, moved, AppState::update);
    assert_eq!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );

    // Moving the top card up is a no-op (already first).
    let moved = state.update(Message::MoveOverviewCardUp(OverviewCardKind::Traffic));
    drain_surface_task(&mut state, moved, AppState::update);
    assert_eq!(state.diag.overview_card_order[0], OverviewCardKind::Traffic);

    // After a custom order, reset restores the default.
    let moved = state.update(Message::MoveOverviewCardUp(OverviewCardKind::Quota));
    drain_surface_task(&mut state, moved, AppState::update);
    assert_ne!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
    let moved = state.update(Message::ResetOverviewCardOrder);
    drain_surface_task(&mut state, moved, AppState::update);
    assert_eq!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
}
#[test]
fn overview_topology_pipeline_adapts_to_viewport_tier() {
    use crate::view::overview_topology::topology_card;
    use infiltrator_shared::locales::Lang;

    let (mut state, _) = AppState::new();
    let lang = Lang("en-US");

    // Compact viewport: folds vertically
    let _ = state.update(Message::WindowResized(390.0, 844.0));
    assert_eq!(state.shell.viewport.tier, ViewportTier::Compact);
    let compact_card = topology_card(&state, &lang, true);
    drop(compact_card);

    // Expanded viewport: wide horizontal flow
    let _ = state.update(Message::WindowResized(1000.0, 780.0));
    assert_eq!(state.shell.viewport.tier, ViewportTier::Expanded);
    let expanded_card = topology_card(&state, &lang, true);
    drop(expanded_card);
}

#[test]
fn full_spectrum_elasticity_across_all_four_tiers_and_thirteen_routes() {
    use crate::types::app::Route;

    let (mut state, _) = AppState::new();

    let routes = [
        Route::Overview,
        Route::Profiles,
        Route::Proxies,
        Route::Runtime,
        Route::Connections,
        Route::Logs,
        Route::Rules,
        Route::Dns,
        Route::Sync,
        Route::Editor,
        Route::Settings,
        Route::AppRouting,
        Route::Doctor,
    ];

    let viewports = [
        // Compact (<600px): extreme mobile (360px) & standard mobile. The
        // fourth column is the page gutter, one contract value everywhere; it
        // used to be a per-tier ramp (16/24/40/48) that `SPACEUTIL-018`
        // retired because the reference keeps 16dp at every width.
        (
            360.0,
            640.0,
            ViewportTier::Compact,
            16,
            1usize,
            2usize,
            None,
        ),
        (414.0, 896.0, ViewportTier::Compact, 16, 1, 2, None),
        (599.0, 800.0, ViewportTier::Compact, 16, 1, 2, None),
        // Medium (600..840px): small tablet / foldables (64px rail)
        (600.0, 900.0, ViewportTier::Medium, 16, 2, 3, Some(64u16)),
        (768.0, 1024.0, ViewportTier::Medium, 16, 2, 3, Some(64)),
        (839.0, 1000.0, ViewportTier::Medium, 16, 2, 3, Some(64)),
        // Expanded (840..1200px): laptops & standard desktops (240px sidebar)
        (840.0, 700.0, ViewportTier::Expanded, 16, 3, 6, Some(240)),
        (1024.0, 768.0, ViewportTier::Expanded, 16, 3, 6, Some(240)),
        (1199.0, 800.0, ViewportTier::Expanded, 16, 3, 6, Some(240)),
        // Ultra (>=1200px): wide monitors & 4K displays (280px wide sidebar)
        (1200.0, 900.0, ViewportTier::Ultra, 16, 4, 6, Some(280)),
        (1920.0, 1080.0, ViewportTier::Ultra, 16, 4, 6, Some(280)),
        (2560.0, 1440.0, ViewportTier::Ultra, 16, 4, 6, Some(280)),
    ];

    for (
        w,
        h,
        expected_tier,
        expected_gutter,
        expected_proxies,
        expected_metrics,
        expected_sidebar,
    ) in viewports
    {
        let _ = state.update(Message::WindowResized(w, h));

        assert_eq!(
            state.shell.viewport.tier, expected_tier,
            "tier mismatch at {w}x{h}"
        );
        // The `expected_pad` column of this table used to carry a per-tier
        // ramp (16/24/40/48). One gutter is now subtracted at every tier, so
        // what this asserts instead is that the page body the grid will be
        // measured against is exactly the window less that gutter twice and
        // less the tier's own navigation column - never more than once.
        assert_eq!(
            state
                .shell
                .viewport
                .tier
                .page_body_width_px(w.round() as u16)
                + state.shell.viewport.tier.nav_column_px()
                + 2 * PAGE_PADDING_PX,
            w.round() as u16,
            "the gutter must be subtracted exactly once at {w}x{h}"
        );
        assert_eq!(expected_gutter, usize::from(PAGE_PADDING_PX));
        assert_eq!(
            state.shell.viewport.tier.proxy_grid_columns(false),
            expected_proxies
        );
        assert_eq!(state.shell.viewport.metrics_columns, expected_metrics);

        let form = sidebar_form_for_width(w);
        assert_eq!(form.width_px(), expected_sidebar);

        // Render every single route under this viewport to guarantee zero crash and zero unhandled bounds
        for route in routes {
            let _ = state.update(Message::Navigate(route));
            assert_eq!(state.shell.current_route, route);
            let view_element = state.view();
            drop(view_element);
        }
    }
}
