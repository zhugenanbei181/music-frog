//! Responsive multi-size elasticity tests for the Iced shell.
//!
//! Verifies that the window-resize message drives the shared 4-tier viewport
//! projection and that the shell's navigation form and content padding follow
//! it, instead of the sidebar staying static. Mounted via `src/test_mounts.rs`.
//! test-intent: behavior

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::sidebar::{RAIL_WIDTH, SIDEBAR_WIDTH, sidebar_form_for_width};
use infiltrator_contract::responsive_viewport::{SidebarForm, ViewportTier};

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
    assert_eq!(SidebarForm::Rail.width_px(), Some(RAIL_WIDTH as u16));

    // Expanded: standard labelled sidebar, width from the shared contract.
    assert_eq!(sidebar_form_for_width(1000.0), SidebarForm::Standard);
    assert_eq!(SidebarForm::Standard.width_px(), Some(SIDEBAR_WIDTH as u16));
    assert_eq!(SIDEBAR_WIDTH, 240.0);

    // Ultra: wide sidebar.
    assert_eq!(sidebar_form_for_width(1600.0), SidebarForm::Wide);
    assert_eq!(SidebarForm::Wide.width_px(), Some(280));
}

#[test]
fn responsive_padding_shrinks_on_narrow_tiers() {
    // A compact window must not keep the wide 48px desktop gutters.
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::WindowResized(400.0, 800.0));
    assert_eq!(state.shell.viewport.tier.content_padding_px(), 16);

    let _ = state.update(Message::WindowResized(1920.0, 1080.0));
    assert_eq!(state.shell.viewport.tier.content_padding_px(), 48);
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
    use infiltrator_contract::overview_layout::OverviewCardKind;

    let (mut state, _) = AppState::new();
    assert_eq!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );

    // Move the first card down: it swaps with the second card.
    let _ = state.update(Message::MoveOverviewCardDown(OverviewCardKind::ModeSegment));
    assert_eq!(state.diag.overview_card_order[0], OverviewCardKind::Traffic);
    assert_eq!(
        state.diag.overview_card_order[1],
        OverviewCardKind::ModeSegment
    );

    // Move it back up restores the canonical order.
    let _ = state.update(Message::MoveOverviewCardUp(OverviewCardKind::ModeSegment));
    assert_eq!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );

    // Moving the top card up is a no-op (already first).
    let _ = state.update(Message::MoveOverviewCardUp(OverviewCardKind::ModeSegment));
    assert_eq!(
        state.diag.overview_card_order[0],
        OverviewCardKind::ModeSegment
    );

    // After a custom order, reset restores the default.
    let _ = state.update(Message::MoveOverviewCardUp(OverviewCardKind::Quota));
    assert_ne!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
    let _ = state.update(Message::ResetOverviewCardOrder);
    assert_eq!(
        state.diag.overview_card_order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
}
